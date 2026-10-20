// Three real TLS daemons with fresh profiles and --observe-only.
// No capture, injection, real peer configuration, or user certificates.
import {spawn} from 'node:child_process';
import net from 'node:net';
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import assert from 'node:assert/strict';
import {placePeer} from '../apps/desktop/src/switchingLayout.ts';

const exe=path.resolve(process.argv[2]);
const root=path.resolve('artifacts',`three-layout-${Date.now()}`);
fs.mkdirSync(root,{recursive:true});
const children=[];
const delay=ms=>new Promise(r=>setTimeout(r,ms));
function start(name) {
  const profile=path.join(root,name);fs.mkdirSync(profile,{recursive:true});
  const child=spawn(exe,['--stdio','--observe-only'],{
    windowsHide:true,stdio:['pipe','pipe','pipe'],env:{...process.env,LOCALAPPDATA:profile},
  });
  const d={name,child,status:null,error:'',seq:0};children.push(d);
  child.stderr.on('data',data=>d.error+=data.toString());
  readline.createInterface({input:child.stdout}).on('line',line=>d.status=JSON.parse(line));
  return d;
}
async function until(d,fn,label) {
  const end=Date.now()+18000;
  while(Date.now()<end) {
    if(d.status&&fn(d.status))return d.status;
    if(d.child.exitCode!==null)throw Error(`${label}: backend exited: ${d.error}`);
    await delay(40);
  }
  throw Error(`${label}: timed out; ${d.error}; issues=${JSON.stringify(Object.values(d.status?.connection_issues??{}))}`);
}
async function command(d,command,fields={}) {
  const request_id=String(++d.seq);
  d.child.stdin.write(JSON.stringify({command,request_id,...fields})+'\n');
  const s=await until(d,s=>s.last_command?.id===request_id,command);
  assert.equal(s.last_command.error,null);
  return s;
}
async function quit(d) {
  if(d.child.exitCode!==null)return;
  const done=new Promise(r=>d.child.once('exit',r));
  d.child.stdin.end('{"command":"quit"}\n');
  const timeout=setTimeout(()=>d.child.kill(),4000);
  await done;clearTimeout(timeout);
}
async function port() {
  for(let n=0;n<30;n++) {
    const number=30000+Math.floor(Math.random()*10000);
    const sockets=[net.createServer(),net.createServer()];
    const available=await Promise.all(sockets.map((s,i)=>new Promise(r=>{
      s.once('error',()=>r(false));s.listen(number+i,'0.0.0.0',()=>r(true));
    })));
    await Promise.all(sockets.filter(s=>s.listening).map(s=>new Promise(r=>s.close(r))));
    if(available.every(Boolean))return number;
  }
  throw Error('No isolated test ports available');
}
const online=(d,peer)=>until(d,s=>s.topology.some(t=>t.device.id===peer.status.identity&&t.device.online),'authenticated presence');
async function pair(a,b) {
  await command(b,'pair_open');
  await until(b,s=>s.pairing?.phase==='listening','pair code');
  await command(a,'pair_connect',{address:`127.0.0.1:${b.status.config.port}`,code:b.status.pairing.code});
  await Promise.all([a,b].map(d=>until(d,s=>s.pairing?.phase==='confirming','confirmation')));
  await Promise.all([a,b].map(d=>command(d,'pair_confirm',{session:d.status.pairing.session_id,accepted:true})));
  await Promise.all([a,b].map(d=>until(d,s=>s.pairing?.phase==='completed','paired')));
  await Promise.all([online(a,b),online(b,a)]);
}
const links=(d,a,b)=>d.status.config.links.filter(l=>
  (l.from.device===a&&l.to.device===b)||(l.from.device===b&&l.to.device===a));
const normalized=items=>JSON.stringify([...items].sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b))));
async function stable(a,b,c) {
  await Promise.all([online(a,b),online(b,a),online(a,c),online(c,a)]);
  await until(b,()=>normalized(links(a,a.status.identity,b.status.identity))===normalized(links(b,a.status.identity,b.status.identity)),'same A/B routes');
  await delay(2400);
  for(const d of[a,b,c]){
    assert.equal(d.status.observe_only,true);
    assert.equal(d.status.active,null);
    assert.equal(d.status.received,0);
    assert.equal(d.status.topology.filter(t=>t.device.online).length,d===a?3:2);
    assert.deepEqual(Object.values(d.status.connection_issues),[]);
  }
}
try {
  const machines=['one','two','three'].map(start);
  for(const d of machines) {
    await until(d,s=>!!s.config,'startup');
    await command(d,'configure',{config:{...d.status.config,port:await port()}});
  }
  // Deterministic identity order reproduces the former default-edge collision.
  machines.sort((a,b)=>a.status.identity.localeCompare(b.status.identity));
  let [a,b,c]=machines;
  await pair(a,c);
  const saved=placePeer(a.status.topology,a.status.effective_links,a.status.identity,c.status.identity,'Right');
  await command(a,'configure',{config:{...a.status.config,links:saved}});
  await until(c,s=>normalized(s.config.links)===normalized(saved),'existing pair saved');
  await pair(a,b);
  await stable(a,b,c);
  assert.equal(normalized(links(a,a.status.identity,c.status.identity)),normalized(saved),'existing placement unchanged');
  const newPair=links(a,a.status.identity,b.status.identity);
  assert.equal(newPair.length,2);
  assert.notEqual(newPair.find(l=>l.from.device===a.status.identity).from.edge,'Right','occupied edge not reused');
  console.log('PASS third computer joins existing saved pair; both ends agree on free edge, TLS stays online, zero input');

  const newPlacement=placePeer(b.status.topology,b.status.effective_links,b.status.identity,a.status.identity,'Top');
  await command(b,'configure',{config:{...b.status.config,links:newPlacement}});
  await until(a,()=>normalized(links(a,a.status.identity,b.status.identity))===normalized(newPlacement),'new peer placement sync');
  assert.equal(normalized(links(a,a.status.identity,c.status.identity)),normalized(saved));
  console.log('PASS new peer can move the pair; previous computer placement stays unchanged');

  const aName=a.name;
  await quit(a);a=start(aName);
  await until(a,s=>!!s.config,'hub restart');
  await stable(a,b,c);
  assert.equal(normalized(links(a,a.status.identity,c.status.identity)),normalized(saved));
  console.log('PASS cold restart with saved routes to both peers; arbitrary topology arrival order reconnects');

  const cName=c.name;
  await quit(c);
  await until(a,s=>!s.topology.some(t=>t.device.id===c.status.identity&&t.device.online),'peer offline');
  assert.ok(a.status.topology.some(t=>t.device.id===b.status.identity&&t.device.online));
  c=start(cName);await until(c,s=>!!s.config,'peer restart');
  await stable(a,b,c);
  assert.equal(normalized(links(a,a.status.identity,c.status.identity)),normalized(saved));
  console.log('PASS one peer goes offline and rejoins; other connection and saved placement preserved');
} finally {
  await Promise.all(children.map(quit));
}
