// Two isolated native daemon processes, real loopback TCP, no input roles or hooks.
import {spawn,spawnSync} from 'node:child_process';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import assert from 'node:assert/strict';
import {placePeer} from '../apps/desktop/src/switchingLayout.ts';
const exe=path.resolve(process.argv[2] ?? 'target/debug/crossdesk-daemon.exe');
const edition=spawnSync(exe,['--profile-info'],{encoding:'utf8',windowsHide:true});
const dataDirectory=edition.status===0?JSON.parse(edition.stdout).data_directory:'CrossDesk';
const root=path.resolve('artifacts',`pairing-${Date.now()}`);
fs.mkdirSync(root,{recursive:true});
const children=[];
function daemon(name) {
  const profile=path.join(root,name); fs.mkdirSync(profile,{recursive:true});
  const child=spawn(exe,['--stdio','--observe-only'],{windowsHide:true,stdio:['pipe','pipe','pipe'],env:{...process.env,LOCALAPPDATA:profile}});
  const d={child,status:null,error:'',waiters:[],seq:0};children.push(d);
  child.stderr.on('data',b=>d.error+=b.toString());
  readline.createInterface({input:child.stdout}).on('line',line=>{d.status=JSON.parse(line);for(const f of [...d.waiters])f();});
  return d;
}
function until(d,predicate,label,ms=12000) {
  return new Promise((resolve,reject)=>{
    let timer;
    const check=()=>{if(d.status && predicate(d.status)){clearTimeout(timer);d.waiters=d.waiters.filter(f=>f!==check);resolve(d.status);}};
    timer=setTimeout(()=>{d.waiters=d.waiters.filter(f=>f!==check);reject(Error(`timeout: ${label}; ${d.error}`));},ms);
    d.waiters.push(check);check();
  });
}
async function cmd(d,command,fields={}) {
  const id=String(++d.seq);d.child.stdin.write(JSON.stringify({command,...fields,request_id:id})+'\n');
  const s=await until(d,s=>s.last_command?.id===id,command);
  if(s.last_command.error)throw Error(s.last_command.error);
  return s;
}
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
async function portPair() {
  for(let attempt=0;attempt<20;attempt++) {
    // Keep test listening ports outside Windows' ephemeral range: mDNS wake sockets also use ephemeral TCP ports.
    const port=30000+Math.floor(Math.random()*10000);
    const first=net.createServer();
    const ready=await new Promise(resolve=>{first.once('error',()=>resolve(false));first.listen(port,'0.0.0.0',()=>resolve(true));});
    if(!ready)continue;
    const second=net.createServer();
    const available=await new Promise(resolve=>{second.once('error',()=>resolve(false));second.listen(port+1,'0.0.0.0',()=>resolve(true));});
    await new Promise(resolve=>first.close(resolve));if(available){await new Promise(resolve=>second.close(resolve));return port;}
  }
  throw Error('cannot allocate test port');
}
let a=daemon('a');let b=daemon('b');let eofServer;const eofSockets=new Set();
try {
  await Promise.all([until(a,s=>!!s.config,'A startup'),until(b,s=>!!s.config,'B startup')]);
  const pa=await portPair(),pb=await portPair();
  await cmd(a,'configure',{config:{...a.status.config,port:pa}});
  await cmd(b,'configure',{config:{...b.status.config,port:pb}});
  try {
    await Promise.all([
      until(a,s=>s.discovered.some(p=>p.id===b.status.identity && p.addresses.some(v=>v.endsWith(`:${pb}`))),'A discovery',8000),
      until(b,s=>s.discovered.some(p=>p.id===a.status.identity && p.addresses.some(v=>v.endsWith(`:${pa}`))),'B discovery',8000),
    ]);
    console.log('PASS native mDNS: both local daemons discovered each other with updated ports');
  } catch {
    console.log('NOT VERIFIED native mDNS on this network; manual IP tests continue');
  }
  async function begin(codeOverride) {
    await cmd(a,'pair_cancel');await cmd(b,'pair_open');
    const s=await until(b,s=>s.pairing?.phase==='listening','B code ready');
    const code=codeOverride ?? s.pairing.code;
    await cmd(a,'pair_connect',{address:`127.0.0.1:${pb}`,code});
  }
  await cmd(b,'pair_open');
  const wrong=b.status.pairing.code==='00000000'?'00000001':'00000000';
  await cmd(a,'pair_connect',{address:`127.0.0.1:${pb}`,code:wrong});
  await until(a,s=>s.pairing?.phase==='error','wrong code rejected');
  assert.equal(a.status.trusted.length,0);assert.equal(b.status.trusted.length,0);
  console.log('PASS wrong code: no trust saved on either side');
  for(let attempt=1;attempt<5;attempt++) {
    await cmd(a,'pair_connect',{address:`127.0.0.1:${pb}`,code:wrong});
    await until(a,s=>s.pairing?.phase==='error','wrong code rejected');
  }
  await until(b,s=>s.pairing?.phase==='error','attempt limit');
  assert.equal(b.status.pairing.code,null);
  assert.equal(b.status.trusted.length,0);
  console.log('PASS five failed attempts close the pairing window and erase displayed code');
  await begin();
  await Promise.all([until(a,s=>s.pairing?.phase==='confirming','A confirmation'),until(b,s=>s.pairing?.phase==='confirming','B confirmation')]);
  assert.equal(a.status.pairing.code,b.status.pairing.code);
  await assert.rejects(()=>cmd(a,'pair_confirm',{session:'stale-session',accepted:true}));
  await cmd(a,'pair_confirm',{session:a.status.pairing.session_id,accepted:true});
  await sleep(700);
  assert.equal(a.status.trusted.length,0);assert.equal(b.status.trusted.length,0);
  console.log('PASS stale confirmation rejected; one-sided confirmation cannot authorize');
  await cmd(b,'pair_confirm',{session:b.status.pairing.session_id,accepted:false});
  await Promise.all([until(a,s=>s.pairing?.phase==='error','A rejection'),until(b,s=>s.pairing?.phase==='error','B rejection')]);
  assert.equal(a.status.trusted.length,0);assert.equal(b.status.trusted.length,0);
  console.log('PASS rejection: neither side is trusted');
  await begin();
  await Promise.all([until(a,s=>s.pairing?.phase==='confirming','A confirmation'),until(b,s=>s.pairing?.phase==='confirming','B confirmation')]);
  await cmd(a,'pair_cancel');
  await until(b,s=>s.pairing?.phase==='error','disconnect cancels pending pairing');
  assert.equal(a.status.trusted.length,0);assert.equal(b.status.trusted.length,0);
  console.log('PASS cancellation/disconnect: no trust saved');
  await begin();
  await Promise.all([until(a,s=>s.pairing?.phase==='confirming','A confirmation'),until(b,s=>s.pairing?.phase==='confirming','B confirmation')]);
  await Promise.all([cmd(a,'pair_confirm',{session:a.status.pairing.session_id,accepted:true}),cmd(b,'pair_confirm',{session:b.status.pairing.session_id,accepted:true})]);
  await Promise.all([until(a,s=>s.pairing?.phase==='completed','A completed'),until(b,s=>s.pairing?.phase==='completed','B completed')]);
  assert.equal(a.status.trusted[0].id,b.status.identity);assert.equal(b.status.trusted[0].id,a.status.identity);
  assert.equal(a.status.config.endpoints[0].address,`127.0.0.1:${pb}`);
  assert.equal(b.status.config.endpoints[0].address,`127.0.0.1:${pa}`);
  assert.equal(a.status.pairing.code,null);assert.equal(b.status.pairing.code,null);
  for(const d of[a,b]){assert.equal(d.status.config.sharing,true);assert.equal(d.status.observe_only,true);assert.equal(d.status.active,null);}
  console.log('PASS mutual confirmation: identities pinned, both endpoints saved, automatic sharing authorized, observation mode prevents native input');
  for(const name of['a','b'])assert.equal(JSON.parse(fs.readFileSync(path.join(root,name,dataDirectory,'trust.json'),'utf8')).length,1);
  console.log('PASS both trust stores persisted through normal native storage');
  await Promise.all([
    until(a,s=>s.topology.some(t=>t.device.id===b.status.identity && t.device.online),'A authenticated presence'),
    until(b,s=>s.topology.some(t=>t.device.id===a.status.identity && t.device.online),'B authenticated presence'),
  ]);
  assert.equal(a.status.effective_links.length,2);assert.equal(b.status.effective_links.length,2);
  await sleep(3500);
  for(const d of[a,b]) {assert.equal(d.status.topology.filter(t=>t.device.online).length,2);assert.equal(d.status.received,0);}
  console.log('PASS real mutual TLS presence persists with automatic reciprocal edges and zero input');
  for (const [local,remote] of [[a,b],[b,a]]) {
    const own=local.status.topology.find(t=>t.device.id===local.status.identity);
    const received=remote.status.topology.find(t=>t.device.id===local.status.identity);
    assert.deepEqual(received.monitors,own.monitors,'all screen geometry/rotation/scale survives authenticated transport');
    assert.equal(received.version,own.version);
  }
  await cmd(a,'refresh');
  assert.equal(a.status.topology_error,null);
  assert.ok(a.status.topology.some(t=>t.device.id===b.status.identity&&t.device.online));
  console.log('PASS real screen geometry/rotation/scale matches on both ends; unchanged refresh preserves connection');
  const visualLinks=placePeer(a.status.topology,a.status.effective_links,a.status.identity,b.status.identity,'Top');
  await cmd(a,'configure',{config:{...a.status.config,links:visualLinks}});
  await until(a,s=>s.effective_links.length===2&&s.effective_links[0].from.edge==='Top','visual placement routes applied');
  assert.deepEqual(a.status.effective_links,visualLinks);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root,'a',dataDirectory,'config.json'),'utf8')).links,visualLinks);
  assert.equal(a.status.received,0);
  await until(b,s=>JSON.stringify(s.config.links)===JSON.stringify(visualLinks)&&JSON.stringify(s.effective_links)===JSON.stringify(visualLinks),'saved layout synchronized to receiver');
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(root,'b',dataDirectory,'config.json'),'utf8')).links,visualLinks);
  const returnLinks=placePeer(b.status.topology,b.status.effective_links,b.status.identity,a.status.identity,'Left');
  await cmd(b,'configure',{config:{...b.status.config,links:returnLinks}});
  await until(a,s=>JSON.stringify(s.config.links)===JSON.stringify(returnLinks)&&JSON.stringify(s.effective_links)===JSON.stringify(returnLinks),'receiver can edit and synchronize back to source');
  assert.equal(a.status.received,0);assert.equal(b.status.received,0);
  console.log('PASS authenticated layout sync in both directions, persisted on both peers with zero input');
  await cmd(a,'configure',{config:{...a.status.config,links:[]}});
  await until(a,s=>s.config.links.length===0&&s.topology.some(t=>t.device.id===b.status.identity&&t.device.online)&&s.effective_links.length===2,'default routes restored');
  await until(b,s=>s.config.links.length===0&&s.effective_links.length===2,'restore defaults synchronized to receiver');
  console.log('PASS visual placement uses real screen IDs: reciprocal top/bottom routes acknowledged, persisted and applied; defaults restored with zero input');
  await cmd(a,'configure',{config:{...a.status.config,same_subnet_only:false,tuning:{...a.status.config.tuning,mouse_speed:2,switch_modifier:'Shift'},appearance:{dark:true,debug_info:true}}});
  await until(b,s=>s.config.same_subnet_only===false&&s.config.tuning.mouse_speed===2&&s.config.tuning.switch_modifier==='Shift'&&s.config.appearance.dark===true,'A settings synchronized to B');
  assert.equal(b.status.config.port,pb,'sync preserves local port');
  await cmd(b,'configure',{config:{...b.status.config,same_subnet_only:true,tuning:{...b.status.config.tuning,mouse_speed:1.5,switch_modifier:'None'},appearance:{dark:false,debug_info:false}}});
  await until(a,s=>s.config.same_subnet_only===true&&s.config.tuning.mouse_speed===1.5&&s.config.tuning.switch_modifier==='None'&&s.config.appearance.dark===false,'B settings synchronized to A');
  assert.equal(a.status.config.port,pa);
  for(const [name,d] of [['a',a],['b',b]]) {
    const saved=JSON.parse(fs.readFileSync(path.join(root,name,dataDirectory,'config.json'),'utf8'));
    assert.equal(saved.tuning.mouse_speed,1.5);assert.equal(saved.tuning.switch_modifier,'None');
    assert.equal(saved.appearance.debug_info,false);assert.equal(d.status.received,0);
  }
  console.log('PASS bidirectional input/appearance settings sync, immediate applied values, local ports preserved and persistence, zero input');
  if(process.argv.includes('--autosave-ui')) {
    const {checkAutosave}=await import('./check-autosave-native-browser.mjs');
    await checkAutosave(a,b);
  }
  const beforeInvalid=a.status.config;
  await assert.rejects(()=>cmd(a,'configure',{config:{...beforeInvalid,port:65535}}));
  assert.deepEqual(a.status.config,beforeInvalid);
  assert.ok(a.status.topology.some(t=>t.device.id===b.status.identity&&t.device.online));
  console.log('PASS invalid port rejected before stopping an established connection; saved configuration retained');
  for(const d of[a,b]) {
    assert.ok(Array.isArray(d.status.local_addresses));
    for(const address of d.status.local_addresses)assert.ok(address.address.endsWith(':'+d.status.config.port));
  }
  await cmd(a,'export_diagnostics');
  const diagnosticPath=a.status.diagnostics_path;
  assert.ok(diagnosticPath&&fs.existsSync(diagnosticPath));
  await sleep(1200);
  assert.equal(a.status.diagnostics_path,diagnosticPath);
  const diagnostic=fs.readFileSync(diagnosticPath,'utf8');
  assert.ok(!diagnostic.includes(a.status.identity));
  assert.ok(!diagnostic.includes(b.status.identity));
  assert.ok(!diagnostic.includes('127.0.0.1'));
  console.log('PASS local-address field uses current port; exported path survives status refresh and diagnostics omit peer identity/IP');
  await cmd(a,'pause');
  await until(b,s=>s.topology.some(t=>t.device.id===a.status.identity && t.device.online) && s.peer_sharing[a.status.identity]===false,'paused A stays online');
  assert.equal(a.status.config.sharing,false);
  await cmd(a,'resume');
  await until(a,s=>s.enabled && s.topology.some(t=>t.device.id===b.status.identity && t.device.online),'roleless resume');
  await sleep(2500);
  assert.equal(a.status.topology.filter(t=>t.device.online).length,2);
  assert.equal(b.status.topology.filter(t=>t.device.online).length,2);
  console.log('PASS pause/resume keeps authenticated screen presence without selecting a role');
  const oldB=b;
  await new Promise(resolve=>{oldB.child.once('exit',resolve);oldB.child.stdin.end(JSON.stringify({command:'quit'})+'\n');});
  await until(a,s=>s.topology.some(t=>t.device.id===oldB.status.identity && !t.device.online),'offline after shutdown');
  await until(a,s=>!!s.connection_issues?.[oldB.status.identity],'disconnect diagnostic recorded per peer');
  assert.equal(a.status.last_error,null,'passive disconnection must not become a global error');
  // Reproduce cold startup with saved trust and a peer that closes during TLS.
  eofServer=net.createServer(socket=>{eofSockets.add(socket);socket.once('close',()=>eofSockets.delete(socket));socket.on('error',()=>{});socket.end();});
  await new Promise((resolve,reject)=>{eofServer.once('error',reject);eofServer.listen(pb,'127.0.0.1',resolve);});
  const oldA=a;
  await new Promise(resolve=>{oldA.child.once('exit',resolve);oldA.child.stdin.end(JSON.stringify({command:'quit'})+'\n');});
  a=daemon('a');
  await until(a,s=>/tls handshake eof/i.test(s.connection_issues?.[oldB.status.identity]??''),'cold-start TLS EOF diagnostic');
  let noisy=false;const watch=()=>{if(a.status.last_error)noisy=true;};a.waiters.push(watch);
  await sleep(2600);a.waiters=a.waiters.filter(f=>f!==watch);
  assert.equal(noisy,false,'repeated automatic reconnects must stay out of global errors');
  assert.equal(a.status.last_error,null);assert.equal(a.status.trusted[0].id,oldB.status.identity);
  assert.equal(a.status.pairing,null);assert.equal(a.status.received,0);assert.equal(a.status.active,null);
  console.log('PASS saved-pair cold startup and repeated TLS EOF stay quiet; per-device reason retained, trust preserved, zero input');
  for(const socket of eofSockets)socket.destroy();
  await new Promise(resolve=>eofServer.close(resolve));eofServer=undefined;
  b=daemon('b');
  await until(b,s=>s.identity===oldB.status.identity && s.config.sharing,'persisted identity and consent');
  await Promise.all([
    until(a,s=>s.topology.some(t=>t.device.id===b.status.identity && t.device.online),'A reconnect'),
    until(b,s=>s.topology.some(t=>t.device.id===a.status.identity && t.device.online),'B reconnect'),
  ]);
  assert.equal(a.status.connection_issues?.[b.status.identity],undefined,'reconnect clears only this peer diagnostic');
  assert.equal(b.status.connection_issues?.[a.status.identity],undefined);
  assert.equal(a.status.last_error,null);assert.equal(b.status.last_error,null);
  assert.equal(b.status.received,0);assert.equal(b.status.observe_only,true);
  for(const d of [a,b]) {assert.equal(d.status.config.tuning.mouse_speed,1.5);assert.equal(d.status.config.tuning.switch_modifier,'None');}
  console.log('PASS restart automatically reconnects both peers using saved trust and addresses; no old input replay');
  if(process.argv.includes('--expiry')) {
    await cmd(b,'pair_open');
    await until(b,s=>s.pairing?.phase==='error','real 120-second expiry',126000);
    assert.equal(b.status.pairing.code,null);
    assert.equal(b.status.observe_only,true);
    console.log('PASS actual 120-second pairing expiry: code cleared, input remains OFF');
  }
  await cmd(a,'revoke',{id:b.status.identity});
  await until(a,s=>s.trusted.length===0&&!s.topology.some(t=>t.device.id===b.status.identity&&t.device.online),'revocation disconnects current peer');
  await sleep(3000);
  assert.ok(!a.status.topology.some(t=>t.device.id===b.status.identity&&t.device.online));
  assert.equal(a.status.active,null);
  assert.equal(a.status.connection_issues?.[b.status.identity],undefined,'revoke removes the peer diagnostic');
  assert.equal(a.status.received,0);
  console.log('PASS revoke disconnects the active TLS peer and saved identity cannot reconnect without fresh authorization');
} finally {
  for(const socket of eofSockets)socket.destroy();
  if(eofServer?.listening)await new Promise(resolve=>eofServer.close(resolve));
  await Promise.all(children.map(d=>new Promise(resolve=>{
    if(d.child.exitCode!==null){resolve();return;}
    d.child.once('exit',resolve);d.child.stdin.end(JSON.stringify({command:'quit'})+'\n');
    setTimeout(()=>{if(d.child.exitCode===null)d.child.kill();resolve();},3000).unref();
  })));
}




