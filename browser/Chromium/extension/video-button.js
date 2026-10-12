"use strict";
let videoButton=null,videoPanel=null,videoToggle=null,videoDock=null,dismissed=false,busy=false;
const pageVideos=new Map();
function youtubePage(){return /(^|\.)youtube\.com$|^youtu\.be$/i.test(location.hostname);}
function videoSource(video){
 if(youtubePage())return "";
 return [video?.currentSrc,video?.src,...Array.from(video?.querySelectorAll("source")||[],s=>s.src)].find(v=>/^https?:\/\//i.test(v||""))||"";
}
function rememberVideo(video,promote=false){
 const url=videoSource(video);if(!url)return;
 const old=pageVideos.get(url);let title=video?.getAttribute("title")||video?.getAttribute("aria-label")||"";
 if(!title){try{title=decodeURIComponent(new URL(url).pathname.split("/").pop());}catch{}}
 if(!title||youtubePage())title=document.title||"Video";
 if(Number.isFinite(video?.duration))title+=" · "+Math.floor(video.duration/60)+":"+String(Math.floor(video.duration%60)).padStart(2,"0");
 pageVideos.set(url,{url,title,time:promote||!old?Date.now():old.time});
}
async function detectedVideos(){
 for(const video of document.querySelectorAll("video"))rememberVideo(video);
 let remote=[];if(!youtubePage()){try{remote=(await browser.runtime.sendMessage({action:"detected-videos"}))?.videos||[];}catch{}}
 const all=new Map(remote.map(v=>[v.url,v]));for(const v of pageVideos.values()){const old=all.get(v.url);all.set(v.url,{...v,time:v.time});}
 return [...all.values()].sort((a,b)=>b.time-a.time);
}
function button(label,aria){const b=document.createElement("button");b.type="button";b.textContent=label;b.setAttribute("aria-label",aria||label);b.style.cssText="border:0;background:transparent;color:inherit;font:600 14px system-ui;cursor:pointer;padding:12px 14px;border-radius:12px";return b;}
async function downloadVideo(video){
 if(busy)return;busy=true;videoButton.disabled=true;videoPanel.hidden=true;videoToggle.setAttribute("aria-expanded","false");videoButton.textContent="Opening download…";
 try{const reply=await browser.runtime.sendMessage({action:"video-download",url:video?.url||"",pageUrl:location.href});videoButton.textContent=reply?.ok?"Download started":reply?.canceled?"Download canceled":reply?.error||"No downloadable video detected";}
 catch(error){videoButton.textContent=error.message;}
 finally{busy=false;videoButton.disabled=false;setTimeout(()=>{if(videoButton)videoButton.textContent="Download video";},5000);}
}
async function showVideoList(){
 if(!videoPanel.hidden){videoPanel.hidden=true;videoToggle.setAttribute("aria-expanded","false");return;}
 const videos=await detectedVideos();videoPanel.replaceChildren();
 videos.forEach((video,index)=>{const item=button((index===0?"Latest · ":"")+video.title);item.dataset.odmVideoChoice="true";item.setAttribute("role","menuitem");item.title=video.url;item.style.cssText+=";display:block;text-align:left;width:100%;color:#172338;font-weight:500;white-space:normal;overflow-wrap:anywhere";item.addEventListener("click",e=>{e.preventDefault();e.stopPropagation();downloadVideo(video);});videoPanel.append(item);});
 if(!videos.length){const empty=document.createElement("div");empty.textContent="No downloadable video link detected yet.";empty.style.cssText="padding:14px;color:#64748b";videoPanel.append(empty);}
 videoPanel.hidden=false;videoToggle.setAttribute("aria-expanded","true");
}
function createVideoButton(){
 if(videoDock||dismissed)return;
 videoDock=document.createElement("div");videoDock.dataset.odmVideoDock="true";videoDock.style.cssText="position:fixed;right:18px;bottom:18px;z-index:2147483647;display:flex;align-items:center;background:#2563eb;color:white;border-radius:14px;box-shadow:0 4px 22px #10204033;font:14px system-ui;max-width:calc(100vw - 36px)";
 videoButton=button("Download video","Download the latest detected video");videoButton.dataset.odmVideo="true";
 videoButton.addEventListener("click",async e=>{e.preventDefault();e.stopPropagation();const videos=await detectedVideos();await downloadVideo(videos[0]);});
 videoToggle=button("▴","Choose from detected videos");videoToggle.dataset.odmVideoToggle="true";videoToggle.setAttribute("aria-haspopup","menu");videoToggle.setAttribute("aria-expanded","false");videoToggle.addEventListener("click",e=>{e.preventDefault();e.stopPropagation();showVideoList();});
 const close=button("×","Hide video download button");close.dataset.odmVideoClose="true";close.addEventListener("click",e=>{e.preventDefault();e.stopPropagation();dismissed=true;videoDock.remove();});
 videoPanel=document.createElement("div");videoPanel.dataset.odmVideoList="true";videoPanel.setAttribute("role","menu");videoPanel.hidden=true;videoPanel.style.cssText="position:absolute;bottom:calc(100% + 8px);right:0;width:340px;max-width:calc(100vw - 36px);max-height:min(360px,60vh);overflow-y:auto;overscroll-behavior:contain;background:white;border:1px solid #dce4ef;border-radius:14px;padding:6px;box-shadow:0 6px 28px #10204033;color:#172338;box-sizing:border-box";
 videoDock.append(videoButton,videoToggle,close,videoPanel);(document.body||document.documentElement).append(videoDock);
 document.addEventListener("keydown",e=>{if(e.key==="Escape"){videoPanel.hidden=true;videoToggle.setAttribute("aria-expanded","false");}});
}
function updateVideoButton(event){
 if(dismissed)return;const videos=Array.from(document.querySelectorAll("video"));for(const video of videos)rememberVideo(video,event?.type==="play"&&event.target===video);
 if(!videos.length&&!pageVideos.size)return;createVideoButton();
}
document.addEventListener("play",updateVideoButton,true);document.addEventListener("loadedmetadata",updateVideoButton,true);setInterval(updateVideoButton,2000);updateVideoButton();

