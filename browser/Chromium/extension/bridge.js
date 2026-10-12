"use strict";
function nativeRequest(host,message){
 return new Promise((resolve,reject)=>{
  const port=chrome.runtime.connectNative(host);let settled=false;
  const timer=setTimeout(()=>{if(!settled){settled=true;port.disconnect();reject(new Error("The download prompt timed out."));}},600000);
  port.onMessage.addListener(reply=>{if(settled)return;settled=true;clearTimeout(timer);resolve(reply);port.disconnect();});
  port.onDisconnect.addListener(()=>{if(settled)return;settled=true;clearTimeout(timer);reject(new Error(chrome.runtime.lastError?.message||"The app connection closed."));});
  port.postMessage(message);
 });
}
const browser={tabs:chrome.tabs,storage:chrome.storage,downloads:chrome.downloads,webRequest:chrome.webRequest,contextMenus:chrome.contextMenus,browserAction:chrome.action,
 runtime:{getURL:chrome.runtime.getURL,onInstalled:chrome.runtime.onInstalled,sendNativeMessage:nativeRequest,onMessage:{addListener(handler){chrome.runtime.onMessage.addListener((message,sender,respond)=>{Promise.resolve(handler(message,sender)).then(respond,error=>respond({ok:false,error:error.message}));return true;});}}}};

