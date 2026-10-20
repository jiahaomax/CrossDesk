// Isolated IPC fixtures: never requests OS permissions or starts native capture.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {makeDemo} from '../apps/desktop/src/demoScenarios.ts';
import {macPermissionCapabilities} from '../apps/desktop/src/permissionState.ts';
const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
const baseline=process.argv.includes('--baseline');
const root=path.resolve(baseline?'output/permission-capabilities-2026-10-20/baseline/dist':'apps/desktop/dist');
const out=path.resolve('output/playwright/permission-capabilities-2026-10-20');
fs.mkdirSync(out,{recursive:true});
const server=createServer((req,res)=>{
  const file=path.resolve(root,new URL(req.url,'http://localhost').pathname==='/'?'index.html':'.'+new URL(req.url,'http://localhost').pathname);
  if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}
  res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');
  res.end(fs.readFileSync(file));
});
await new Promise(r=>server.listen(0,'127.0.0.1',r));
const browser=await chromium.launch({channel:'msedge',headless:true});
const errors=[];
try {
  for(const [theme,width,height] of [['light',940,640],['dark',1200,820]]) {
    const context=await browser.newContext({viewport:{width,height}});
    const page=await context.newPage();
    page.on('pageerror',e=>errors.push(e.message));
    const state=makeDemo('paused','macos');
    state.input_permissions={accessibility:baseline,input_monitoring:false,post_event:false};
    state.topology[0].device.permission='Denied';
    state.config.appearance={dark:theme==='dark'};
    state.last_error=baseline?'source not ready':null;
    const commands=[];let failOpen=false;
    await page.exposeFunction('permissionFixture',async(command,args)=>{
      if(command==='daemon_status')return structuredClone(state);
      if(command==='daemon_command'){
        const m=args.message;commands.push(m);
        assert.ok(['open_permission','refresh','resume'].includes(m.command));
        setTimeout(()=>{
          const error=m.command==='open_permission'&&failOpen?'无法打开系统设置，请手动进入系统设置 → 隐私与安全性':null;
          state.last_command={id:m.request_id,error};
          state.last_error=error;
          if(m.command==='resume'){
            assert.ok(macPermissionCapabilities(state.input_permissions).ready);
            state.enabled=true;state.config.sharing=true;
          }
        },100);
        return;
      }
      throw Error(`Unexpected fixture command: ${command}`);
    });
    await page.addInitScript(({theme})=>{
      Object.defineProperty(navigator,'platform',{value:'MacIntel'});
      localStorage.setItem('crossdesk.ui.theme',theme);
      globalThis.isTauri=true;
      globalThis.__TAURI_INTERNALS__={invoke:async(c,a)=>{
        const value=await globalThis.permissionFixture(c,a);
        if(c==='daemon_status')globalThis.__permissionSeen=JSON.stringify(value.input_permissions);
        return value;
      }};
    },{theme});
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    await page.getByRole('button',{name:'启用共享',exact:true}).waitFor();
    await page.evaluate(()=>{
      const note=document.createElement('div');
      note.textContent='Mac 权限测试样例 · Windows 浏览器 · 未申请系统权限';
      note.style='position:fixed;bottom:5px;left:90px;z-index:9999;color:#805b15;background:#fff6df;padding:3px 6px;font-size:12px';
      document.body.append(note);
    });
    await page.screenshot({path:path.join(out,`${baseline?'before':'after'}-workspace-${width}-${theme}-FIXTURE.png`)});
    if(baseline){
      await page.getByRole('button',{name:'启用共享',exact:true}).click();
      await page.getByRole('dialog',{name:'Mac 键鼠权限'}).waitFor();
      await page.screenshot({path:path.join(out,`before-accessibility-only-${width}-${theme}-FIXTURE.png`)});
      await context.close();continue;
    }
    assert.equal(commands.length,0,'polling never asks for permission');
    await page.getByRole('button',{name:'启用共享',exact:true}).click();
    const dialog=page.getByRole('dialog',{name:'Mac 键鼠权限'});
    await dialog.waitFor();
    assert.equal(commands.length,0,'enable opens guidance without enabling capture');
    assert.equal(await dialog.getByRole('button',{name:'启用共享',exact:true}).isDisabled(),true);
    await page.screenshot({path:path.join(out,`missing-${width}-${theme}-FIXTURE.png`)});
    await dialog.getByRole('button',{name:'打开输入监控',exact:true}).click();
    await dialog.getByRole('button',{name:'打开输入监控',exact:true}).waitFor();
    assert.deepEqual(commands.map(m=>[m.command,m.kind]),[['open_permission','input_monitoring']]);
    assert.equal(state.enabled,false,'opening settings is not permission granted');
    state.input_permissions.post_event=true;
    await dialog.getByText('可用',{exact:true}).waitFor();
    assert.equal(await dialog.getByRole('button',{name:'启用共享',exact:true}).isDisabled(),true);
    failOpen=true;
    await dialog.getByRole('button',{name:'打开辅助功能',exact:true}).click();
    await dialog.getByRole('alert').getByText('无法打开系统设置',{exact:false}).waitFor();
    assert.ok(await dialog.evaluate(el=>{
      const button=el.querySelector('.dialog-actions button').getBoundingClientRect();
      const box=el.getBoundingClientRect();
      return button.bottom<box.bottom&&button.top>box.top;
    }),'permission recovery controls stay visible at minimum height');
    await page.screenshot({path:path.join(out,`open-failed-${width}-${theme}-FIXTURE.png`)});
    failOpen=false;
    await dialog.getByRole('button',{name:'重新检测',exact:true}).click();
    await dialog.getByRole('button',{name:'重新检测',exact:true}).waitFor();
    await page.keyboard.press('Escape');
    assert.equal(await dialog.isVisible(),false);
    assert.equal(await page.getByRole('button',{name:'启用共享',exact:true}).evaluate(el=>el===document.activeElement),true);
    await page.getByRole('button',{name:'设置权限',exact:true}).click();
    // Real permission overlap, not "all booleans true": test every combination.
    for(const accessibility of [false,true]){
      for(const input_monitoring of [false,true]){
        for(const post_event of [false,true]){
          state.input_permissions={accessibility,input_monitoring,post_event};
          const expected=accessibility||(input_monitoring&&post_event);
          assert.equal(macPermissionCapabilities(state.input_permissions).ready,expected);
          state.topology[0].device.permission=expected?'Granted':'Denied';
          await page.waitForFunction(value=>globalThis.__permissionSeen===value,JSON.stringify(state.input_permissions));
          await page.waitForFunction(expected=>document.querySelector('dialog[open] .dialog-actions .button:last-child')?.disabled===!expected,expected);
          assert.equal(state.enabled,false,'permission result alone must not start sharing');
        }
      }
    }
    state.input_permissions={accessibility:true,input_monitoring:false,post_event:false};
    await dialog.getByText('辅助功能已包含，无需重复开启',{exact:true}).waitFor();
    assert.equal(state.enabled,false);
    assert.equal(await page.locator('.global-error').count(),0,'resolved permission error must not remain as a red banner');
    await page.screenshot({path:path.join(out,`after-accessibility-only-${width}-${theme}-FIXTURE.png`)});
    // Exact raw permissions from the user's rc22 diagnostics after TCC reset.
    state.input_permissions={accessibility:false,input_monitoring:true,post_event:true};
    await page.waitForFunction(value=>globalThis.__permissionSeen===value,JSON.stringify(state.input_permissions));
    await page.waitForFunction(()=>document.querySelector('dialog[open] .dialog-actions .button:last-child')?.disabled===false);
    assert.equal(await dialog.getByText('可用',{exact:true}).count(),2);
    assert.equal(await dialog.getByText('后台暂未取得',{exact:true}).count(),0);
    assert.equal(commands.filter(m=>m.command==='resume').length,0);
    assert.equal(state.enabled,false,'both Quartz grants do not auto-enable sharing');
    await page.screenshot({path:path.join(out,`after-quartz-grants-${width}-${theme}-FIXTURE.png`)});
    await page.keyboard.press('Tab');
    assert.ok(await dialog.evaluate(el=>el.contains(document.activeElement)));
    await dialog.getByRole('button',{name:'启用共享',exact:true}).click();
    await page.getByRole('button',{name:'暂停共享',exact:true}).waitFor();
    assert.equal(commands.filter(m=>m.command==='resume').length,1);
    assert.equal(await page.locator('.permission-reminder').count(),0);
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
    await context.close();
  }
  assert.deepEqual(errors,[]);
  console.log(baseline?'CAPTURE rc22 incorrectly blocks an Accessibility-only grant; two sizes/themes, fixtures only':'PASS all 8 permission combinations, user false/true/true grants enable with acknowledgement, Accessibility includes listen/post, partial grants remain blocked, explicit requests only, no fake grant, open failure, refresh, Escape/focus; light 940 / dark 1200 fixtures');
} finally {await browser.close();await new Promise(r=>server.close(r));}
