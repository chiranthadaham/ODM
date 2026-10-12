"use strict";
const HOST = "com.opendownloadmanager";
const FILE_TYPES = new RegExp("\\.(?:"+DOWNLOAD_FORMATS.join("|")+")(?:$|[?#])", "i");
let consentAccepted = false;
let consentChanged = false;
let capturePreference = false;
let captureChanged = false;
const captureReady = browser.storage.local.get({capture:true,consentVersion:0}).then(value => {
  if (!consentChanged) consentAccepted = value.consentVersion === 1;
  if (!captureChanged) capturePreference = value.capture !== false;
}).catch(() => {});
if (browser.storage.onChanged) browser.storage.onChanged.addListener((changes,area) => {
  if (area === "local" && changes.consentVersion) { consentChanged = true; consentAccepted = changes.consentVersion.newValue === 1; if (!consentAccepted) for (const cache of [requestContexts,detectedVideos,videoCandidates,fetchedFiles]) cache.clear(); }
  if (area === "local" && changes.capture) { captureChanged = true; capturePreference = changes.capture.newValue !== false; }
});
async function captureEnabled() { await captureReady; return consentAccepted && capturePreference; }
const pending = new Set();
const detectedVideos=new Map();
const videoCandidates=new Map();
const browserManaged = new Map();
const fetchedFiles = new Map();
function responseFilename(headers) {
  const value=(headers||[]).find(h=>h.name.toLowerCase()==="content-disposition")?.value||"";
  const encoded=/filename\*\s*=\s*UTF-8''([^;]+)/i.exec(value);
  if(encoded){try{return decodeURIComponent(encoded[1].trim());}catch{}}
  const plain=/filename\s*=\s*(?:"([^"]+)"|([^;]+))/i.exec(value);
  return plain?(plain[1]||plain[2]).trim():"";
}
function frameKey(sender){return String(sender?.tab?.id)+":"+String(sender?.frameId||0);}
let lastStatus = "Ready. Use the capture switch to choose where downloads go.";
function fileSample(url){try{const u=new URL(url);return ["filesamples.com","www.filesamples.com"].includes(u.hostname.toLowerCase())&&/^\/samples\/[^/]+\/[^/]+\/[^/]+/.test(u.pathname);}catch{return false;}}
function supported(url) { try { return ["http:", "https:"].includes(new URL(url).protocol); } catch { return false; } }
const requestContexts=new Map();
if(browser.webRequest.onBeforeSendHeaders)browser.webRequest.onBeforeSendHeaders.addListener(details=>{
 if(details.incognito||!consentAccepted)return;const get=name=>(details.requestHeaders||[]).find(h=>h.name.toLowerCase()===name)?.value||"";let referer=get("referer")||details.documentUrl||details.originUrl||details.initiator||"";
 try{const source=new URL(referer);referer=["http:","https:"].includes(source.protocol)?source.origin+"/":"";}catch{referer="";}
 if(requestContexts.size>=500)requestContexts.clear();requestContexts.set(details.url,{referer,time:Date.now()});
},{urls:["http://*/*","https://*/*"]},["requestHeaders"]);

