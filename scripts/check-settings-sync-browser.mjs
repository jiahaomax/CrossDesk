// Isolated browser fixtures only. No native process, network command or global input.
// Uses the production build, with an explicitly labelled in-memory IPC test double.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {makeDemo} from '../apps/desktop/src/demoScenarios.ts';
const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
const before=process.argv.includes('--baseline');
const root=path.resolve(before?(process.env.UI_BASELINE_DIRECTORY||'output/ui-refresh-2026-10-18/baseline/dist'):'apps/desktop/dist');
const out=path.resolve(process.env.UI_CAPTURE_DIRECTORY||'output/playwright/autosave-2026-10-19');fs.mkdirSync(out,{recursive:true});
const report={source:'Labelled synthetic fixtures; browser CSS pixels, NOT native Windows DPI or native input',captures:[],checks:[],errors:[]};
const server=createServer((req,res)=>{const name=new URL(req.url,'http://localhost').pathname;const file=path.resolve(root,name==='/'?'index.html':'.'+name);if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const url=`http://127.0.0.1:${server.address().port}`;
const browser=await chromium.launch({channel:'msedge',headless:true});
const wait=ms=>new Promise(r=>setTimeout(r,ms));
function sample(scenario='workspace',platform='windows'){const s=makeDemo(scenario,platform);if(s){s.config.same_subnet_only=true;s.state='独立测试样例 · 没有真实设备或输入';s.config.endpoints=[{peer:'b',address:'192.0.2.20:24872'}];if(scenario==='reconnecting'){for(const t of s.topology)if(t.device.id!==s.identity)t.device.online=false;s.peer_sharing={};s.connection_issues={b:'tls handshake eof',c:'连接超时'};}}return s;}
async function setup({scenario='workspace',width=940,height=640,dpr=1,theme='light',platform='windows',native=true}={}) {
 const ctx=await browser.newContext({viewport:{width,height},deviceScaleFactor:dpr});const page=await ctx.newPage();page.setDefaultTimeout(10000);let state=sample(scenario,platform),rejectRead=false,failNext=false,delay=350;const commands=[];
 page.on('pageerror',e=>report.errors.push(e.message));
 if(native){await page.exposeFunction('uiTestInvoke',async(command,args)=>{if(command==='daemon_status'){if(rejectRead)throw Error('后台暂时不可用：测试样例断开');return state;}if(command==='daemon_command'){const m=args.message;commands.push(m);setTimeout(()=>{if(failNext){state.last_command={id:m.request_id,error:'测试样例：配置文件写入失败。你的草稿已保留，请重试。'};state.last_error=state.last_command.error;failNext=false;return;}if(m.command==='configure'){const sharing=state.config.sharing;state.config=structuredClone(m.config);state.config.sharing=sharing;state.effective_links=state.config.links.length?structuredClone(state.config.links):sample('workspace').effective_links;}else if(m.command==='pause'){state.enabled=false;state.config.sharing=false;state.pairing=null;}else if(m.command==='resume'){state.enabled=true;state.config.sharing=true;}else if(m.command==='pair_connect'){state.enabled=false;state.pairing={phase:'confirming',session_id:'test-confirm',peer_name:'测试笔记本',peer_address:m.address,code:null,expires_at_ms:Date.now()+90000,error:null};}else if(m.command==='pair_open'){state.enabled=false;state.pairing={phase:'listening',session_id:'test-code',peer_name:null,peer_address:null,code:'12345678',expires_at_ms:Date.now()+90000,error:null};}else if(m.command==='pair_cancel'){state.pairing=null;}else if(m.command==='pair_confirm'){state.pairing.phase=m.accepted?'waiting':'error';}else if(m.command==='revoke'){state.trusted=state.trusted.filter(p=>p.id!==m.id);state.topology=state.topology.filter(t=>t.device.id!==m.id);}else if(m.command==='export_diagnostics')state.diagnostics_path='C:\\测试样例\\diagnostics.json';state.last_command={id:m.request_id,error:null};state.last_error=null;},delay);return;}if(command==='quit_app'){commands.push({command});return;}throw Error('test double forbids '+command);});}
 await page.addInitScript(({theme,platform,native})=>{localStorage.setItem('crossdesk.ui.theme',theme);Object.defineProperty(navigator,'platform',{get:()=>platform==='macos'?'MacIntel':platform==='ubuntu'?'Linux x86_64':'Win32'});if(native){globalThis.isTauri=true;globalThis.__TAURI_INTERNALS__={invoke:(c,a)=>globalThis.uiTestInvoke(c,a)};}}, {theme,platform,native});
 await page.goto(url);if(native)await page.locator('.screen-target').first().waitFor();
 await page.evaluate(()=>{const n=document.createElement('div');n.textContent='测试样例 · 无真实设备 / 输入';n.setAttribute('data-test-label','');n.style='position:fixed;left:50%;transform:translateX(-50%);bottom:7px;z-index:99;font:13px Segoe UI,Microsoft YaHei;background:#fff6df;color:#805b15;padding:2px 8px;border-radius:4px;pointer-events:none';document.body.append(n);});
 return {page,ctx,commands,get state(){return state;},set state(s){state=s;},fail(){failNext=true;},readFail(v){rejectRead=v;},delay(ms){delay=ms;}};
}
async function capture(t,name){const p=t.page;await p.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));if(!before&&/^workspace-\d+-light-FIXTURE$/.test(name)){const canvas=await p.locator('.canvas').boundingBox();for(const r of await p.locator('.monitor').all()){const b=await r.boundingBox();assert.ok(b.x>=canvas.x-1&&b.x+b.width<=canvas.x+canvas.width+1&&b.y>=canvas.y&&b.y+b.height<=canvas.y+canvas.height-48,name+' fitted screen clipped');}}await p.screenshot({animations:'disabled',path:path.join(out,`${before?'before':'after'}-${name}.png`)});report.captures.push({name,viewport:p.viewportSize(),canvas:await p.locator('.canvas-wrap').isVisible()?await p.locator('.canvas-wrap').boundingBox():null,overflow:await p.evaluate(()=>document.documentElement.scrollWidth>innerWidth)});assert.equal(report.captures.at(-1).overflow,false,name+' page overflow');}
async function nav(t,label){await t.page.getByRole('navigation',{name:'主导航',exact:true}).getByRole('button',{name:label,exact:true}).click();}

