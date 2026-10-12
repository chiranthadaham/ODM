const fs=require('fs'),vm=require('vm'),assert=require('assert'),path=require('path');
function fixture(file,initial=true,consent=1){
 let reads=0,value=initial,native=0;const listeners={};
 const event=name=>({addListener(fn){(listeners[name]??=[]).push(fn);}});
 const browser={tabs:{create:async()=>{}},storage:{local:{get:async()=>{reads++;const capture=value;await new Promise(r=>setTimeout(r,10));return {capture,consentVersion:consent};},set:async v=>{value=v.capture;for(const fn of listeners.storage||[])fn({capture:{newValue:value}},'local');}},onChanged:event('storage')},downloads:{onCreated:event('created'),onChanged:event('changed'),onDeterminingFilename:event('filename')},webRequest:{onBeforeSendHeaders:event('headers'),onHeadersReceived:event('response')},contextMenus:{create(){},onClicked:event('context')},browserAction:{setBadgeText:async()=>{},setBadgeBackgroundColor:async()=>{}},runtime:{getURL:p=>p,onInstalled:event('installed'),onMessage:event('message'),sendNativeMessage:async()=>{native++;return {ok:true,queued:true};}}};
 const ctx=vm.createContext({browser,DOWNLOAD_FORMATS:['zip','exe'],URL,console,setTimeout,clearTimeout});vm.runInContext(fs.readFileSync(file,'utf8'),ctx);
 return {ctx,listeners,get reads(){return reads},get native(){return native},get value(){return value}};
}
(async()=>{
 const results=[];
 for(const [browser,file]of [['Firefox','background.js'],['Chromium','core.js']]){
  const root=path.resolve(__dirname,'../browser',browser,'extension');const f=fixture(path.join(root,file));
  await vm.runInContext('captureReady',f.ctx);
  const started=performance.now();
  for(let n=0;n<40;n++)assert((await f.ctx.captureHeaders({url:`https://example.com/file-${n}.zip`,method:'GET',statusCode:200,type:'main_frame',tabId:1,responseHeaders:[{name:'Content-Type',value:'application/zip'}]})).cancel);
  assert.equal(f.reads,1);assert.equal(f.native,40);
  for(const fn of f.listeners.storage)fn({capture:{newValue:false}},'local');assert.equal(await f.ctx.captureEnabled(),false);
  for(const fn of f.listeners.installed)fn({reason:'update'});assert.equal(await f.ctx.captureEnabled(),false);
  await f.listeners.message[0]({action:'capture',enabled:true},{});assert.equal(await f.ctx.captureEnabled(),true);
  assert.equal((await f.ctx.captureHeaders({url:'https://example.com/private.zip',method:'GET',statusCode:200,incognito:true})).cancel,undefined);
  const race=fixture(path.join(root,file),true);
  for(const fn of race.listeners.storage)fn({capture:{newValue:false}},'local');await vm.runInContext('captureReady',race.ctx);assert.equal(await race.ctx.captureEnabled(),false);
  const denied=fixture(path.join(root,file),true,0);await vm.runInContext('captureReady',denied.ctx);
  assert.equal(await denied.ctx.captureEnabled(),false);
  assert.equal((await denied.listeners.message[0]({action:'download',url:'https://example.com/file.zip'},{})).ok,false);
  assert.equal(denied.native,0);
  assert.equal((await f.listeners.message[0]({action:'download',url:'https://example.com/file.zip'},{tab:{incognito:true}})).ok,false);
  if(browser==='Chromium')for(const url of ['https://www.youtube.com/watch?v=abc','https://rr.googlevideo.com/video.mp4','https://youtu.be/abc']){
   const before=f.native;assert.equal((await f.listeners.message[0]({action:'download',url},{})).ok,false);assert.equal(f.native,before);
  }
  for(const fn of f.listeners.storage)fn({consentVersion:{newValue:0}},'local');assert.equal(await f.ctx.captureEnabled(),false);
  results.push({browser,passed:true,metadataEvents:40,storageReads:f.reads,seconds:(performance.now()-started)/1000});
 }
 console.log(JSON.stringify(results,null,2));if(process.argv[2])fs.writeFileSync(process.argv[2],JSON.stringify(results,null,2));
})().catch(e=>{console.error(e);process.exitCode=1});