async function sendDownload(url,filename="",sourcePage="",identifiedFile=false) {
  await captureReady;
  if (!consentAccepted) throw new Error("Review privacy and permissions before enabling downloads.");
  if (!supported(url)) throw new Error("Choose an HTTP or HTTPS link.");
  const context=requestContexts.get(url);const details=context&&Date.now()-context.time<120000?{...context}:{};if(!details.referer&&supported(sourcePage)){details.referer=new URL(sourcePage).origin+"/";}
  const reply = await browser.runtime.sendNativeMessage(HOST, {action:"download", url, filename, verifyAccess:!identifiedFile,userAgent:"",referer:details.referer||""});
  if (!reply || !reply.ok || !reply.queued) {const error=new Error(reply?.error || "The app did not accept this download.");error.canceled=!!reply?.canceled;throw error;}
  lastStatus = "Sent to Open Download Manager.";
  await browser.browserAction.setBadgeText({text:""});
  return reply;
}
async function report(error) {
  lastStatus = error.message || String(error);
  await browser.browserAction.setBadgeText({text:"!"});
  await browser.browserAction.setBadgeBackgroundColor({color:"#c2410c"});
}
async function capture(item) {
  if (pending.has(item.id) || item.incognito || item.state !== "in_progress" || !supported(item.url)) return;
  if (!(await captureEnabled())) return;
  pending.add(item.id);let paused=false;
  try {
    // A real browser download is already identified, regardless of extension,
    // length, or range support. Do not wait for its body or require canResume.
    if ((browserManaged.get(item.url)||0)>Date.now()) return;
    try { await browser.downloads.pause(item.id); paused=true; } catch {}
    await sendDownload(item.finalUrl||item.url,(item.filename||"").split(/[\\/]/).pop(),item.referrer||"",true);
    await browser.downloads.cancel(item.id);
    lastStatus="Captured a file download. It is now in the app's queue.";
  } catch(error) {
    if(error.canceled){await browser.downloads.cancel(item.id);lastStatus="Download canceled.";return;}
    if (paused) {try { await browser.downloads.resume(item.id); } catch { /* Already complete or canceled by the user. */ }}
    await report(new Error("Firefox kept this download. " + (error.message || error)));
  } finally {pending.delete(item.id);}
}
browser.downloads.onCreated.addListener(capture);
// Firefox can reject a response before the header-capture hook sees a file.
// Offer the normal validated app prompt once for transport/server failures only.
const recoverableFailures=new Set(["SERVER_FAILED","SERVER_FORBIDDEN","NETWORK_FAILED","NETWORK_TIMEOUT","NETWORK_DISCONNECTED","NETWORK_SERVER_DOWN"]);
const recovering=new Set();
async function recoverFailedDownload(id) {
  if(recovering.has(id))return;recovering.add(id);
  try {
    for(let n=0;n<40&&pending.has(id);n++)await new Promise(resolve=>setTimeout(resolve,50));
    if(pending.has(id))return;
    const item=(await browser.downloads.search({id}))[0];
    if(!item||item.incognito||item.state!=="interrupted"||!recoverableFailures.has(item.error)||!supported(item.url)||(browserManaged.get(item.url)||0)>Date.now())return;
    if(!(await captureEnabled()))return;
    // Suppress duplicate failure events for this attempt; a later manual retry is allowed.
    browserManaged.set(item.url,Date.now()+60000);
    await sendDownload(item.finalUrl||item.url,(item.filename||"").split(/[\\/]/).pop(),item.referrer||"");
    lastStatus="Recovered a failed Firefox request in Open Download Manager.";
  } catch(error) {if(error.canceled){lastStatus="Download canceled.";return;}await report(new Error("Firefox could not download this file, and the app could not accept it. "+error.message));}
  finally {recovering.delete(id);}
}
browser.downloads.onChanged.addListener(delta=>{
  if(delta.state?.current==="interrupted"||recoverableFailures.has(delta.error?.current))return recoverFailedDownload(delta.id);
});
browser.contextMenus.create({id:"odm-link",title:"Download link with Open Download Manager",contexts:["link"]});
browser.contextMenus.create({id:"odm-page",title:"Download this video page with Open Download Manager",contexts:["page"]});
browser.contextMenus.onClicked.addListener(async(info,tab)=>{
  try {await sendDownload(info.menuItemId==="odm-link"?info.linkUrl:(info.pageUrl||tab.url));}
  catch(error){await report(error);}
});
browser.runtime.onMessage.addListener(async(message,sender)=>{
  await captureReady;
  if(sender?.tab?.incognito)return {ok:false,error:"Private-window downloads stay in the browser."};
  if(!consentAccepted && !["status","capture"].includes(message.action))return {ok:false,error:"Review privacy and permissions before enabling downloads."};
  if(message.action==="detected-videos"){const prefix=String(sender?.tab?.id)+":";const unique=new Map();for(const [key,entries] of videoCandidates)if(key.startsWith(prefix))for(const video of entries)if(!unique.has(video.url)||unique.get(video.url).time<video.time)unique.set(video.url,video);return {videos:[...unique.values()].sort((a,b)=>b.time-a.time)};}
  if(message.action==="resolve-blob-download"){
    if(!(await captureEnabled()))return {ok:false};
    const name=String(message.filename||"").toLowerCase();
    // Exact filenames and the requesting frame prevent unrelated fetches from being substituted.
    const matches=(fetchedFiles.get(frameKey(sender))||[]).filter(f=>Date.now()-f.time<120000&&f.filename.toLowerCase()===name);
    const urls=[...new Set(matches.map(f=>f.url))];
    return urls.length===1?{ok:true,url:urls[0],filename:matches.at(-1).filename}:{ok:false};
  }
  if(message.action==="video-download"){const key=String(sender?.tab?.id)+":"+String(sender?.frameId||0);const found=detectedVideos.get(key);const url=supported(message.url)?message.url:(found&&Date.now()-found.time<600000?found.url:"");if(!url)return {ok:false,error:"This player exposes no downloadable HTTP video. Protected or browser-only streams may be unavailable."};try{return await sendDownload(url);}catch(error){return {ok:false,canceled:!!error.canceled,error:error.message};}}
  if(message.action==="status")return {lastStatus,capture:consentAccepted&&capturePreference,consent:consentAccepted};
  if(message.action==="capture"){if(!consentAccepted)return {ok:false,error:"Accept permissions first."};await browser.storage.local.set({capture:!!message.enabled});captureChanged=true;capturePreference=!!message.enabled;return {ok:true};}
  if(message.action==="ping"){try{return await browser.runtime.sendNativeMessage(HOST,{action:"ping"});}catch(error){return {ok:false,error:error.message};}}
  if(message.action==="download"){try{return await sendDownload(message.url,message.filename||"",sender?.tab?.url||"",message.identifiedFile===true);}catch(error){await report(error);return {ok:false,canceled:!!error.canceled,error:lastStatus};}}
  return {ok:false,error:"Unknown action"};
});

