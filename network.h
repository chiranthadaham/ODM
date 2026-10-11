bool needsNetworkTransport(const std::wstring& url){auto settings=networkSnapshot();return url.rfind(L"ftp://",0)==0||url.rfind(L"ftps://",0)==0||settings.proxy.rfind(L"socks",0)==0||settings.proxy.rfind(L"https://",0)==0;}

std::string curlSetting(const char* name,const std::wstring& value){std::string escaped;for(char c:utf8(value)){if(c=='\\'||c=='"')escaped+='\\';if(c=='\r'||c=='\n'||c==0)throw std::runtime_error("Invalid transfer option");escaped+=c;}return std::string(name)+" = \""+escaped+"\"\n";}
void curlTransfer(const std::shared_ptr<Job>& j){
 wchar_t system[MAX_PATH]{};if(!GetSystemDirectoryW(system,MAX_PATH))fail("Find Windows transport");auto executable=fs::path(system)/L"curl.exe";if(!fs::exists(executable))throw std::runtime_error("FTP/proxy/authentication requires Windows curl (available on current Windows 10/11)");
 auto partial=fs::path(j->path+L".part"),headers=fs::path(j->path+L".headers.tmp");fs::create_directories(partial.parent_path());bool ftp=j->url.rfind(L"ftp",0)==0;int restarts=0;
 struct Cleanup{fs::path path;~Cleanup(){std::error_code ec;fs::remove(path,ec);}} cleanup{headers};
 for(;;){
  auto options=networkSnapshot();int configuredLimit=limitKiB.load();uint64_t offset=fs::exists(partial)?fs::file_size(partial):0;std::wstring validator;{std::lock_guard<std::mutex> lock(j->mutex);validator=j->etag.empty()?j->modified:j->etag;}
  if(offset&&!ftp&&validator.empty()){fs::remove(partial);offset=0;}
  std::string config="silent\nshow-error\nfail\nlocation\ngloboff\nconnect-timeout = 15\nspeed-time = 30\nspeed-limit = 1\nmax-redirs = 10\nproto = \"=http,https,ftp,ftps\"\nproto-redir = \"=http,https,ftp,ftps\"\n";
  if(j->probeOnly)config+="head\n";
  config+=curlSetting("proto-redir",j->url.rfind(L"https://",0)==0?L"=https":ftp?L"=ftp,ftps":L"=http,https");
  config+=curlSetting("url",j->url)+curlSetting("output",partial.wstring())+curlSetting("dump-header",headers.wstring());
  if(offset){config+=curlSetting("continue-at",std::to_wstring(offset));if(!ftp)config+=curlSetting("header",L"If-Range: "+validator);}
  if(!j->userAgent.empty())config+=curlSetting("user-agent",j->userAgent);if(!j->referer.empty())config+=curlSetting("referer",j->referer);
  if(!options.proxy.empty()){config+=curlSetting("proxy",options.proxy)+curlSetting("noproxy",L"");if(!options.proxyUser.empty())config+=curlSetting("proxy-user",options.proxyUser+L":"+options.proxyPassword)+"proxy-anyauth\n";}
  if(!options.serverUser.empty()&&urlOrigin(j->url)==options.serverOrigin)config+=curlSetting("user",options.serverUser+L":"+options.serverPassword)+(ftp?"":"anyauth\n");
  if(configuredLimit>0)config+=curlSetting("limit-rate",std::to_wstring(configuredLimit*1024ULL));
  SECURITY_ATTRIBUTES sa{sizeof(sa),nullptr,TRUE};NativeHandle inputRead,inputWrite,errorRead,errorWrite;
  if(!CreatePipe(&inputRead.h,&inputWrite.h,&sa,0)||!CreatePipe(&errorRead.h,&errorWrite.h,&sa,0)||!SetHandleInformation(inputWrite.h,HANDLE_FLAG_INHERIT,0)||!SetHandleInformation(errorRead.h,HANDLE_FLAG_INHERIT,0))fail("Create transfer pipes");
  NativeHandle sink(CreateFileW(L"NUL",GENERIC_WRITE,FILE_SHARE_READ|FILE_SHARE_WRITE,&sa,OPEN_EXISTING,0,nullptr));STARTUPINFOW si{sizeof(si)};si.dwFlags=STARTF_USESTDHANDLES|STARTF_USESHOWWINDOW;si.wShowWindow=SW_HIDE;si.hStdInput=inputRead.h;si.hStdOutput=sink.h;si.hStdError=errorWrite.h;PROCESS_INFORMATION pi{};auto command=quoteArgument(executable.wstring())+L" --disable --config -";
  if(!CreateProcessW(executable.c_str(),command.data(),nullptr,nullptr,TRUE,CREATE_NO_WINDOW|CREATE_SUSPENDED,nullptr,nullptr,&si,&pi))fail("Start network transport");NativeHandle child(pi.hProcess),thread(pi.hThread),processJob(CreateJobObjectW(nullptr,nullptr));JOBOBJECT_EXTENDED_LIMIT_INFORMATION info{};info.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
  if(!processJob.h||!SetInformationJobObject(processJob.h,JobObjectExtendedLimitInformation,&info,sizeof(info))||!AssignProcessToJobObject(processJob.h,child.h)){TerminateProcess(child.h,1);fail("Protect network worker");}ResumeThread(thread.h);CloseHandle(inputRead.h);inputRead.h=nullptr;CloseHandle(errorWrite.h);errorWrite.h=nullptr;
  DWORD sent=0;if(!WriteFile(inputWrite.h,config.data(),(DWORD)config.size(),&sent,nullptr)||sent!=config.size()){TerminateProcess(child.h,1);throw std::runtime_error("Cannot send transfer settings");}SecureZeroMemory(config.data(),config.size());CloseHandle(inputWrite.h);inputWrite.h=nullptr;
  j->lanes=1;j->done=offset;std::string error;auto sampled=GetTickCount64();uint64_t prior=offset;bool rateChanged=false;DWORD exitCode=1;
  for(;;){
   DWORD available=0;if(PeekNamedPipe(errorRead.h,nullptr,0,nullptr,&available,nullptr)&&available){char buffer[2048];DWORD read=0;if(ReadFile(errorRead.h,buffer,std::min<DWORD>(sizeof(buffer),available),&read,nullptr)){error.append(buffer,read);if(error.size()>4096)error.erase(0,error.size()-4096);}}
   std::error_code ec;auto length=fs::file_size(partial,ec);if(!ec)j->done=length;
   auto now=GetTickCount64();if(now-sampled>=500){j->speed=(j->done.load()-std::min(prior,j->done.load()))*1000/(now-sampled);prior=j->done;sampled=now;
    std::ifstream headerFile(headers);std::string line;while(std::getline(headerFile,line)){if(!line.empty()&&line.back()=='\r')line.pop_back();auto colon=line.find(':');if(colon==std::string::npos)continue;auto key=line.substr(0,colon);std::transform(key.begin(),key.end(),key.begin(),::tolower);auto value=line.substr(colon+1);while(!value.empty()&&value.front()==' ')value.erase(0,1);if(key=="etag"||key=="last-modified"){std::lock_guard<std::mutex> lock(j->mutex);if(key=="etag"&&value.rfind("W/",0)!=0)j->etag=utf16(value);else if(key=="last-modified")j->modified=utf16(value);}if(key=="content-range"){auto slash=value.find('/');if(slash!=std::string::npos)try{j->total=std::stoull(value.substr(slash+1));}catch(...){}}else if(key=="content-length"&&j->total==0)try{j->total=offset+std::stoull(value);}catch(...){}}
   }
   if(j->pauseAfter&&j->done.load()-offset>=j->pauseAfter){j->status=Paused;j->stop=true;}
   if(j->stop||(rateChanged=configuredLimit!=limitKiB.load())){TerminateProcess(child.h,1);WaitForSingleObject(child.h,5000);break;}
   if(WaitForSingleObject(child.h,50)==WAIT_OBJECT_0){GetExitCodeProcess(child.h,&exitCode);break;}
  }
  if(j->stop)return;if(rateChanged)continue;
  if(exitCode==33&&offset&&restarts++<1){fs::remove(partial);j->done=0;j->total=0;continue;}
  if(exitCode!=0){if(exitCode==22)throw PermanentDownloadError("Server refused the transfer. Check the URL and connection credentials.");throw std::runtime_error(error.empty()?"Network transfer failed (curl code "+std::to_string(exitCode)+")":error);}
  std::map<std::string,std::string> finalHeaders;{std::ifstream in(headers);std::string line;while(std::getline(in,line)){if(line.rfind("HTTP/",0)==0)finalHeaders.clear();if(!line.empty()&&line.back()=='\r')line.pop_back();auto colon=line.find(':');if(colon==std::string::npos)continue;auto key=line.substr(0,colon);std::transform(key.begin(),key.end(),key.begin(),::tolower);auto value=line.substr(colon+1);while(!value.empty()&&value.front()==' ')value.erase(0,1);finalHeaders[key]=value;}}
  auto type=finalHeaders["content-type"],disposition=finalHeaders["content-disposition"];std::transform(type.begin(),type.end(),type.begin(),::tolower);std::transform(disposition.begin(),disposition.end(),disposition.begin(),::tolower);auto path=j->url.substr(0,j->url.find_first_of(L"?#"));auto extension=fs::path(path).extension().wstring();std::transform(extension.begin(),extension.end(),extension.begin(),::towlower);
  if((type.find("text/html")!=std::string::npos||type.find("application/xhtml")!=std::string::npos)&&extension!=L".html"&&extension!=L".htm"&&extension!=L".xhtml"&&disposition.rfind("attachment",0)!=0)throw PermanentDownloadError("This link returns a web page, not a downloadable file");
  if(j->probeOnly)return;
  if(!fs::is_regular_file(partial))throw std::runtime_error("Network transport produced no file");j->done=fs::file_size(partial);j->total=j->done.load();finalizeDownload(j,partial);j->status=Complete;{std::lock_guard<std::mutex> lock(j->mutex);j->error.clear();}return;
 }
}

void checkNetworkHandoff(const std::wstring& url,const std::wstring& userAgent,const std::wstring& referer){
 wchar_t temporary[MAX_PATH]{};GetTempPathW(MAX_PATH,temporary);auto j=std::make_shared<Job>();j->url=url;j->path=(fs::path(temporary)/(L"odm-check-"+std::to_wstring(GetCurrentProcessId())+L"-"+std::to_wstring(GetTickCount64()))).wstring();j->userAgent=userAgent;j->referer=referer;j->probeOnly=true;
 struct Cleanup{std::wstring path;~Cleanup(){std::error_code ec;fs::remove(path+L".part",ec);}} cleanup{j->path};curlTransfer(j);
}
