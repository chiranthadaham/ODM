"use strict";
const result=document.getElementById("result");
for(const id of ["accept","decline"])document.getElementById(id).addEventListener("click",async()=>{await browser.storage.local.set({consentVersion:id==="accept"?1:0,capture:id==="accept"});result.textContent=id==="accept"?"Enabled. You can close this tab and use ODM.":"Disabled. Downloads stay in your browser.";});

document.getElementById("remove").addEventListener("click",()=>browser.management.uninstallSelf({showConfirmDialog:true}));
