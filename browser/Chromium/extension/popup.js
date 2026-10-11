"use strict";
const $=id=>document.getElementById(id);
async function status(){const result=await browser.runtime.sendMessage({action:"status"});$("capture").checked=result.capture;}
async function send(){const button=$("send");button.disabled=true;button.title="";try{const result=await browser.runtime.sendMessage({action:"download",url:$("url").value.trim()});button.textContent=result.ok?"Sent to app":"Try again";button.title=result.ok?"":result.error||"The app could not accept this link.";}catch(error){button.textContent="Try again";button.title=error.message;}finally{button.disabled=false;}}
$("send").addEventListener("click",send);$("url").addEventListener("keydown",e=>{if(e.key==="Enter")send();});
$("url").addEventListener("input",()=>{$("send").textContent="Send to app";$("send").title="";});
$("capture").addEventListener("change",async()=>{try{await browser.runtime.sendMessage({action:"capture",enabled:$("capture").checked});}catch(error){$("capture").checked=!$("capture").checked;$("capture").title=error.message;}});
$("check").addEventListener("click",async()=>{const button=$("check");button.disabled=true;try{const result=await browser.runtime.sendMessage({action:"ping"});button.textContent=result.ok?"App connected":"App unavailable";button.title=result.ok?"Version "+result.version:result.error||"Connection failed.";}catch(error){button.textContent="App unavailable";button.title=error.message;}finally{button.disabled=false;}});
status().catch(error=>{$("check").textContent="App unavailable";$("check").title=error.message;});
