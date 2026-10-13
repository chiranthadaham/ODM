//! Local, per-user browser handoff. No browser-to-window messages or listener port.
use crate::{http, model::*, win::*};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs::File, io::{Read, Write}, os::windows::{io::{AsRawHandle, FromRawHandle}, process::CommandExt}, sync::{Arc, atomic::{AtomicUsize, Ordering}}, time::{Duration, Instant}};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::{Security::{*, Authorization::*}, System::Pipes::*};
const MAX_FRAME: usize = 65536;
static NEXT_ID: AtomicUsize = AtomicUsize::new(1usize << 32);

fn user_sid() -> Result<String,String> {
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)==0 {return Err(error("Read current user"));}
        let token=Handle(token);let mut buf=[0usize;128];let mut size=0;
        if GetTokenInformation(token.0,TokenUser,buf.as_mut_ptr().cast(),std::mem::size_of_val(&buf) as u32,&mut size)==0 {return Err(error("Read user SID"));}
        let user=&*(buf.as_ptr() as *const TOKEN_USER);let mut text=null_mut();
        if ConvertSidToStringSidW(user.User.Sid,&mut text)==0 {return Err(error("Format user SID"));}
        let mut len=0;while *text.add(len)!=0 {len+=1;}
        let sid=String::from_utf16_lossy(std::slice::from_raw_parts(text,len));LocalFree(text.cast());Ok(sid)
    }
}
fn pipe_name(engine:&Engine) -> Result<String,String> {
    let mut session=0;unsafe {if ProcessIdToSessionId(GetCurrentProcessId(),&mut session)==0{return Err(error("Read session"));}}
    let identity=format!("{}|{}|{}",user_sid()?,session,engine.data.to_string_lossy().to_lowercase());
    Ok(format!(r"\\.\pipe\ODM-{}",format!("{:x}",Sha256::digest(identity.as_bytes()))))
}
fn trusted_process(pid:u32) -> bool {
    unsafe {
        let process=Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid));
        if process.0.is_null(){return false;}
        let mut path=vec![0u16;32768];let mut size=path.len() as u32;
        if QueryFullProcessImageNameW(process.0,0,path.as_mut_ptr(),&mut size)==0{return false;}
        std::env::current_exe().is_ok_and(|own|path_key(&own)==path_key(std::path::Path::new(&String::from_utf16_lossy(&path[..size as usize]))))
    }
}
fn read_exact_until(file:&mut File,mut bytes:&mut [u8],deadline:Instant)->Result<(),String>{
    while !bytes.is_empty(){
        if Instant::now()>=deadline{return Err("Browser connection timed out".into());}
        let mut available=0;
        if unsafe{PeekNamedPipe(file.as_raw_handle().cast(),null_mut(),0,null_mut(),&mut available,null_mut())}==0{return Err("Browser connection closed".into());}
        if available==0 {std::thread::sleep(Duration::from_millis(10));continue;}
        let count=bytes.len().min(available as usize);
        let n=file.read(&mut bytes[..count]).map_err(|e|e.to_string())?;
        if n==0{return Err("Browser connection closed".into());}bytes=&mut bytes[n..];
    }Ok(())
}
fn read_frame(file:&mut File,timeout:Duration)->Result<Value,String>{
    let deadline=Instant::now()+timeout;let mut prefix=[0u8;4];read_exact_until(file,&mut prefix,deadline)?;
    let size=u32::from_le_bytes(prefix) as usize;if size==0||size>MAX_FRAME{return Err("Invalid browser message size".into());}
    let mut bytes=vec![0;size];read_exact_until(file,&mut bytes,deadline)?;serde_json::from_slice(&bytes).map_err(|_|"Invalid browser message".into())
}
fn write_frame(file:&mut File,value:&Value)->Result<(),String>{
    let bytes=serde_json::to_vec(value).map_err(|e|e.to_string())?;if bytes.len()>MAX_FRAME{return Err("Browser message too large".into());}
    file.write_all(&(bytes.len() as u32).to_le_bytes()).and_then(|_|file.write_all(&bytes)).map_err(|e|e.to_string())
}
fn validate(request:&Value)->Result<(),String>{
    let address=request["url"].as_str().ok_or("Missing URL")?;let url=valid_url(address)?;
    if !["http","https"].contains(&url.scheme()){return Err("Only HTTP and HTTPS browser links are supported".into());}
    for key in ["userAgent","referer"]{let value=request[key].as_str().unwrap_or("");if value.len()>2048||value.chars().any(|c|matches!(c,'\0'|'\r'|'\n')){return Err("Invalid browser request context".into());}}
    Ok(())
}
unsafe fn message(app:HWND,kind:u32,id:usize,value:isize)->Result<usize,String>{
    let mut result=0;SetLastError(0);
    if SendMessageTimeoutW(app,kind,id,value,SMTO_ABORTIFHUNG|SMTO_BLOCK,4000,&mut result)==0{return Err(error("App handoff"));}Ok(result)
}
fn serve(engine:Arc<Engine>,app:usize,file:&mut File)->Result<Value,String>{
    let mut request=read_frame(file,Duration::from_secs(5))?;
    if request["action"]=="ping"{return Ok(json!({"ok":true,"version":env!("CARGO_PKG_VERSION")}));}
    if request["action"]!="download"{return Err("Unknown browser message action".into());}validate(&request)?;
    let id=NEXT_ID.fetch_add(1,Ordering::Relaxed);let checking=request["verifyAccess"].as_bool().unwrap_or(false)&&!video_url(request["url"].as_str().unwrap_or(""));
    request["promptId"]=json!(id);request["checking"]=json!(checking);
    let bytes=serde_json::to_vec(&request).map_err(|e|e.to_string())?;if bytes.len()>MAX_FRAME{return Err("Browser message too large".into());}
    let app=app as HWND;let data=COPYDATASTRUCT{dwData:0x4f444d33,cbData:bytes.len() as u32,lpData:bytes.as_ptr() as *mut c_void};
    // Only this worker inside ODM sends window messages: UIPI is no longer involved.
    if unsafe{message(app,WM_COPYDATA,0,(&data as *const COPYDATASTRUCT) as isize)}?==0{return Err("The app rejected the link or its pending prompt queue is full".into());}
    let result=(|| {
        let (tx,rx)=std::sync::mpsc::channel();
        if checking {let address=request["url"].as_str().unwrap_or("").to_owned();let agent=request["userAgent"].as_str().unwrap_or("").to_owned();let referer=request["referer"].as_str().unwrap_or("").to_owned();std::thread::spawn(move||{let _=tx.send(http::check_access(&engine,&address,&agent,&referer));});}
        let mut notified=!checking;let deadline=Instant::now()+Duration::from_secs(600);
        loop{
            if unsafe{IsWindow(app)}==0{return Err("The app closed before the download started".into());}
            if !notified {if let Ok(result)=rx.try_recv(){unsafe{message(app,WM_APP+73,id,if result.is_ok(){1}else{2})}?;result?;notified=true;}}
            let decision=unsafe{message(app,WM_APP+71,id,0)}?;
            if decision!=0{return Ok(if decision==1{json!({"ok":true,"queued":true})}else{json!({"ok":false,"canceled":true,"error":"Download canceled"})});}
            if Instant::now()>=deadline{return Err("Download prompt timed out".into());}
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    let _=unsafe{message(app,WM_APP+76,id,0)};result
}
fn create_pipe(name:&str,first:bool)->Result<File,String>{
    unsafe{
        // Only this Windows user; Medium label permits its browser helper, not Low clients.
        // Remote clients are rejected independently of the ACL.
        let sddl=wide(format!("D:P(A;;GA;;;{})S:(ML;;NW;;;ME)",user_sid()?));let mut descriptor=null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(),1,&mut descriptor,null_mut())==0{return Err(error("Secure browser connection"));}
        let attrs=SECURITY_ATTRIBUTES{nLength:std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,lpSecurityDescriptor:descriptor,bInheritHandle:0};
        let handle=CreateNamedPipeW(wide(name).as_ptr(),PIPE_ACCESS_DUPLEX|if first{FILE_FLAG_FIRST_PIPE_INSTANCE}else{0},PIPE_TYPE_BYTE|PIPE_READMODE_BYTE|PIPE_WAIT|PIPE_REJECT_REMOTE_CLIENTS,32,MAX_FRAME as u32+4,MAX_FRAME as u32+4,5000,&attrs);
        LocalFree(descriptor);if handle==INVALID_HANDLE_VALUE{return Err(error("Create browser connection"));}Ok(File::from_raw_handle(handle.cast()))
    }
}
pub fn start(engine:Arc<Engine>,app:HWND)->Result<(),String>{
    let name=pipe_name(&engine)?;let first=create_pipe(&name,true)?;let app=app as usize;
    std::thread::spawn(move||{
        let mut pipe=first;
        loop{
            let connected=unsafe{ConnectNamedPipe(pipe.as_raw_handle().cast(),null_mut())}!=0||unsafe{GetLastError()}==ERROR_PIPE_CONNECTED;
            if connected {
                let mut pid=0;
                let authenticated=unsafe{GetNamedPipeClientProcessId(pipe.as_raw_handle().cast(),&mut pid)}!=0&&trusted_process(pid);
                if authenticated{let engine=engine.clone();std::thread::spawn(move||{let reply=serve(engine,app,&mut pipe).unwrap_or_else(|e|json!({"ok":false,"error":e}));if write_frame(&mut pipe,&reply).is_ok(){unsafe{FlushFileBuffers(pipe.as_raw_handle().cast());}}});}
            }
            if unsafe{IsWindow(app as HWND)}==0{break;}
            loop {
                match create_pipe(&name,false) { Ok(next)=>{pipe=next;break;}, Err(_)=>{
                    if unsafe{IsWindow(app as HWND)}==0{return;}
                    std::thread::sleep(Duration::from_millis(100));
                }}
            }
        }
    });Ok(())
}
pub fn send(engine:&Engine,request:Value)->Result<Value,String>{
    if request["action"]!="ping"{validate(&request)?;}let name=pipe_name(engine)?;let mut launched=false;let mut running_since=None;let deadline=Instant::now()+Duration::from_secs(10);
    loop{
        let handle=unsafe{CreateFileW(wide(&name).as_ptr(),FILE_READ_DATA|FILE_WRITE_DATA|FILE_READ_ATTRIBUTES|SYNCHRONIZE,0,null(),OPEN_EXISTING,SECURITY_SQOS_PRESENT|SECURITY_IDENTIFICATION,null_mut())};
        if handle!=INVALID_HANDLE_VALUE{
            let mut file=unsafe{File::from_raw_handle(handle.cast())};let mut pid=0;
            if unsafe{GetNamedPipeServerProcessId(file.as_raw_handle().cast(),&mut pid)}==0||!trusted_process(pid){return Err("Browser connection identity check failed".into());}
            write_frame(&mut file,&request)?;return read_frame(&mut file,Duration::from_secs(610));
        }
        if !launched {
            let app=unsafe{FindWindowW(wide(crate::bridge::CLASS).as_ptr(),null())};
            if !app.is_null(){
                let since=running_since.get_or_insert_with(Instant::now);
                if since.elapsed()>Duration::from_secs(2){return Err("The running ODM copy does not provide the updated browser connection. Exit it from the tray and open the latest installed version.".into());}
                std::thread::sleep(Duration::from_millis(10));continue;
            }
            launched=true;
            let mut in_job=0;
            let breakaway=unsafe {
                if IsProcessInJob(GetCurrentProcess(),null_mut(),&mut in_job)==0 {true}
                else if in_job==0 {false}
                else {
                    let mut limits=JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                    QueryInformationJobObject(null_mut(),JobObjectExtendedLimitInformation,(&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),std::mem::size_of_val(&limits) as u32,null_mut())==0 || limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE != 0
                }
            };
            std::process::Command::new(std::env::current_exe().map_err(|e|e.to_string())?).arg("--background").creation_flags(CREATE_NO_WINDOW|if breakaway {CREATE_BREAKAWAY_FROM_JOB}else{0}).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().map_err(|e|e.to_string())?;
        }
        if Instant::now()>=deadline{return Err("The app did not open its browser connection".into());}
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_context_is_bounded() {
        assert!(validate(&json!({"url":"https://example.com/file","referer":"https://example.com/"})).is_ok());
        assert!(validate(&json!({"url":"https://example.com/file","referer":"https://example.com/\r\nX: bad"})).is_err());
        assert!(validate(&json!({"url":"https://example.com/file","userAgent":"x".repeat(2049)})).is_err());
    }
    #[test]
    fn browser_transport_rejects_local_and_unsupported_urls() {
        for url in ["file:///C:/Windows/win.ini","javascript:alert(1)","ftp://example.com/file",""] {
            assert!(validate(&json!({"url":url})).is_err());
        }
    }
}
