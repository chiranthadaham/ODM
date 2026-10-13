const fs=require('fs'),path=require('path'),vm=require('vm'),assert=require('assert'),crypto=require('crypto');
const {fixture}=require('./test_detection.cjs');
(async()=>{
 for(const [edition,root,enabled] of [['local',process.argv[2],true],['store',process.argv[3],false]]){
  const manifest=JSON.parse(fs.readFileSync(path.join(root,'manifest.json'),'utf8'));
  const f=fixture(path.join(root,'core.js'));vm.runInContext(fs.readFileSync(path.join(root,'edition.js'),'utf8'),f.ctx);
  await vm.runInContext('captureReady',f.ctx);
  for(const url of ['https://www.youtube.com/watch?v=3PbdkXLnK74','https://www.youtube.com/shorts/test','https://youtu.be/test','https://rr.googlevideo.com/video.mp4']){
   assert.equal(f.ctx.supported(url),enabled);
   const before=f.native;const reply=await f.listeners.message[0]({action:'video-download',url},{tab:{id:1,url}});
   assert.equal(reply.ok,enabled);assert.equal(f.native,before+(enabled?1:0));
  }
  for(const url of ['https://www.youtube.com/watch?v=test','https://www.youtube.com/shorts/test','https://youtu.be/test']){
   const ctx=vm.createContext({location:new URL(url),Array});
   vm.runInContext(fs.readFileSync(path.join(root,'edition.js'),'utf8'),ctx);
   // Multiple matching content-script groups may load this file in one frame.
   vm.runInContext(fs.readFileSync(path.join(root,'edition.js'),'utf8'),ctx);
   const video=fs.readFileSync(path.join(root,'video-button.js'),'utf8');vm.runInContext(video.slice(0,video.indexOf('function rememberVideo')),ctx);
   assert.equal(vm.runInContext('videoSource({currentSrc:"blob:https://www.youtube.com/example"})',ctx),enabled?url:'');
  }
  if(enabled){
   assert(manifest.content_scripts.every(s=>!s.exclude_matches));assert(manifest.key);
   const hex=crypto.createHash('sha256').update(Buffer.from(manifest.key,'base64')).digest('hex').slice(0,32);
   const id=[...hex].map(c=>String.fromCharCode(97+parseInt(c,16))).join('');assert.equal(id,'jancefngmifbnaldahilleoinncjppjh');
  }else{
   assert(!manifest.key);assert(manifest.content_scripts.every(s=>s.exclude_matches.includes('*://*.youtube.com/*')));
  }
  console.log('PASS: '+edition+' package: YouTube page detection/handoff '+(enabled?'enabled':'disabled')+', manifest and native-host identity checked');
 }
})().catch(e=>{console.error(e);process.exitCode=1});
