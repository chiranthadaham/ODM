// Included by main.cpp after the sequential transfer engine.
// Persistent, independently resumable byte ranges; single-connection fallback.
struct SegmentRange {uint64_t start,end;};
void clearSegments(const fs::path& folder) {
    std::error_code ec;
    for(int i=0;i<128;++i)fs::remove(folder/(std::to_wstring(i)+L".part"),ec);
    fs::remove(folder/L"metadata.txt",ec);fs::remove(folder/L"metadata.tmp",ec);fs::remove(folder,ec);
}
bool segmentedTransfer(const std::shared_ptr<Job>& j) {
    resetPacing(j);
    int count=std::clamp(connectionsPerFile.load(),1,16);j->lanes=1;
    const auto folder=fs::path(j->path+L".segments");
    auto single=[&](){if(fs::exists(folder))clearSegments(folder);return false;};
    // Preserve older single-stream partial downloads instead of discarding their progress.
    if(fs::exists(j->path+L".part")&&!fs::exists(folder))return false;
    URL_COMPONENTS p{};p.dwStructSize=sizeof(p);p.dwSchemeLength=p.dwHostNameLength=p.dwUrlPathLength=p.dwExtraInfoLength=(DWORD)-1;
    if(!WinHttpCrackUrl(j->url.c_str(),0,0,&p)||(p.nScheme!=INTERNET_SCHEME_HTTP&&p.nScheme!=INTERNET_SCHEME_HTTPS))return false;
    const std::wstring host(p.lpszHostName,p.dwHostNameLength);
    std::wstring resource(p.lpszUrlPath,p.dwUrlPathLength);if(p.dwExtraInfoLength)resource.append(p.lpszExtraInfo,p.dwExtraInfoLength);if(resource.empty())resource=L"/";
    const DWORD flags=p.nScheme==INTERNET_SCHEME_HTTPS?WINHTTP_FLAG_SECURE:0;
    uint64_t total=0;std::wstring validator;bool useEtag=false;
    Internet session(openHttpSession(j->userAgent));WinHttpSetTimeouts(session,10000,10000,10000,30000);
    Internet connection(WinHttpConnect(session,host.c_str(),p.nPort,0));
    {
        Internet req(WinHttpOpenRequest(connection,L"GET",resource.c_str(),nullptr,WINHTTP_NO_REFERER,WINHTTP_DEFAULT_ACCEPT_TYPES,flags));
        DWORD policy=WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;WinHttpSetOption(req,WINHTTP_OPTION_REDIRECT_POLICY,&policy,sizeof(policy));
        std::wstring probeHeaders=L"Range: bytes=0-0\r\nAccept-Encoding: identity\r\n";if(!j->referer.empty())probeHeaders+=L"Referer: "+j->referer+L"\r\n";
        sendHttpRequest(req,probeHeaders);
        DWORD code=0,n=sizeof(code);WinHttpQueryHeaders(req,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,WINHTTP_HEADER_NAME_BY_INDEX,&code,&n,WINHTTP_NO_HEADER_INDEX);
        if(code==200){single();transfer(j,true,req);return true;}
        if(code!=206){httpFailure(code);return single();}
        auto range=header(req,WINHTTP_QUERY_CONTENT_RANGE);unsigned long long a=0,b=0,t=0;wchar_t extra=0;
        if(swscanf(range.c_str(),L"bytes %llu-%llu/%llu%c",&a,&b,&t,&extra)!=3||a!=0||b!=0||t<512*1024)return single();
        auto encoding=header(req,WINHTTP_QUERY_CONTENT_ENCODING);if(!encoding.empty()&&encoding!=L"identity")return single();
        total=t;validator=header(req,WINHTTP_QUERY_ETAG);if(validator.rfind(L"W/",0)==0)validator.clear();useEtag=!validator.empty();if(validator.empty())validator=header(req,WINHTTP_QUERY_LAST_MODIFIED);
        if(validator.empty())return single();
    }
    count=std::min<uint64_t>(128,total/(256*1024));if(count<2)return single();
    fs::create_directories(folder);
    bool matches=false;
    {
        std::ifstream in(folder/L"metadata.txt");std::string savedUrl,savedValidator;uint64_t savedTotal=0;int savedCount=0;
        if(in>>std::quoted(savedUrl)>>std::quoted(savedValidator)>>savedTotal>>savedCount){matches=savedUrl==utf8(j->url)&&savedValidator==utf8(validator)&&savedTotal==total&&savedCount>=2&&savedCount<=128;if(matches)count=savedCount;}
    }
    if(!matches){clearSegments(folder);fs::create_directories(folder);}
    {
        std::ofstream out(folder/L"metadata.tmp",std::ios::trunc);out<<std::quoted(utf8(j->url))<<'\n'<<std::quoted(utf8(validator))<<'\n'<<total<<' '<<count<<'\n';out.flush();if(!out)throw std::runtime_error("Cannot save segment metadata");out.close();
        if(!MoveFileExW((folder/L"metadata.tmp").c_str(),(folder/L"metadata.txt").c_str(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH))fail("Save segment metadata");
    }
    std::vector<SegmentRange> ranges;
    uint64_t existing=0;
    for(int i=0;i<count;++i){uint64_t a=total*i/count,b=total*(i+1)/count-1;ranges.push_back({a,b});auto path=folder/(std::to_wstring(i)+L".part");uint64_t length=fs::exists(path)?fs::file_size(path):0;if(length>b-a+1){fs::remove(path);length=0;}existing+=length;}
    j->done=existing;j->total=total;j->lanes=std::min(count,connectionsPerFile.load());
    std::atomic<bool> abort{false},unsupported{false};std::atomic<int> finished{0},nextRange{0};
    std::mutex failureMutex,finishMutex;std::condition_variable finishChanged;std::string failure;
    std::vector<std::thread> threads;
    auto began=std::chrono::steady_clock::now();
    const int workerCount=std::min(16,count);
    try {for(int lane=0;lane<workerCount;++lane)threads.emplace_back([&,lane]{
        try {
          while(!j->stop&&!abort){
            while(lane>=connectionsPerFile.load()&&!j->stop&&!abort&&nextRange<count)Sleep(50);
            if(j->stop||abort||nextRange>=count)break;
            int i=nextRange.fetch_add(1);if(i>=count)break;
            j->lanes=std::min(count,connectionsPerFile.load());
            auto path=folder/(std::to_wstring(i)+L".part");uint64_t have=fs::exists(path)?fs::file_size(path):0;
            auto range=ranges[i];uint64_t wanted=range.end-range.start+1;
            if(have<wanted&&!j->stop&&!abort) {
                Internet req(WinHttpOpenRequest(connection,L"GET",resource.c_str(),nullptr,WINHTTP_NO_REFERER,WINHTTP_DEFAULT_ACCEPT_TYPES,flags));
                DWORD policy=WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;WinHttpSetOption(req,WINHTTP_OPTION_REDIRECT_POLICY,&policy,sizeof(policy));
                std::wstring headers=L"Accept-Encoding: identity\r\nRange: bytes="+std::to_wstring(range.start+have)+L"-"+std::to_wstring(range.end)+L"\r\nIf-Range: "+validator+L"\r\n";
                if(!j->referer.empty())headers+=L"Referer: "+j->referer+L"\r\n";
                sendHttpRequest(req,headers);
                DWORD code=0,n=sizeof(code);WinHttpQueryHeaders(req,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,WINHTTP_HEADER_NAME_BY_INDEX,&code,&n,WINHTTP_NO_HEADER_INDEX);
                if(code==200||code==416||code==403||code==429||code==503){unsupported=true;throw std::runtime_error("Server stopped accepting validated byte ranges");}
                if(code!=206)throw std::runtime_error("Segment HTTP status "+std::to_string(code));
                unsigned long long a=0,b=0,t=0;wchar_t extra=0;auto returned=header(req,WINHTTP_QUERY_CONTENT_RANGE);
                if(swscanf(returned.c_str(),L"bytes %llu-%llu/%llu%c",&a,&b,&t,&extra)!=3||a!=range.start+have||b!=range.end||t!=total){unsupported=true;throw std::runtime_error("Server returned an inconsistent byte range");}
                auto returnedValidator=header(req,useEtag?WINHTTP_QUERY_ETAG:WINHTTP_QUERY_LAST_MODIFIED);
                if(!returnedValidator.empty()&&returnedValidator!=validator){unsupported=true;throw std::runtime_error("Remote file changed during segmentation");}
                auto encoding=header(req,WINHTTP_QUERY_CONTENT_ENCODING);if(!encoding.empty()&&encoding!=L"identity"){unsupported=true;throw std::runtime_error("Encoded byte range cannot be assembled safely");}
                File file(CreateFileW(path.c_str(),GENERIC_WRITE,FILE_SHARE_READ,nullptr,OPEN_ALWAYS,FILE_ATTRIBUTE_NORMAL,nullptr));LARGE_INTEGER at;at.QuadPart=have;if(!SetFilePointerEx(file,at,nullptr,FILE_BEGIN))fail("Seek segment");
                char buffer[262144];
                while(have<wanted&&!j->stop&&!abort) {
                    DWORD bytes=0;if(!WinHttpReadData(req,buffer,(DWORD)std::min<uint64_t>(transferBufferSize(),wanted-have),&bytes))fail("Read segment");if(!bytes)throw std::runtime_error("Segment ended before expected byte count");
                    DWORD wrote=0;if(!WriteFile(file,buffer,bytes,&wrote,nullptr)||wrote!=bytes)fail("Write segment");have+=bytes;j->done.fetch_add(bytes);
                    if(j->pauseAfter&&j->done.load()-existing>=j->pauseAfter){j->status=Paused;j->stop=true;}
                    paceDownload(j,bytes,&abort);
                }
                if(!FlushFileBuffers(file))fail("Flush segment");
            }
          }
        } catch(const std::exception& e){std::lock_guard<std::mutex> lock(failureMutex);if(failure.empty())failure=e.what();abort=true;}
        {std::lock_guard<std::mutex> lock(finishMutex);++finished;}finishChanged.notify_one();
    });
    }catch(...){abort=true;for(auto& t:threads)if(t.joinable())t.join();throw;}
    uint64_t prior=existing;auto sampled=std::chrono::steady_clock::now();
    while(finished<workerCount){{std::unique_lock<std::mutex> lock(finishMutex);finishChanged.wait_for(lock,std::chrono::milliseconds(100),[&]{return finished>=workerCount;});}auto now=std::chrono::steady_clock::now();double elapsed=std::chrono::duration<double>(now-sampled).count();auto current=j->done.load();if(elapsed>=.25){j->speed=(uint64_t)((current-prior)/elapsed);prior=current;sampled=now;}}
    for(auto& t:threads)t.join();
    if(j->stop)return true;
    if(unsupported){clearSegments(folder);j->lanes=1;transfer(j,true);return true;}
    if(!failure.empty())throw std::runtime_error(failure);
    j->assembled=0;j->assembling=true;j->speed=0;
    struct AssemblyReset {std::shared_ptr<Job> j;~AssemblyReset(){j->assembling=false;}} assemblyReset{j};
    {std::lock_guard<std::mutex> lock(j->mutex);j->error=L"Assembling downloaded segments…";}
    auto partial=fs::path(j->path+L".part");
    {
        File output(CreateFileW(partial.c_str(),GENERIC_WRITE,FILE_SHARE_READ,nullptr,CREATE_ALWAYS,FILE_ATTRIBUTE_NORMAL|FILE_FLAG_SEQUENTIAL_SCAN,nullptr));
        // Reserve disk allocation without extending/zero-filling the entire file first.
        FILE_ALLOCATION_INFO allocation{};allocation.AllocationSize.QuadPart=total;
        if(!SetFileInformationByHandle(output,FileAllocationInfo,&allocation,sizeof(allocation))&&GetLastError()==ERROR_DISK_FULL)fail("Reserve space for assembled file");
        std::vector<char> buffer(4*1024*1024);
        for(int i=0;i<count;++i){
            auto path=folder/(std::to_wstring(i)+L".part");auto wanted=ranges[i].end-ranges[i].start+1;
            if(fs::file_size(path)!=wanted)throw std::runtime_error("Segment size mismatch before assembly");
            File input(CreateFileW(path.c_str(),GENERIC_READ,FILE_SHARE_READ,nullptr,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL|FILE_FLAG_SEQUENTIAL_SCAN,nullptr));
            uint64_t copied=0;
            while(copied<wanted&&!j->stop){
                DWORD bytes=0;if(!ReadFile(input,buffer.data(),(DWORD)std::min<uint64_t>(buffer.size(),wanted-copied),&bytes,nullptr))fail("Read completed segment");
                if(!bytes)throw std::runtime_error("Completed segment ended early");
                DWORD offset=0;while(offset<bytes){DWORD wrote=0;if(!WriteFile(output,buffer.data()+offset,bytes-offset,&wrote,nullptr))fail("Assemble segments");if(!wrote)throw std::runtime_error("Assembly write made no progress");offset+=wrote;}
                copied+=bytes;j->assembled.fetch_add(bytes);
            }
            if(j->stop)return true;
        }
        if(!FlushFileBuffers(output))fail("Flush assembled file");
    }
    if(fs::file_size(partial)!=total)throw std::runtime_error("Assembled file size mismatch");
    finalizeDownload(j,partial);
    clearSegments(folder);j->status=Complete;{std::lock_guard<std::mutex> lock(j->mutex);j->error.clear();}return true;
}
