"use strict";
// Capture explicit sample-file clicks, including formats that Firefox would display inline.
// Site navigation and page resources continue normally. No extension list restricts this route.
let enabled=true;
browser.storage.local.get({capture:true}).then(settings=>enabled=settings.capture);
browser.storage.onChanged.addListener((changes,area)=>{if(area==="local"&&changes.capture)enabled=changes.capture.newValue;});
function sampleLink(url){try{const u=new URL(url);return u.protocol==="https:"&&["filesamples.com","www.filesamples.com"].includes(u.hostname.toLowerCase())&&/^\/samples\/[^/]+\/[^/]+\/[^/]+/.test(u.pathname);}catch{return false;}}
function showFailure(message,url){
 const box=document.createElement("div");box.setAttribute("role","alert");box.style.cssText="position:fixed;bottom:20px;right:20px;z-index:2147483647;max-width:420px;padding:18px;background:#fff;color:#1e293b;border:1px solid #cbd5e1;border-radius:12px;box-shadow:0 4px 20px #0003;font:14px system-ui";
 const text=document.createElement("p");text.textContent=message;box.append(text);
 const link=document.createElement("a");link.href=url;link.textContent="Download in browser instead";link.style.cssText="color:#2563eb";
 link.addEventListener("click",event=>event.stopPropagation());box.append(link);
 const close=document.createElement("button");close.textContent="Close";close.style.marginLeft="18px";close.addEventListener("click",()=>box.remove());box.append(close);(document.body||document.documentElement).append(box);
}
async function handle(event){
 if(!enabled||!event.isTrusted||![0,1].includes(event.button))return;
 const target=event.target?.closest?.("a[href]");if(!target||target.closest('[role="alert"]')||!sampleLink(target.href))return;
 event.preventDefault();event.stopImmediatePropagation();
 try{const reply=await browser.runtime.sendMessage({action:"download",url:target.href});if(!reply?.ok&&!reply?.canceled)showFailure(reply?.error||"The app could not accept the download.",target.href);}
 catch(error){showFailure(error.message,target.href);}
}
document.addEventListener("click",handle,true);
document.addEventListener("auxclick",handle,true);