async function until(predicate,label) {for(let i=0;i<120;i++){if(await predicate())return;await wait(50);}throw Error('timeout: '+label);}
try {
 for(const [width,height,theme] of [[940,640,'light'],[1200,820,'light'],[940,640,'dark']]) {
  const t=await setup({width,height,theme});const p=t.page;
  await nav(t,'设置');await p.getByRole('button',{name:'输入与快捷键',exact:true}).click();
  const speed=p.getByRole('slider',{name:'远端鼠标速度'}),modifier=p.getByLabel('跨机切换按键');
  assert.equal(await p.getByRole('button',{name:/保存设置|放弃修改/}).count(),0);
  assert.equal(await speed.inputValue(),'1');
  for(const value of ['1.5','2','2.5']){await speed.fill(value);await wait(70);}
  await modifier.selectOption('Shift');await speed.focus();
  await until(()=>t.state.config.tuning.mouse_speed===2.5,'automatic save');
  await wait(550);assert.equal(t.commands.length,1,'rapid edits coalesce into one command');
  assert.equal(t.state.config.tuning.switch_modifier,'Shift');
  assert.equal(await speed.evaluate(el=>el===document.activeElement),true);
  await capture(t,`input-settings-${width}-${theme}-FIXTURE`);
  // Slow receipt must not overwrite a newer edit or disable the focused input.
  t.delay(1000);await speed.fill('2');await until(()=>t.commands.length===2,'first save sent');
  assert.equal(await speed.isEnabled(),true);await speed.fill('3');
  await until(()=>t.state.config.tuning.mouse_speed===2,'older save applied');
  await wait(150);assert.equal(await speed.inputValue(),'3','old receipt cannot roll the control back');
  await until(()=>t.state.config.tuning.mouse_speed===3,'newest edit applied');await wait(400);
  assert.equal(t.commands.length,3);assert.equal(await speed.evaluate(el=>el===document.activeElement),true);
  // A genuine later remote save wins without refresh/discard, preserving focus.
  t.state.config.tuning.mouse_speed=1.5;t.state.config.tuning.switch_modifier='None';
  await until(async()=>await speed.inputValue()==='1.5','remote rebase');
  assert.equal(await modifier.inputValue(),'None');assert.equal(await speed.evaluate(el=>el===document.activeElement),true);
  assert.equal(await p.getByText(/后台同时修改|放弃草稿|另一处修改/).count(),0);
  t.delay(200);t.fail();await speed.fill('2.5');
  await p.getByText('操作未完成',{exact:true}).waitFor();await p.getByRole('button',{name:'重试',exact:true}).waitFor();
  const failedCount=t.commands.length;await wait(1300);assert.equal(t.commands.length,failedCount,'failure does not trigger an endless retry loop');
  assert.equal(await speed.inputValue(),'2.5');assert.equal(t.state.config.tuning.mouse_speed,1.5);
  if(width===940&&theme==='light')await capture(t,'save-failure-940-light-FIXTURE');
  await p.getByRole('button',{name:'重试',exact:true}).click();await until(()=>t.state.config.tuning.mouse_speed===2.5,'retry');await wait(550);
  assert.equal(await p.getByText('操作未完成',{exact:true}).count(),0);
  // Partial numeric entry is not sent; correcting it saves without a button.
  await p.getByText('高级发送节奏与安全超时',{exact:true}).click();
  const heartbeat=p.getByLabel('心跳间隔（50–200 ms）');const beforeInvalid=t.commands.length;
  await heartbeat.fill('2');await wait(800);assert.equal(t.commands.length,beforeInvalid);
  await heartbeat.fill('120');await until(()=>t.state.config.tuning.heartbeat_ms===120,'valid number');
  await nav(t,'帮助');assert.ok((await p.locator('.help-page').innerText()).includes('直接移向边缘'));
  await nav(t,'工作区');await wait(500);
  const rect=await p.locator('.device-group[data-device-group="b"] .monitor').first().boundingBox();
  const local=await p.locator('.device-group[data-device-group="a"] .monitor').first().boundingBox();
  await p.mouse.move(rect.x+rect.width/2,rect.y+rect.height/2);await p.mouse.down();
  await p.mouse.move(local.x+local.width/2,Math.max(120,local.y-70),{steps:12});await p.mouse.up();
  await until(()=>t.state.config.links.length>0,'drag saved routes');await wait(650);
  assert.equal(await p.getByRole('button',{name:/保存布局|保存切换方向/}).count(),0);
  await capture(t,`workspace-${width}-${theme}-FIXTURE`);
  const homeSpeed=p.getByRole('slider',{name:'鼠标移动速度'});
  const direction=await p.getByRole('button',{name:/电脑方向/}).boundingBox(),home=await p.locator('.workspace-mouse-setting').boundingBox();
  assert.ok(home.y>=direction.y+direction.height-1,'mouse control below computer direction');
  assert.ok((await p.locator('.canvas-wrap').boundingBox()).height>=380,'minimum canvas height');
  await homeSpeed.fill('2.75');await until(()=>t.state.config.tuning.mouse_speed===2.75,'home slider auto save');
  await homeSpeed.focus();await p.keyboard.press('ArrowRight');await until(()=>t.state.config.tuning.mouse_speed===2.8,'home keyboard control');
  t.state.config.tuning.mouse_speed=1.25;await until(async()=>await homeSpeed.inputValue()==='1.25','home peer update');
  assert.equal(await homeSpeed.evaluate(el=>el===document.activeElement),true);
  await capture(t,`home-mouse-${width}-${theme}-FIXTURE`);
  await p.getByRole('button',{name:'放大',exact:true}).click();const zoom=await p.getByLabel('画布缩放').innerText();
  t.state.config.links[0].from.start=.1;await wait(800);assert.equal(await p.getByLabel('画布缩放').innerText(),zoom);
  t.state.config.appearance={dark:theme!=='dark',debug_info:true};await wait(800);
  assert.equal(await p.locator('html').getAttribute('data-theme'),theme==='dark'?'light':'dark');
  await nav(t,'设置');await p.getByRole('button',{name:'权限与诊断',exact:true}).click();
  await p.getByRole('checkbox',{name:/显示调试信息/}).click();await until(()=>!t.state.config.appearance.debug_info,'appearance save');
  report.checks.push(`${width}x${height} ${theme}: automatic speed/modifier/layout/appearance, debounce, edit during receipt, remote update, focus, failure/retry, invalid input, viewport`);
  await t.ctx.close();
 }

 const paused=await setup();const p=paused.page;
 await nav(paused,'设置');await p.getByRole('button',{name:'输入与快捷键',exact:true}).click();
 const slider=p.getByRole('slider',{name:'远端鼠标速度'});
 paused.delay(1000);await slider.fill('2');await until(()=>paused.commands.length===1,'autosave before pause');
 await slider.fill('3');paused.delay(1200);await p.getByRole('button',{name:'暂停共享',exact:true}).click();
 await until(()=>paused.state.enabled===false,'pause receipt');
 await until(()=>paused.state.config.tuning.mouse_speed===3,'queued edit after pause');
 assert.equal(paused.state.config.sharing,false);assert.equal(await slider.inputValue(),'3');
 await until(async()=>await p.locator('.autosave-status').count()===0,'save complete');
 await p.getByRole('button',{name:'外观与常用',exact:true}).click();
 await p.getByRole('button',{name:'深色',exact:true}).click();
 await p.getByRole('button',{name:'退出软件',exact:true}).click();
 await p.getByRole('button',{name:'退出并停止共享',exact:true}).click();
 await until(()=>paused.commands.some(c=>c.command==='quit_app'),'quit flushes latest edit');
 assert.equal(paused.state.config.appearance.dark,true);
 assert.equal(paused.commands.at(-1).command,'quit_app');
 report.checks.push('pause preempts saving without restoring sharing or losing a newer edit; quit flushes queued edits');
 await paused.ctx.close();
 assert.deepEqual(report.errors,[]);console.log('PASS rendered autosave, delayed receipts, focus, remote sync, failure/retry, validation and drag (fixtures)');
 fs.writeFileSync(path.join(out,'report.json'),JSON.stringify(report,null,2));
} finally {await browser.close();server.close();}
