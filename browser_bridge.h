// Firefox native messaging: framed UTF-8 JSON over inherited standard handles.
// The bridge only accepts HTTP(S) URLs; it exposes no shell or filesystem commands.
// Check access in the host process, never on the application's UI thread.
// A queued job alone is not sufficient reason to cancel Firefox's working response.
void checkBrowserHandoff(const std::wstring& url,int redirectDepth=0,const std::wstring& userAgent=L"",const std::wstring& referer=L""){
    if(isYouTube(url))return;
    if(needsNetworkTransport(url)){checkNetworkHandoff(url,userAgent,referer);return;}
    URL_COMPONENTS p{};p.dwStructSize=sizeof(p);p.dwHostNameLength=p.dwUrlPathLength=p.dwExtraInfoLength=(DWORD)-1;
    if(!WinHttpCrackUrl(url.c_str(),0,0,&p))throw std::runtime_error("Invalid download URL");
    std::wstring host(p.lpszHostName,p.dwHostNameLength),resource(p.lpszUrlPath,p.dwUrlPathLength);
    if(p.dwExtraInfoLength)resource.append(p.lpszExtraInfo,p.dwExtraInfoLength);if(resource.empty())resource=L"/";
    Internet session(openHttpSession(userAgent));
    WinHttpSetTimeouts(session,10000,10000,10000,30000);
    Internet connection(WinHttpConnect(session,host.c_str(),p.nPort,0));
    Internet request(WinHttpOpenRequest(connection,L"GET",resource.c_str(),nullptr,WINHTTP_NO_REFERER,WINHTTP_DEFAULT_ACCEPT_TYPES,p.nScheme==INTERNET_SCHEME_HTTPS?WINHTTP_FLAG_SECURE:0));
    DWORD policy=WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;WinHttpSetOption(request,WINHTTP_OPTION_REDIRECT_POLICY,&policy,sizeof(policy));
    std::wstring headers=L"Accept-Encoding: identity\r\n";if(!referer.empty())headers+=L"Referer: "+referer+L"\r\n";
    sendHttpRequest(request,headers);
    DWORD status=0,n=sizeof(status);if(!WinHttpQueryHeaders(request,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,WINHTTP_HEADER_NAME_BY_INDEX,&status,&n,WINHTTP_NO_HEADER_INDEX))fail("Check HTTP status");
    if(status!=200&&status!=206)httpFailure(status);
    auto type=header(request,WINHTTP_QUERY_CONTENT_TYPE);std::transform(type.begin(),type.end(),type.begin(),::towlower);
    auto ext=fs::path(std::wstring(p.lpszUrlPath,p.dwUrlPathLength)).extension().wstring();std::transform(ext.begin(),ext.end(),ext.begin(),::towlower);
    auto disposition=header(request,WINHTTP_QUERY_CONTENT_DISPOSITION);std::transform(disposition.begin(),disposition.end(),disposition.begin(),::towlower);
    if((type.find(L"text/html")!=std::wstring::npos||type.find(L"application/xhtml")!=std::wstring::npos)&&ext!=L".html"&&ext!=L".htm"&&ext!=L".xhtml"&&disposition.rfind(L"attachment",0)!=0)
        {auto next=redirectDepth<4?downloadRedirect(request,url):L"";if(!next.empty()){checkBrowserHandoff(next,redirectDepth+1,userAgent,referer);return;}
        throw PermanentDownloadError("This link returns a web page, not a downloadable file. Keep it in Firefox and use the page's actual download button.");}
}
bool pipeRead(HANDLE pipe,void* target,DWORD bytes){auto p=(char*)target;while(bytes){DWORD n=0;if(!ReadFile(pipe,p,bytes,&n,nullptr)||!n)return false;p+=n;bytes-=n;}return true;}
bool pipeWrite(HANDLE pipe,const void* source,DWORD bytes){auto p=(const char*)source;while(bytes){DWORD n=0;if(!WriteFile(pipe,p,bytes,&n,nullptr)||!n)return false;p+=n;bytes-=n;}return true;}
int nativeHost(){
    HANDLE input=GetStdHandle(STD_INPUT_HANDLE),output=GetStdHandle(STD_OUTPUT_HANDLE);
    for(;;){uint32_t length=0;if(!pipeRead(input,&length,4))return 0;if(length==0||length>65536)return 1;
        std::string bytes(length,0);if(!pipeRead(input,bytes.data(),length))return 1;nlohmann::json response;
        try{
            auto request=nlohmann::json::parse(bytes);auto action=request.value("action",std::string());
            if(action=="ping")response={{"ok",true},{"version","2.6.0"}};
            else if(action=="download"){
                auto url=request.at("url").get<std::string>();if(url.size()>16384||url.find_first_of("\r\n")!=std::string::npos||url.find('\0')!=std::string::npos)throw std::runtime_error("Invalid URL");
                auto wide=utf16(url);URL_COMPONENTS parts{};parts.dwStructSize=sizeof(parts);parts.dwHostNameLength=(DWORD)-1;
                if(!WinHttpCrackUrl(wide.c_str(),0,0,&parts)||!parts.dwHostNameLength||(parts.nScheme!=INTERNET_SCHEME_HTTP&&parts.nScheme!=INTERNET_SCHEME_HTTPS))throw std::runtime_error("Only HTTP and HTTPS links are supported");
                // Warm the tray app while the independent access check runs.
                bool checking=request.value("verifyAccess",false)&&!isYouTube(wide);
                // A promise-backed future does not block cancellation when it is destroyed.
                auto userAgent=utf16(request.value("userAgent",std::string())),referer=utf16(request.value("referer",std::string()));
                for(const auto& value:{userAgent,referer})if(value.size()>2048||value.find_first_of(L"\r\n")!=std::wstring::npos||value.find(L'\0')!=std::wstring::npos)throw std::runtime_error("Invalid browser request context");
                if(!referer.empty()&&referer.rfind(L"https://",0)!=0&&referer.rfind(L"http://",0)!=0)throw std::runtime_error("Invalid referring site");
                std::packaged_task<void()> check([wide,checking,userAgent,referer]{if(checking)checkBrowserHandoff(wide,0,userAgent,referer);});auto access=check.get_future();std::thread(std::move(check)).detach();
                HWND app=FindWindowW(L"OpenDownloadManagerCpp",nullptr);
                if(!app){wchar_t executable[32768];DWORD n=GetModuleFileNameW(nullptr,executable,32768);if(!n||n>=32768)fail("Locate application");auto command=quoteArgument(executable)+L" --background";STARTUPINFOW start{};start.cb=sizeof(start);PROCESS_INFORMATION process{};
                    if(!CreateProcessW(executable,command.data(),nullptr,nullptr,FALSE,0,nullptr,nullptr,&start,&process))fail("Start download manager");CloseHandle(process.hThread);CloseHandle(process.hProcess);
                    for(int i=0;i<400&&!app;++i){Sleep(25);app=FindWindowW(L"OpenDownloadManagerCpp",nullptr);}
                }
                if(!app)throw std::runtime_error("The download manager did not start");
                auto filename=request.value("filename",std::string());if(filename.size()>1024)filename.clear();
                DWORD promptId=GetCurrentProcessId();auto payload=nlohmann::json{{"url",url},{"promptId",promptId},{"filename",filename},{"checking",checking},{"userAgent",utf8(userAgent)},{"referer",utf8(referer)}}.dump();COPYDATASTRUCT data{0x4f444d33,(DWORD)payload.size(),payload.data()};DWORD_PTR accepted=0;
                if(!SendMessageTimeoutW(app,WM_COPYDATA,0,(LPARAM)&data,SMTO_ABORTIFHUNG|SMTO_BLOCK,4000,&accepted)||!accepted)throw std::runtime_error("The download manager could not accept this link (Windows error "+std::to_string(GetLastError())+"). Keep downloading in Firefox.");
                DWORD_PTR decision=0;bool notified=!checking;
                for(int i=0;i<24000;++i){
                    if(checking&&!notified&&access.wait_for(std::chrono::seconds(0))==std::future_status::ready){
                        try{access.get();}catch(...){DWORD_PTR ignored=0;SendMessageTimeoutW(app,WM_APP+73,promptId,2,SMTO_ABORTIFHUNG|SMTO_BLOCK,1000,&ignored);throw;}
                        DWORD_PTR ignored=0;if(!SendMessageTimeoutW(app,WM_APP+73,promptId,1,SMTO_ABORTIFHUNG|SMTO_BLOCK,1000,&ignored))throw std::runtime_error("The app did not finish checking this link.");notified=true;
                    }
                    if(!IsWindow(app))throw std::runtime_error("The app closed before the download was started.");
                    if(SendMessageTimeoutW(app,WM_APP+71,promptId,0,SMTO_ABORTIFHUNG|SMTO_BLOCK,1000,&decision)&&decision)break;
                    Sleep(25);
                }
                if(decision==1)response={{"ok",true},{"queued",true}};
                else if(decision==2)response={{"ok",false},{"canceled",true},{"error","Download canceled."}};
                else throw std::runtime_error("The download was not started. Try again from Firefox.");
            }else throw std::runtime_error("Unknown native message action");
        }catch(const std::exception& e){response={{"ok",false},{"error",e.what()}};}
        auto result=response.dump();uint32_t size=(uint32_t)result.size();if(!pipeWrite(output,&size,4)||!pipeWrite(output,result.data(),size))return 1;
    }
}
