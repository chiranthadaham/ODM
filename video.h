// YouTube routing and a hidden, cancellable helper process. No command shell is used.
fs::path toolsOverride;
struct VideoSettingsChanged:std::runtime_error{VideoSettingsChanged():std::runtime_error("Applying updated transfer settings"){} };
bool isYouTube(const std::wstring& url) {
    URL_COMPONENTS p{};p.dwStructSize=sizeof(p);p.dwHostNameLength=(DWORD)-1;
    if(!WinHttpCrackUrl(url.c_str(),0,0,&p))return false;
    std::wstring host(p.lpszHostName,p.dwHostNameLength);std::transform(host.begin(),host.end(),host.begin(),::towlower);
    return host==L"youtu.be"||host==L"youtube.com"||(host.size()>12&&host.compare(host.size()-12,12,L".youtube.com")==0);
}
bool needsVideoExtractor(const std::wstring& url){
    if(isYouTube(url))return true;auto path=url.substr(0,url.find_first_of(L"?#"));std::transform(path.begin(),path.end(),path.begin(),::towlower);
    return (path.size()>=5&&path.compare(path.size()-5,5,L".m3u8")==0)||(path.size()>=4&&path.compare(path.size()-4,4,L".mpd")==0);
}
fs::path toolFolder() {if(!toolsOverride.empty())return toolsOverride;auto embedded=embeddedRuntime();if(!embedded.empty())return embedded;wchar_t p[32768];DWORD n=GetModuleFileNameW(nullptr,p,32768);if(!n||n==32768)fail("Locate application folder");return fs::path(p).parent_path()/L"tools";}
std::wstring quoteArgument(const std::wstring& arg) {
    std::wstring result=L"\"";size_t slashes=0;
    for(wchar_t c:arg){if(c==L'\\'){++slashes;continue;}if(c==L'\"'){result.append(slashes*2+1,L'\\');result+=c;}else{result.append(slashes,L'\\');result+=c;}slashes=0;}
    result.append(slashes*2,L'\\');return result+L"\"";
}
struct NativeHandle {HANDLE h=nullptr;NativeHandle()=default;explicit NativeHandle(HANDLE value):h(value){}~NativeHandle(){if(h&&h!=INVALID_HANDLE_VALUE)CloseHandle(h);}NativeHandle(const NativeHandle&)=delete;};
fs::path finishHlsTransportStream(const std::shared_ptr<Job>& j,const fs::path& source,const fs::path& tools,HANDLE group){
 auto temporary=source.parent_path()/(L".odm-ts-"+std::to_wstring(GetCurrentProcessId())+L"-"+std::to_wstring(GetTickCount64())+L".part");
 struct Cleanup{fs::path path;~Cleanup(){std::error_code ec;fs::remove(path,ec);}} cleanup{temporary};
 {std::lock_guard<std::mutex> lock(j->mutex);j->error=L"Finalizing HLS transport stream...";}
 std::vector<std::wstring> args={(tools/L"ffmpeg.exe").wstring(),L"-hide_banner",L"-loglevel",L"error",L"-nostdin",L"-i",source.wstring(),L"-map",L"0:v?",L"-map",L"0:a?",L"-c",L"copy",L"-f",L"mpegts",temporary.wstring()};std::wstring command;for(auto& arg:args){if(!command.empty())command+=L' ';command+=quoteArgument(arg);}
 SECURITY_ATTRIBUTES security{sizeof(security),nullptr,TRUE};NativeHandle sink(CreateFileW(L"NUL",GENERIC_WRITE,FILE_SHARE_READ|FILE_SHARE_WRITE,&security,OPEN_EXISTING,0,nullptr));STARTUPINFOW start{sizeof(start)};start.dwFlags=STARTF_USESTDHANDLES|STARTF_USESHOWWINDOW;start.wShowWindow=SW_HIDE;start.hStdInput=nullptr;start.hStdOutput=sink.h;start.hStdError=sink.h;PROCESS_INFORMATION process{};
 if(!CreateProcessW(args[0].c_str(),command.data(),nullptr,nullptr,TRUE,CREATE_NO_WINDOW|CREATE_SUSPENDED,nullptr,nullptr,&start,&process))fail("Start HLS merge");NativeHandle child(process.hProcess),thread(process.hThread);if(!AssignProcessToJobObject(group,child.h)){TerminateProcess(child.h,1);fail("Protect HLS merger");}ResumeThread(thread.h);
 while(WaitForSingleObject(child.h,50)!=WAIT_OBJECT_0){if(j->stop){TerminateProcess(child.h,2);WaitForSingleObject(child.h,5000);return source;}}
 DWORD code=1;GetExitCodeProcess(child.h,&code);if(code||!fs::is_regular_file(temporary))throw std::runtime_error("Cannot create the HLS transport stream; the downloaded source was preserved");
 auto destination=source;destination.replace_extension(L".ts");{std::lock_guard<std::mutex> lock(j->mutex);j->path=destination.wstring();}finalizeDownload(j,temporary);
 fs::path result;{std::lock_guard<std::mutex> lock(j->mutex);result=j->path;}if(result!=source){std::error_code ec;fs::remove(source,ec);}return result;
}
void videoAttempt(const std::shared_ptr<Job>& j,int attempt) {
    {std::lock_guard<std::mutex> lock(j->mutex);j->error=L"Preparing video support…";}
    auto tools=toolFolder();auto executable=tools/L"yt-dlp.exe";
    for(auto name:{L"yt-dlp.exe",L"deno.exe",L"ffmpeg.exe",L"ffprobe.exe"})if(!fs::exists(tools/name))throw std::runtime_error("Video support files are missing. Restart the app to restore bundled support.");
    std::wstring initialPath;{std::lock_guard<std::mutex> lock(j->mutex);initialPath=j->path;j->error=L"Finding available video and audio formats…";}
    auto folder=fs::path(initialPath).parent_path();fs::create_directories(folder);
    auto temporary=folder/(L".odm-video-"+std::to_wstring(std::hash<std::wstring>{}(initialPath)));
    fs::create_directories(temporary);SetFileAttributesW(temporary.c_str(),FILE_ATTRIBUTE_HIDDEN);
    int videoLimit=limitKiB.load(),videoConnections=connectionsPerFile.load();auto network=networkSnapshot();
    std::vector<std::wstring> args={executable.wstring(),L"--ignore-config",L"--no-playlist",L"--no-simulate",L"--newline",L"--progress",L"--encoding",L"utf-8",L"--windows-filenames",L"--trim-filenames",L"180",L"--no-overwrites",L"--continue",L"--concurrent-fragments",std::to_wstring(isYouTube(j->url)?(attempt?1:std::min(4,connectionsPerFile.load())):connectionsPerFile.load()),L"--ffmpeg-location",tools.wstring(),L"--js-runtimes",L"deno:"+(tools/L"deno.exe").wstring(),L"--format",L"bv*+ba/b",L"--merge-output-format",L"mp4",L"--paths",L"home:"+folder.wstring(),L"--paths",L"temp:"+temporary.wstring(),L"--output",L"%(title).120B [%(id)s].%(ext)s",L"--print",L"after_move:ODM_FILE:%(filepath)s",L"--progress-template",L"download:ODM_PROGRESS:%(progress.downloaded_bytes)s|%(progress.total_bytes,progress.total_bytes_estimate)s|%(progress.speed)s"};
    args.insert(args.end(),{L"--retries",L"3",L"--fragment-retries",L"3",L"--abort-on-unavailable-fragments",L"--socket-timeout",L"15"});
    bool hls=j->url.substr(0,j->url.find_first_of(L"?#")).find(L".m3u8")!=std::wstring::npos;
    if(hls)args.push_back(L"--hls-use-mpegts");
    if(isYouTube(j->url))args.insert(args.end(),{L"--no-cache-dir",L"--check-formats"});
    if(attempt)args.insert(args.end(),{L"--format",L"bv*[protocol=https]+ba[protocol=https]/b[protocol=https]/bv*+ba/b"});
    if(videoLimit>0){args.push_back(L"--limit-rate");args.push_back(std::to_wstring(videoLimit)+L"K");}
    std::string privateConfig;
    if(!network.proxy.empty()){
        auto proxy=network.proxy;auto encode=[](const std::wstring& value){std::wstring out;const wchar_t* digits=L"0123456789ABCDEF";for(unsigned char c:utf8(value)){if((c>='a'&&c<='z')||(c>='A'&&c<='Z')||(c>='0'&&c<='9'))out+=c;else{out+=L'%';out+=digits[c>>4];out+=digits[c&15];}}return out;};
        if(!network.proxyUser.empty()){auto at=proxy.find(L"://")+3;proxy.insert(at,encode(network.proxyUser)+L":"+encode(network.proxyPassword)+L"@");}
        std::string escaped;for(char c:utf8(proxy)){if(c=='\\'||c=='"')escaped+='\\';escaped+=c;}privateConfig="--proxy \""+escaped+"\"\n";
        args.insert(args.end(),{L"--config-locations",L"-"});
    }
    args.push_back(L"--");args.push_back(j->url);
    std::wstring command;for(auto& arg:args){if(!command.empty())command+=L' ';command+=quoteArgument(arg);}
    SECURITY_ATTRIBUTES security{sizeof(security),nullptr,TRUE};NativeHandle readPipe,writePipe;
    if(!CreatePipe(&readPipe.h,&writePipe.h,&security,0)||!SetHandleInformation(readPipe.h,HANDLE_FLAG_INHERIT,0))fail("Create video progress pipe");
    NativeHandle input,inputWrite;
    if(privateConfig.empty())input.h=CreateFileW(L"NUL",GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_WRITE,&security,OPEN_EXISTING,0,nullptr);
    else if(!CreatePipe(&input.h,&inputWrite.h,&security,0)||!SetHandleInformation(inputWrite.h,HANDLE_FLAG_INHERIT,0))fail("Create private video settings pipe");
    STARTUPINFOW start{};start.cb=sizeof(start);start.dwFlags=STARTF_USESTDHANDLES|STARTF_USESHOWWINDOW;start.wShowWindow=SW_HIDE;start.hStdInput=input.h;start.hStdOutput=writePipe.h;start.hStdError=writePipe.h;
    PROCESS_INFORMATION process{};
    if(!CreateProcessW(executable.c_str(),command.data(),nullptr,nullptr,TRUE,CREATE_NO_WINDOW|CREATE_SUSPENDED,nullptr,tools.c_str(),&start,&process))fail("Start video extractor");
    NativeHandle child(process.hProcess),thread(process.hThread),group(CreateJobObjectW(nullptr,nullptr));
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits{};limits.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if(!group.h||!SetInformationJobObject(group.h,JobObjectExtendedLimitInformation,&limits,sizeof(limits))||!AssignProcessToJobObject(group.h,child.h)){TerminateProcess(child.h,1);fail("Manage video helper process");}
    ResumeThread(thread.h);
    if(!privateConfig.empty()){DWORD sent=0;if(!WriteFile(inputWrite.h,privateConfig.data(),(DWORD)privateConfig.size(),&sent,nullptr)||sent!=privateConfig.size()){TerminateJobObject(group.h,1);throw std::runtime_error("Cannot send video connection settings");}SecureZeroMemory(privateConfig.data(),privateConfig.size());CloseHandle(inputWrite.h);inputWrite.h=nullptr;}CloseHandle(writePipe.h);writePipe.h=nullptr;
    std::string pending,lastError;std::wstring finalPath;
    auto line=[&](std::string value){
        while(!value.empty()&&(value.back()=='\r'||value.back()=='\n'))value.pop_back();
        if(value.rfind("ODM_FILE:",0)==0){finalPath=utf16(value.substr(9));return;}
        if(value.rfind("ODM_PROGRESS:",0)==0){
            std::istringstream fields(value.substr(13));std::string done,total,speed;std::getline(fields,done,'|');std::getline(fields,total,'|');std::getline(fields,speed,'|');
            auto numeric=[](const std::string& s)->uint64_t{try{double n=std::stod(s);return n>0?(uint64_t)n:0;}catch(...){return 0;}};
            j->done=numeric(done);j->total=numeric(total);j->speed=numeric(speed);j->lanes=isYouTube(j->url)?(attempt?1:std::min(4,connectionsPerFile.load())):connectionsPerFile.load();
            if(j->pauseAfter&&j->done>=j->pauseAfter){j->status=Paused;j->stop=true;}
            std::lock_guard<std::mutex> lock(j->mutex);j->error=L"Downloading video/audio…";return;
        }
        if(value.find("ERROR:")!=std::string::npos){lastError=value;std::lock_guard<std::mutex> lock(j->mutex);j->error=utf16(value);}
        else if(value.find("[Merger]")!=std::string::npos||value.find("[VideoRemuxer]")!=std::string::npos){std::lock_guard<std::mutex> lock(j->mutex);j->error=L"Combining video and audio…";}
    };
    bool terminated=false;
    for(;;){
        if(j->stop&&!terminated){TerminateJobObject(group.h,2);terminated=true;}
        if(videoLimit!=limitKiB.load()||videoConnections!=connectionsPerFile.load()){TerminateJobObject(group.h,1);WaitForSingleObject(child.h,5000);throw VideoSettingsChanged();}
        DWORD available=0;
        if(PeekNamedPipe(readPipe.h,nullptr,0,nullptr,&available,nullptr)&&available){char buffer[4096];DWORD bytes=0;if(ReadFile(readPipe.h,buffer,std::min<DWORD>(sizeof(buffer),available),&bytes,nullptr)&&bytes){pending.append(buffer,bytes);size_t end;while((end=pending.find('\n'))!=std::string::npos){line(pending.substr(0,end));pending.erase(0,end+1);}if(pending.size()>65536)pending.erase(0,pending.size()-65536);continue;}}
        if(WaitForSingleObject(child.h,0)==WAIT_OBJECT_0)break;
        Sleep(50);
    }
    if(!pending.empty())line(pending);DWORD exitCode=1;GetExitCodeProcess(child.h,&exitCode);
    if(j->stop)return;
    if(exitCode!=0)throw std::runtime_error(lastError.empty()?"YouTube download failed. The video may be unavailable or the site may require sign-in; see the app details.":lastError);
    if(finalPath.empty()||!fs::is_regular_file(finalPath))throw std::runtime_error("Video helper finished without a completed output file");
    if(hls){finalPath=finishHlsTransportStream(j,finalPath,tools,group.h).wstring();if(j->stop)return;}
    {std::lock_guard<std::mutex> lock(j->mutex);j->path=finalPath;j->error.clear();}
    std::error_code cleanupError;fs::remove(temporary,cleanupError);
    j->done=fs::file_size(finalPath);j->total=j->done.load();j->status=Complete;
}

// A fresh extractor process renews expiring/rejected media URLs, keeping partial files.
// Only transient YouTube media 403s are retried; unavailable/private videos fail promptly.
void videoTransfer(const std::shared_ptr<Job>& j){
    for(int attempt=0;attempt<3&&!j->stop;++attempt){
        try{videoAttempt(j,attempt);return;}
        catch(const VideoSettingsChanged&){--attempt;continue;}
        catch(const std::exception& e){
            std::string message=e.what();bool forbidden=message.find("403")!=std::string::npos&&(message.find("HTTP")!=std::string::npos||message.find("Forbidden")!=std::string::npos);
            if(!isYouTube(j->url)||!forbidden)throw;
            if(attempt==2)throw std::runtime_error("YouTube refused a media request (HTTP 403) after refreshing the video link. Retry later or use another available video. Details: "+message);
            {std::lock_guard<std::mutex> lock(j->mutex);j->error=L"Refreshing YouTube media links and retrying ("+std::to_wstring(attempt+2)+L"/3)…";}j->speed=0;
            for(int i=0;i<(attempt+1)*20&&!j->stop;++i)Sleep(100);
        }
    }
}
