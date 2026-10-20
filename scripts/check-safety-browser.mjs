// Browser fixtures only. Never starts a native daemon or sends global input.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {makeDemo} from '../apps/desktop/src/demoScenarios.ts';
const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
const baseline=process.argv.includes('--baseline');
const root=path.resolve(baseline?'output/input-safety-2026-10-20/baseline/dist':'apps/desktop/dist');
const out=path.resolve('output/playwright/input-safety-2026-10-20');
fs.mkdirSync(out,{recursive:true});
const server=createServer((req,res)=>{
  const name=new URL(req.url,'http://localhost').pathname;
  const file=path.resolve(root,name==='/'?'index.html':'.'+name);
  if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}
  res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');
  res.end(fs.readFileSync(file));
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch({channel:'msedge',headless:true});
const errors=[];
try {
  for(const [theme,width,height] of [['light',940,640],['dark',1200,820]]) {
    const ctx=await browser.newContext({viewport:{width,height}});
    const page=await ctx.newPage();
    page.on('pageerror',e=>errors.push(e.message));
    const state=makeDemo('workspace','windows');
    state.enabled=false;state.config.sharing=false;
    state.config.appearance={dark:theme==='dark'};
    state.last_error='输入调度未及时响应，共享已暂停。松开按键后可重新启用共享。（等待 612 ms）';
    const commands=[];
    await page.exposeFunction('safetyTestInvoke',async(command,args)=>{
      if(command==='daemon_status')return structuredClone(state);
      if(command==='platform_name')return 'windows';
      if(command==='daemon_command'){
        const m=typeof args.message==='string'?JSON.parse(args.message):args.message;
        assert.equal(m.command,'resume');
        commands.push(m);
        // Success only after the matching native acknowledgement.
        setTimeout(()=>{state.enabled=true;state.config.sharing=true;state.last_error=null;state.last_command={id:m.request_id,error:null};},150);
        return;
      }
      throw Error(`Unexpected test command ${command}`);
    });
    await page.addInitScript(({theme})=>{
      localStorage.setItem('crossdesk.ui.theme',theme);
      globalThis.isTauri=true;
      globalThis.__TAURI_INTERNALS__={invoke:(c,a)=>globalThis.safetyTestInvoke(c,a)};
    },{theme});
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    await page.getByText(baseline?'操作未完成':'输入处理暂时中断',{exact:true}).waitFor();
    assert.equal(await page.getByText('请核对对方 IP',{exact:false}).count(),0);
    await page.evaluate(()=>{
      const label=document.createElement('div');
      label.textContent='测试样例 · 无真实输入';
      label.style='position:fixed;top:17px;left:50%;transform:translateX(-50%);z-index:999;background:#fff6df;color:#805b15;font-size:13px;padding:3px 8px';
      document.body.append(label);
    });
    await page.screenshot({path:path.join(out,`${baseline?'before':'after'}-${width}-${theme}-FIXTURE.png`)});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
    await page.getByText('原始详情',{exact:true}).click();
    await page.locator('.error-notice pre').getByText('等待 612 ms',{exact:false}).waitFor();
    await page.getByRole('button',{name:'启用共享',exact:true}).click();
    await page.getByRole('button',{name:'暂停共享',exact:true}).waitFor();
    assert.equal(commands.length,1);
    if(!baseline)assert.equal(await page.locator('.global-error').count(),0,'confirmed recovery clears the resolved alert');
    await ctx.close();
  }
  assert.deepEqual(errors,[]);
  console.log('PASS precise error, expandable original timing, resume acknowledgement, 940 light / 1200 dark, no overflow or runtime error');
} finally {
  await browser.close();
  await new Promise(r=>server.close(r));
}