// Capture before the response body arrives, so small files do not finish first.
// Page resources and HTML verification pages are never intercepted.
async function captureHeaders(details) {
  if (details.incognito || details.method !== "GET" || ![200,206].includes(details.statusCode) || !supported(details.url)) return {};
  const headers=details.responseHeaders||[];
  const get=name=>headers.find(h=>h.name.toLowerCase()===name)?.value||"";
  const type=get("content-type").toLowerCase();
  if(details.type==="main_frame"&&type.includes("text/html")){for(const cache of [detectedVideos,fetchedFiles,videoCandidates])for(const key of cache.keys())if(key.startsWith(String(details.tabId)+":"))cache.delete(key);}
  const attachment=/^attachment(?:;|$)/i.test(get("content-disposition"));
  // Mirror landing pages can carry a declarative redirect to the actual file.
  // The native host must validate that redirect before Firefox is canceled.
  const mirrorFile = FILE_TYPES.test(details.url.replace(/\/download(?:[?#].*)?$/i,"")) && !/\.(?:html?|xhtml)(?:$|[?#])/i.test(details.url);
  if ((type.includes("text/html") || type.includes("application/xhtml")) && !attachment && !mirrorFile && !(fileSample(details.url)&&/\.(?:html?|xhtml)(?:$|[?#])/i.test(details.url))) return {};
  if (get("cf-mitigated").toLowerCase()==="challenge") return {};
  // File responses need no extension allowlist. Listener types exclude page assets and XHR.
  if (!attachment && !fileSample(details.url) && !mirrorFile && !FILE_TYPES.test(details.url) && (!type || type.includes("text/html"))) return {};
  if (!(await captureEnabled())) return {};
  for (const [url,until] of browserManaged) if(until<Date.now())browserManaged.delete(url);
  browserManaged.set(details.url,Date.now()+60000);
  try {
    const disposition=get("content-disposition");let filename="";
    const encoded=/filename\*\s*=\s*UTF-8''([^;]+)/i.exec(disposition);
    const plain=/filename\s*=\s*(?:"([^"]+)"|([^;]+))/i.exec(disposition);
    if(encoded){try{filename=decodeURIComponent(encoded[1].trim());}catch{}}
    if(!filename&&plain)filename=(plain[1]||plain[2]).trim();
    await sendDownload(details.url,filename,"",true);
    lastStatus="Captured before Firefox saved the file. It is now in the app's queue.";
    return {cancel:true};
  } catch(error) {if(error.canceled){lastStatus="Download canceled.";return {cancel:true};}await report(new Error("Firefox kept this download. "+error.message));return {};}
}
browser.webRequest.onHeadersReceived.addListener(captureHeaders,
  {urls:["http://*/*","https://*/*"],types:["main_frame","sub_frame","other"]},
  ["blocking","responseHeaders"]);

// Watch only response metadata to offer an explicit video button for blob-backed players.
browser.webRequest.onHeadersReceived.addListener(details=>{
 if(!consentAccepted||details.incognito||![200,206].includes(details.statusCode)||!supported(details.url)||details.tabId<0)return;
 const type=(details.responseHeaders||[]).find(h=>h.name.toLowerCase()==="content-type")?.value||"";
 if(details.type==="main_frame"&&/text\/html/i.test(type))for(const cache of [detectedVideos,fetchedFiles,videoCandidates])for(const key of cache.keys())if(key.startsWith(String(details.tabId)+":"))cache.delete(key);
 if(details.type==="xmlhttprequest"&&details.method==="GET"&&supported(details.url)){
  const filename=responseFilename(details.responseHeaders);
  if(filename&&!/text\/html|application\/(?:xhtml|json)/i.test(type)){
   const key=String(details.tabId)+":"+String(details.frameId||0);
   if(fetchedFiles.size>500)fetchedFiles.clear();
   const recent=(fetchedFiles.get(key)||[]).filter(f=>Date.now()-f.time<120000&&f.url!==details.url);
   fetchedFiles.set(key,[...recent,{url:details.url,filename,time:Date.now()}].slice(-20));
  }
 }
 const manifest=/\.(?:m3u8|mpd)(?:$|[?#])/i.test(details.url)||/mpegurl|dash\+xml/i.test(type);
 const segment=/\.(?:ts|m4s|cmfv|cmfa)(?:$|[?#])/i.test(details.url)||/mp2t/i.test(type);
 if(manifest||(/^video\//i.test(type)&&!segment)){
  if(detectedVideos.size>500)detectedVideos.clear();const key=String(details.tabId)+":"+String(details.frameId||0),old=detectedVideos.get(key);
  let title="Video stream";try{const u=new URL(details.url);title=decodeURIComponent(u.pathname.split("/").pop())||u.hostname;}catch{}
  const entries=videoCandidates.get(key)||[];videoCandidates.set(key,[...entries.filter(v=>v.url!==details.url),{url:details.url,title,time:Date.now(),manifest}].slice(-500));
  if(manifest||!old?.manifest||Date.now()-old.time>600000)detectedVideos.set(key,{url:details.url,time:Date.now(),manifest});
 }
},{urls:["http://*/*","https://*/*"],types:["media","xmlhttprequest","main_frame"]},["responseHeaders"]);
if(browser.runtime.onInstalled)browser.runtime.onInstalled.addListener(async()=>{await captureReady;if(!consentAccepted)browser.tabs.create({url:browser.runtime.getURL("consent.html"),active:true});});
