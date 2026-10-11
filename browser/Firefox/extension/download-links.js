"use strict";
// Sites may fetch a file and click an invisible blob-backed download link.
// Resolve only an exact filename from this frame's recent attachment responses.
(()=>{
 let enabled=true;
 const bypass=new WeakSet();
 browser.storage.local.get({capture:true}).then(s=>enabled=s.capture);
 browser.storage.onChanged.addListener((changes,area)=>{if(area==="local"&&changes.capture)enabled=changes.capture.newValue;});
 function failure(message,url){
  const box=document.createElement("div");box.setAttribute("role","alert");box.style.cssText="position:fixed;bottom:20px;right:20px;z-index:2147483647;max-width:420px;padding:18px;background:white;color:#1e293b;border:1px solid #cbd5e1;border-radius:12px;box-shadow:0 4px 20px #0003;font:14px system-ui";
  const text=document.createElement("p");text.textContent=message;box.append(text);
  const link=document.createElement("a");link.href=url;link.textContent="Download in Firefox instead";link.addEventListener("click",event=>event.stopPropagation());box.append(link);
  const close=document.createElement("button");close.textContent="Close";close.style.marginLeft="18px";close.addEventListener("click",()=>box.remove());box.append(close);(document.body||document.documentElement).append(box);
 }
 document.addEventListener("click",async event=>{
  const anchor=event.composedPath().find(el=>el?.tagName==="A"&&el.hasAttribute("download"));
  if(!enabled||!anchor||bypass.has(anchor)||!anchor.href.startsWith("blob:")||!anchor.download)return;
  event.preventDefault();event.stopImmediatePropagation();
  let source;
  try{source=await browser.runtime.sendMessage({action:"resolve-blob-download",filename:anchor.download});}catch{}
  if(!source?.ok){bypass.add(anchor);try{anchor.click();}finally{bypass.delete(anchor);}return;}
  try{
   const reply=await browser.runtime.sendMessage({action:"download",url:source.url,filename:source.filename});
   if(!reply?.ok&&!reply?.canceled)failure(reply?.error||"The app could not accept this download.",source.url);
  }catch(error){failure(error.message,source.url);}
 },true);
})();
