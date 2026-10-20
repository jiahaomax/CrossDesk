// Read-only UI connected to isolated observe-only daemons from the layout test.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';

export async function captureThreeLayout(a,b,c) {
  const {chromium}=createRequire(import.meta.url)(process.env.PLAYWRIGHT_MODULE||'playwright');
  const root=path.resolve('apps/desktop/dist');
  const out=path.resolve('output/playwright/layout-sync-2026-10-20');
  fs.mkdirSync(out,{recursive:true});
  const server=createServer((req,res)=>{
    const file=path.resolve(root,new URL(req.url,'http://localhost').pathname.slice(1)||'index.html');
    if(!file.startsWith(root+path.sep)||!fs.existsSync(file)){res.writeHead(404).end();return;}
    res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':file.endsWith('.css')?'text/css':'text/html');
    res.end(fs.readFileSync(file));
  });
  await new Promise(r=>server.listen(0,'127.0.0.1',r));
  const browser=await chromium.launch({channel:'msedge',headless:true});
  const errors=[];
  try {
    for(const [d,width,height] of [[a,940,640],[a,1200,820],[b,940,640]]) {
      const context=await browser.newContext({viewport:{width,height}});
      const page=await context.newPage();
      page.on('pageerror',e=>errors.push(e.message));
      await page.exposeFunction('layoutTestInvoke',async command=>{
        assert.equal(command,'daemon_status','read-only browser; no commands allowed');
        assert.equal(d.status.observe_only,true);
        return structuredClone(d.status);
      });
      await page.addInitScript(()=>{
        globalThis.isTauri=true;
        globalThis.__TAURI_INTERNALS__={invoke:(c)=>globalThis.layoutTestInvoke(c)};
      });
      await page.goto(`http://127.0.0.1:${server.address().port}`);
      await page.locator('.screen-target').first().waitFor();
      await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
      const position=async device=>{
        const box=await page.locator(`[data-device-group="${device.status.identity}"] .monitor`).first().boundingBox();
        assert.ok(box);return box.x+box.width/2;
      };
      assert.ok(await position(b)<await position(a),'both ends draw new computer to left of hub');
      if(d===a)assert.ok(await position(a)<await position(c),'previous computer stays to right');
      assert.equal(await page.locator('.global-error').count(),0);
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
      await page.evaluate(()=>{
        const label=document.createElement('div');label.textContent='三份隔离后台 · 非实体三机 · 未捕获输入';
        label.style='position:fixed;top:19px;left:50%;transform:translateX(-50%);font-size:12px;padding:2px 7px;background:#fff6df;color:#805b15;z-index:99';
        document.body.append(label);
      });
      await page.screenshot({path:path.join(out,`${d===a?'hub':'new-peer'}-${width}-OBSERVE-ONLY.png`)});
      await context.close();
    }
    assert.deepEqual(errors,[]);
    console.log('PASS 940/1200 rendering: third computer placed consistently on both ends; original placement preserved, no error/overflow');
  } finally {
    await browser.close();await new Promise(r=>server.close(r));
  }
}
