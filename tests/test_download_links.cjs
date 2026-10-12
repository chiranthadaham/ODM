const fs=require('fs'),vm=require('vm'),assert=require('assert'),path=require('path');
(async()=>{for(const product of ['Firefox','Chromium']){
 const sent=[],handlers={};const browser={storage:{local:{get:async()=>({capture:true,consentVersion:1})},onChanged:{addListener(){}}},runtime:{sendMessage:async m=>{sent.push(m);return {ok:true};}}};
 const document={addEventListener:(name,fn)=>handlers[name]=fn};
 vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../browser',product,'extension/download-links.js'),'utf8'),{browser,document,WeakSet,console});
 await new Promise(r=>setTimeout(r,0));
 const anchor={tagName:'A',hasAttribute:n=>n==='download',href:'https://example.com/export?opaque=1',download:'file.unheardof'};
 let prevented=false;await handlers.click({composedPath:()=>[anchor],preventDefault:()=>prevented=true,stopImmediatePropagation(){}});
 assert(prevented);assert.equal(sent[0].action,'download');assert.equal(sent[0].identifiedFile,true);assert.equal(sent[0].url,anchor.href);
 console.log(product+': explicit HTTP download link captured without extension filter');
}})().catch(e=>{console.error(e);process.exitCode=1});
