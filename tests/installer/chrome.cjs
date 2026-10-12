const {chromium}=require('playwright');
const fs=require('fs'),path=require('path'),assert=require('assert');
(async()=>{const cfg=JSON.parse(fs.readFileSync(process.argv[2]));const ctx=await chromium.launchPersistentContext(cfg.profile,{executablePath:process.env.ODM_CHROME||'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,acceptDownloads:true,ignoreDefaultArgs:['--disable-extensions'],args:['--enable-unsafe-extension-debugging']});
try{const errors=[];const cdp=await ctx.browser().newBrowserCDPSession();const {id}=await cdp.send('Extensions.loadUnpacked',{path:cfg.extension});assert.equal(id,'jancefngmifbnaldahilleoinncjppjh');
await cdp.send('Browser.setDownloadBehavior',{behavior:'default',eventsEnabled:true});
const popup=await ctx.newPage();popup.on('pageerror',e=>errors.push(e.message));await popup.goto(`chrome-extension://${id}/popup.html`);await popup.waitForFunction(()=>document.querySelector('#send').disabled);assert(await popup.locator('#capture').isDisabled());
const consent=await ctx.newPage();await consent.goto(`chrome-extension://${id}/consent.html`);await consent.locator('#accept').click();await consent.locator('#result').filter({hasText:'Enabled'}).waitFor();await consent.screenshot({path:path.join(cfg.profile,'consent.png')});
await popup.reload();await popup.waitForFunction(()=>!document.querySelector('#send').disabled);await popup.locator('#check').click();await popup.waitForFunction(()=>document.querySelector('#check').textContent==='App connected');
const page=await ctx.newPage();await page.goto(cfg.url+'/page');const worker=ctx.serviceWorkers()[0];
for(const name of ['chrome.zip','chrome.pdf','chrome.future']){
const event=page.waitForEvent('download');await page.locator(`[href="/${name}"]`).click();const download=await event;const fail=await download.failure();console.log('CAPTURE',name,fail,await worker.evaluate(async()=>({status:lastStatus,consent:consentAccepted,capture:capturePreference,items:await chrome.downloads.search({})})));assert(fail,'Browser must not duplicate accepted download');await new Promise(r=>setTimeout(r,100));
let end=Date.now()+15000;while(!fs.existsSync(path.join(cfg.downloads,name))&&Date.now()<end)await new Promise(r=>setTimeout(r,100));assert(fs.existsSync(path.join(cfg.downloads,name)),name);
}
const blocked=await worker.evaluate(async()=>{try{await sendDownload('https://www.youtube.com/watch?v=test');return false;}catch(e){return e.message.includes('unavailable');}});assert(blocked);
await consent.locator('#decline').click();await popup.reload();await popup.waitForFunction(()=>document.querySelector('#send').disabled);assert.deepEqual(errors,[]);
console.log(JSON.stringify({browser:ctx.browser().version(),id,nativePing:true,consentRequired:true,revocation:true,youtubeBlocked:true,captured:['zip','pdf','future'],popupErrors:errors}));
}finally{await ctx.close();}})().catch(e=>{console.error(e);process.exitCode=1});
