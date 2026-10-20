// Browser UI connected to two real, isolated --observe-only daemons supplied by
// test-pairing.mjs. No input capture, injection, or production profile access.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';

export async function checkAutosave(a,b) {
  assert.ok(a.status.observe_only && b.status.observe_only);
  const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
  const root=path.resolve('apps/desktop/dist');
  const server=createServer((req,res)=>{
    const name=new URL(req.url,'http://localhost').pathname;
    const file=path.resolve(root,name==='/'?'index.html':'.'+name);
    if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}
    res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');res.end(fs.readFileSync(file));
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const browser=await chromium.launch({channel:'msedge',headless:true});
  const errors=[],commands=[];
  const wait=ms=>new Promise(r=>setTimeout(r,ms));
  async function until(check,label){for(let i=0;i<120;i++){if(await check())return;await wait(100);}throw Error('native autosave timeout: '+label);}
  try {
    const pages=[];
    for(const d of [a,b]) {
      const ctx=await browser.newContext({viewport:{width:1200,height:820}}),p=await ctx.newPage();
      p.on('pageerror',e=>errors.push(e.message));
      await p.exposeFunction('nativeTestInvoke',async(command,args)=>{
        assert.equal(d.status.observe_only,true);
        if(command==='daemon_status')return structuredClone(d.status);
        if(command==='daemon_command') {
          assert.equal(args.message.command,'configure','UI test permits only configuration');
          commands.push(args.message);d.child.stdin.write(JSON.stringify(args.message)+'\n');return;
        }
        throw Error('Not allowed in observe-only UI test: '+command);
      });
      await p.addInitScript(()=>{globalThis.isTauri=true;globalThis.__TAURI_INTERNALS__={invoke:(c,a)=>globalThis.nativeTestInvoke(c,a)};});
      await p.goto(`http://127.0.0.1:${server.address().port}`);
      await p.getByRole('slider',{name:'鼠标移动速度'}).waitFor();pages.push(p);
    }
    const [pa,pb]=pages,sa=pa.getByRole('slider',{name:'鼠标移动速度'}),sb=pb.getByRole('slider',{name:'鼠标移动速度'});
    await sa.fill('2.25');await until(()=>b.status.config.tuning.mouse_speed===2.25,'A to B native save');
    await until(async()=>await sb.inputValue()==='2.25','B UI poll');
    await sb.fill('1.5');await until(()=>a.status.config.tuning.mouse_speed===1.5,'B to A native save');
    await until(async()=>await sa.inputValue()==='1.5','A UI poll');
    const out=path.resolve(process.env.UI_CAPTURE_DIRECTORY||'output/playwright/mouse-handoff-2026-10-20');fs.mkdirSync(out,{recursive:true});
    for(const [index,p] of pages.entries()){await p.evaluate(()=>{const label=document.createElement('div');label.textContent='真实后台 · 隔离测试 · 未捕获输入';label.style='position:fixed;bottom:7px;left:40%;font:13px Segoe UI;color:#805b15;background:#fff6df;z-index:99;padding:2px 8px';document.body.append(label);});await p.screenshot({path:path.join(out,`native-home-${index+1}-1200.png`)});}
    await pa.getByRole('navigation',{name:'主导航',exact:true}).getByRole('button',{name:'工作区',exact:true}).click();
    const target=await pa.locator(`[data-device-group="${b.status.identity}"] .monitor`).first().boundingBox();
    const local=await pa.locator(`[data-device-group="${a.status.identity}"] .monitor`).first().boundingBox();
    await pa.mouse.move(target.x+target.width/2,target.y+target.height/2);await pa.mouse.down();
    await pa.mouse.move(local.x+local.width/2,local.y-70,{steps:12});await pa.mouse.up();
    await until(()=>a.status.config.links.some(l=>l.from.device===a.status.identity&&l.from.edge==='Top'),'drag applies routes');
    await until(()=>JSON.stringify(a.status.config.links)===JSON.stringify(b.status.config.links),'drag synchronizes real backend');
    for(const d of [a,b]) {assert.equal(d.status.received,0);assert.equal(d.status.active,null);assert.equal(d.status.observe_only,true);}
    assert.deepEqual(errors,[]);assert.equal(commands.length,3,'speed in both directions and one layout drag, no save-button commands');
    console.log('PASS browser autosave → real daemon receipt → TLS peer → second UI; reciprocal speed and dragged layout, zero native input');
  } finally {await browser.close();await new Promise(r=>server.close(r));}
}
