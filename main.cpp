// Original MIT-licensed implementation. Windows native C++17, Win32 and WinHTTP.
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#define NOMINMAX
#include <windows.h>
#include <winhttp.h>
#include <shlwapi.h>
#include <regex>
#include <commctrl.h>
#include <commdlg.h>
#include <shlobj.h>
#include <shellapi.h>
#include <uxtheme.h>
#include <gdiplus.h>
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <memory>
#include <mutex>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>
#include <condition_variable>
#include <array>
#include <set>
#include <map>
#include <future>
#include <deque>
#include "json.hpp"
namespace fs=std::filesystem;
enum Status {Queued,Downloading,Paused,Complete,Error,Canceled,Scheduled};
const wchar_t* names[]={L"Queued",L"Downloading",L"Paused",L"Complete",L"Error",L"Canceled",L"Scheduled"};
uint64_t downloadAddedNow(){FILETIME time;GetSystemTimeAsFileTime(&time);return (uint64_t(time.dwHighDateTime)<<32)|time.dwLowDateTime;}
struct Job {
    std::wstring url,path,schedule,etag,modified,error,userAgent,referer;
    bool completionNotified=false,probeOnly=false;
    std::mutex mutex,paceMutex;
    int pacedLimit=0;uint64_t paceEpoch=0;std::chrono::steady_clock::time_point nextByteTime{};
    std::atomic<int> status{Queued};
    std::atomic<bool> stop{false},active{false},resumeRequested{false},preparePath{false};
    std::atomic<uint64_t> done{0},total{0},speed{0},assembled{0};
    std::atomic<bool> assembling{false};
    std::atomic<int> lanes{1};
    uint64_t pauseAfter=0,addedAt=downloadAddedNow();
};
std::vector<std::shared_ptr<Job>> jobs;
struct RunningWorker {std::shared_ptr<Job> job;std::thread thread;};
std::vector<RunningWorker> workers;
std::atomic<int> limitKiB{0},connectionsPerFile{8};int maxActive=2;
fs::path state;
bool closing=false;
std::mutex destinationMutex;
std::set<std::wstring> reservedPaths;
void prepareDestination(const std::shared_ptr<Job>& j) {
    if(!j->preparePath)return;
    std::wstring value;{std::lock_guard<std::mutex> lock(j->mutex);value=j->path;}
    std::lock_guard<std::mutex> reservation(destinationMutex);
    fs::path original(value), dest(original);fs::create_directories(original.parent_path());
    for(int n=1;reservedPaths.count(dest.wstring())||fs::exists(dest)||fs::exists(dest.wstring()+L".part")||fs::exists(dest.wstring()+L".segments");++n)
        dest=original.parent_path()/(original.stem().wstring()+L" ("+std::to_wstring(n)+L")"+original.extension().wstring());
    reservedPaths.insert(dest.wstring());
    {std::lock_guard<std::mutex> lock(j->mutex);j->path=dest.wstring();}j->preparePath=false;
}
HWND window,list,urlBox,folderBox,scheduleBox,limitBox,concurrencyBox,detail;
HFONT font;
std::wstring text(HWND h) {int n=GetWindowTextLengthW(h);std::wstring s(n+1,L'\0');GetWindowTextW(h,s.data(),n+1);s.resize(n);return s;}
std::wstring utf16(const std::string& s) {int n=MultiByteToWideChar(CP_UTF8,0,s.data(),(int)s.size(),nullptr,0);std::wstring w(n,0);MultiByteToWideChar(CP_UTF8,0,s.data(),(int)s.size(),w.data(),n);return w;}
std::string utf8(const std::wstring& w) {int n=WideCharToMultiByte(CP_UTF8,0,w.data(),(int)w.size(),nullptr,0,nullptr,nullptr);std::string s(n,0);WideCharToMultiByte(CP_UTF8,0,w.data(),(int)w.size(),s.data(),n,nullptr,nullptr);return s;}
std::wstring nowLocal() {SYSTEMTIME t;GetLocalTime(&t);wchar_t b[32];swprintf(b,32,L"%04u-%02u-%02u %02u:%02u",t.wYear,t.wMonth,t.wDay,t.wHour,t.wMinute);return b;}
void fail(const char* action) {throw std::runtime_error(std::string(action)+" (Windows error "+std::to_string(GetLastError())+")");}
struct Internet {HINTERNET h=nullptr;Internet(HINTERNET value):h(value){if(!h)fail("Network operation");}~Internet(){if(h)WinHttpCloseHandle(h);}operator HINTERNET()const{return h;}Internet(const Internet&)=delete;};
struct File {HANDLE h=INVALID_HANDLE_VALUE;File(HANDLE value):h(value){if(h==INVALID_HANDLE_VALUE)fail("File operation");}~File(){CloseHandle(h);}operator HANDLE()const{return h;}};
// A destination may appear while a long transfer is running. Never overwrite it.
void finalizeDownload(const std::shared_ptr<Job>& j,const fs::path& partial){
    std::lock_guard<std::mutex> reservation(destinationMutex);
    fs::path original;{std::lock_guard<std::mutex> lock(j->mutex);original=j->path;}
    for(int n=0;n<10000;++n){
        auto dest=n?original.parent_path()/(original.stem().wstring()+L" ("+std::to_wstring(n)+L")"+original.extension().wstring()):original;
        if(n&&(reservedPaths.count(dest.wstring())||fs::exists(dest.wstring()+L".part")||fs::exists(dest.wstring()+L".segments")))continue;
        if(MoveFileExW(partial.c_str(),dest.c_str(),MOVEFILE_WRITE_THROUGH)){
            reservedPaths.insert(dest.wstring());std::lock_guard<std::mutex> lock(j->mutex);j->path=dest.wstring();return;
        }
        auto error=GetLastError();if(error!=ERROR_ALREADY_EXISTS&&error!=ERROR_FILE_EXISTS){SetLastError(error);fail("Finalize download");}
    }
    throw std::runtime_error("Cannot find an unused destination filename");
}
std::wstring header(HINTERNET request,DWORD field) {DWORD bytes=0;WinHttpQueryHeaders(request,field,WINHTTP_HEADER_NAME_BY_INDEX,nullptr,&bytes,WINHTTP_NO_HEADER_INDEX);if(GetLastError()!=ERROR_INSUFFICIENT_BUFFER)return L"";std::wstring value(bytes/sizeof(wchar_t),0);if(!WinHttpQueryHeaders(request,field,WINHTTP_HEADER_NAME_BY_INDEX,value.data(),&bytes,WINHTTP_NO_HEADER_INDEX))return L"";value.resize(bytes/sizeof(wchar_t));while(!value.empty()&&value.back()==0)value.pop_back();return value;}
#include "network_options.h"
uint64_t number(const std::wstring& s) {if(s.empty())return 0;size_t end=0;auto n=std::stoull(s,&end);if(end!=s.size())throw std::runtime_error("Invalid numeric response header");return n;}
struct PermanentDownloadError:std::runtime_error{using std::runtime_error::runtime_error;};
void httpFailure(DWORD status){
    if(status==401||status==403)throw PermanentDownloadError("HTTP "+std::to_string(status)+": the website refused this download. Open the website in your browser, complete verification, then click its file download button.");
    if(status==404||status==410)throw PermanentDownloadError("HTTP "+std::to_string(status)+": this file link is missing or has expired. Get a fresh download link from the website.");
    if(status==509)throw PermanentDownloadError("HTTP 509: this website has reached a bandwidth limit. Try again later or use another download source.");
    if(status==429)throw PermanentDownloadError("HTTP 429: the website is limiting downloads. Wait before resuming this file.");
    throw std::runtime_error("HTTP status "+std::to_string(status));
}
#include "download_redirect.h"
DWORD transferBufferSize(){int limit=limitKiB.load();if(limit<=0)return 262144;return (DWORD)std::clamp<uint64_t>((uint64_t)limit*1024/std::max(1,connectionsPerFile.load())/10,1024,262144);}
void resetPacing(const std::shared_ptr<Job>& j){std::lock_guard<std::mutex> lock(j->paceMutex);j->nextByteTime=std::chrono::steady_clock::now();++j->paceEpoch;}
void paceDownload(const std::shared_ptr<Job>& j,uint64_t bytes,const std::atomic<bool>* abort=nullptr){
 for(;;){int limit=limitKiB.load();if(limit<=0)return;std::chrono::steady_clock::time_point target;uint64_t epoch;
  {std::lock_guard<std::mutex> lock(j->paceMutex);auto now=std::chrono::steady_clock::now();if(j->pacedLimit!=limit){j->pacedLimit=limit;j->nextByteTime=now;++j->paceEpoch;}j->nextByteTime=std::max(now,j->nextByteTime)+std::chrono::duration_cast<std::chrono::steady_clock::duration>(std::chrono::duration<double>(bytes/(limit*1024.0)));target=j->nextByteTime;epoch=j->paceEpoch;}
  while(!j->stop&&!(abort&&abort->load())&&std::chrono::steady_clock::now()<target){if(limitKiB.load()!=limit)break;{std::lock_guard<std::mutex> lock(j->paceMutex);if(epoch!=j->paceEpoch)break;}Sleep(25);}
  if(j->stop||(abort&&abort->load())||limitKiB.load()==limit)return;
 }
}
void transfer(const std::shared_ptr<Job>& j,bool forceRestart=false,HINTERNET preparedResponse=nullptr,std::wstring resolvedUrl=L"",int redirectDepth=0) {
    resetPacing(j);
    const auto url=resolvedUrl.empty()?j->url:resolvedUrl;
    URL_COMPONENTS parts{};parts.dwStructSize=sizeof(parts);parts.dwSchemeLength=parts.dwHostNameLength=parts.dwUrlPathLength=parts.dwExtraInfoLength=(DWORD)-1;
    if(!WinHttpCrackUrl(url.c_str(),0,0,&parts)||(parts.nScheme!=INTERNET_SCHEME_HTTP&&parts.nScheme!=INTERNET_SCHEME_HTTPS))throw std::runtime_error("Use an HTTP or HTTPS URL");
    std::wstring host(parts.lpszHostName,parts.dwHostNameLength),resource(parts.lpszUrlPath,parts.dwUrlPathLength);
    if(parts.dwExtraInfoLength)resource.append(parts.lpszExtraInfo,parts.dwExtraInfoLength);if(resource.empty())resource=L"/";
    auto partial=fs::path(j->path+L".part");uint64_t offset=fs::exists(partial)?fs::file_size(partial):0;
    std::wstring validator;{std::lock_guard<std::mutex> lock(j->mutex);validator=j->etag.empty()?j->modified:j->etag;}
    if(validator.empty()||forceRestart)offset=0;
    Internet session(openHttpSession(j->userAgent));
    WinHttpSetTimeouts(session,10000,10000,10000,30000);
    Internet connection(WinHttpConnect(session,host.c_str(),parts.nPort,0));
    Internet ownedRequest(WinHttpOpenRequest(connection,L"GET",resource.c_str(),nullptr,WINHTTP_NO_REFERER,WINHTTP_DEFAULT_ACCEPT_TYPES,parts.nScheme==INTERNET_SCHEME_HTTPS?WINHTTP_FLAG_SECURE:0));
    // A range probe returning 200 already contains the full response. Consume it once.
    HINTERNET request=preparedResponse?preparedResponse:ownedRequest.h;
    if(preparedResponse)offset=0;
    DWORD redirect=WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;WinHttpSetOption(request,WINHTTP_OPTION_REDIRECT_POLICY,&redirect,sizeof(redirect));
    std::wstring headers=L"Accept-Encoding: identity\r\n";if(!j->referer.empty())headers+=L"Referer: "+j->referer+L"\r\n";
    if(offset)headers+=L"Range: bytes="+std::to_wstring(offset)+L"-\r\nIf-Range: "+validator+L"\r\n";
    if(!preparedResponse)sendHttpRequest(request,headers);
    DWORD status=0,bytes=sizeof(status);if(!WinHttpQueryHeaders(request,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,WINHTTP_HEADER_NAME_BY_INDEX,&status,&bytes,WINHTTP_NO_HEADER_INDEX))fail("HTTP status");
    if(status==416&&offset&&!forceRestart){transfer(j,true);return;}
    uint64_t total=0,expected=0;
    if(status==206) {
        auto range=header(request,WINHTTP_QUERY_CONTENT_RANGE);unsigned long long start=0,end=0,length=0;wchar_t extra=0;
        if(swscanf(range.c_str(),L"bytes %llu-%llu/%llu%c",&start,&end,&length,&extra)!=3||start!=offset||end<start||end>=length)throw std::runtime_error("Invalid server byte range");
        total=length;expected=end-start+1;
    } else if(status==200) {offset=0;total=number(header(request,WINHTTP_QUERY_CONTENT_LENGTH));expected=total;}
    else httpFailure(status);
    auto contentType=header(request,WINHTTP_QUERY_CONTENT_TYPE);std::transform(contentType.begin(),contentType.end(),contentType.begin(),::towlower);
    auto extension=fs::path(j->path).extension().wstring();std::transform(extension.begin(),extension.end(),extension.begin(),::towlower);
    auto disposition=header(request,WINHTTP_QUERY_CONTENT_DISPOSITION);std::transform(disposition.begin(),disposition.end(),disposition.begin(),::towlower);
    if((contentType.find(L"text/html")!=std::wstring::npos||contentType.find(L"application/xhtml")!=std::wstring::npos)&&extension!=L".html"&&extension!=L".htm"&&extension!=L".xhtml"&&disposition.rfind(L"attachment",0)!=0)
        {auto next=redirectDepth<4?downloadRedirect(request,url):L"";if(!next.empty()){transfer(j,forceRestart,nullptr,next,redirectDepth+1);return;}
        throw PermanentDownloadError("The link returned a web page instead of the requested file. It may be a download redirect or browser verification page. Open it in Firefox and send the final file link.");}
    auto encoding=header(request,WINHTTP_QUERY_CONTENT_ENCODING);if(!encoding.empty()&&encoding!=L"identity")throw std::runtime_error("Unexpected compressed response; safe byte resume unavailable");
    auto etag=header(request,WINHTTP_QUERY_ETAG);if(etag.rfind(L"W/",0)==0)etag.clear();
    fs::create_directories(fs::path(j->path).parent_path());
    File output(CreateFileW(partial.c_str(),GENERIC_WRITE,FILE_SHARE_READ,nullptr,offset?OPEN_EXISTING:CREATE_ALWAYS,FILE_ATTRIBUTE_NORMAL,nullptr));
    LARGE_INTEGER at;at.QuadPart=offset;if(!SetFilePointerEx(output,at,nullptr,FILE_BEGIN))fail("Seek partial file");
    auto lastModified=header(request,WINHTTP_QUERY_LAST_MODIFIED);{std::lock_guard<std::mutex> lock(j->mutex);j->etag=etag;j->modified=lastModified;j->error.clear();}
    j->done=offset;j->total=total;uint64_t received=0,sample=0;
    auto began=std::chrono::steady_clock::now(),sampleAt=began;
    char buffer[262144];
    while(!j->stop) {
        DWORD count=0;if(!WinHttpReadData(request,buffer,transferBufferSize(),&count))fail("Read response");if(!count)break;
        DWORD wrote=0;if(!WriteFile(output,buffer,count,&wrote,nullptr)||wrote!=count)fail("Write download");
        received+=count;sample+=count;j->done=offset+received;
        auto now=std::chrono::steady_clock::now();double elapsed=std::chrono::duration<double>(now-sampleAt).count();
        if(elapsed>=.25){j->speed=(uint64_t)(sample/elapsed);sample=0;sampleAt=now;}
        if(j->pauseAfter&&received>=j->pauseAfter){j->status=Paused;j->stop=true;}
        paceDownload(j,count);
    }
    if(j->stop)return;
    if((expected&&received!=expected)||(total&&offset+received!=total))throw std::runtime_error("Incomplete response: byte count does not match");
    if(!FlushFileBuffers(output))fail("Flush download");
    // Close the file before atomically moving it. MOVEFILE_REPLACE_EXISTING is deliberately absent.
    CloseHandle(output.h);output.h=INVALID_HANDLE_VALUE;
    finalizeDownload(j,partial);
    j->total=j->done.load();j->status=Complete;
}
#include "segmented.h"
#include "runtime.h"
#include "video.h"
#include "network.h"
void worker(std::shared_ptr<Job> j) {
    j->active=true;
    int attempts=needsVideoExtractor(j->url)?1:3;
    for(int attempt=0;attempt<attempts&&!j->stop;++attempt) {
        try {prepareDestination(j);if(j->stop)break;if(needsVideoExtractor(j->url))videoTransfer(j);else if(needsNetworkTransport(j->url))curlTransfer(j);else if(!segmentedTransfer(j))transfer(j);break;}
        catch(const std::exception& e) {
            {std::lock_guard<std::mutex> lock(j->mutex);j->error=utf16(e.what());}
            if(dynamic_cast<const PermanentDownloadError*>(&e)||attempt==attempts-1){if(!j->stop)j->status=Error;break;}
            for(int n=0;n<(1<<attempt)*20&&!j->stop;++n)Sleep(100);
        }
    }
    j->speed=0;j->active=false;
}
void writeString(std::ostream& out,const std::wstring& w) {auto s=utf8(w);uint32_t n=(uint32_t)s.size();out.write((char*)&n,4);out.write(s.data(),n);}
std::wstring readString(std::istream& in) {uint32_t n=0;if(!in.read((char*)&n,4)||n>1024*1024)throw std::runtime_error("Invalid queue string");std::string s(n,0);if(!in.read(s.data(),n))throw std::runtime_error("Truncated queue");return utf16(s);}
struct QueueWriter {
    std::mutex mutex;std::condition_variable ready;std::string pending,error;
    bool hasPending=false,busy=false,quit=false;std::thread thread;
    QueueWriter():thread([this]{run();}){}
    ~QueueWriter(){{std::lock_guard<std::mutex> lock(mutex);quit=true;}ready.notify_one();if(thread.joinable())thread.join();}
    void submit(std::string bytes){{std::lock_guard<std::mutex> lock(mutex);pending=std::move(bytes);hasPending=true;}ready.notify_one();}
    bool idle(){std::lock_guard<std::mutex> lock(mutex);return !busy&&!hasPending;}
    std::string failure(){std::lock_guard<std::mutex> lock(mutex);return error;}
    void run(){for(;;){std::string bytes;{std::unique_lock<std::mutex> lock(mutex);ready.wait(lock,[&]{return quit||hasPending;});if(!hasPending&&quit)return;bytes=std::move(pending);hasPending=false;busy=true;}
        std::string problem;try{auto temp=state;temp+=L".tmp";std::ofstream out(temp,std::ios::binary|std::ios::trunc);if(!out)throw std::runtime_error("Cannot write queue file");out.write(bytes.data(),bytes.size());out.flush();if(!out)throw std::runtime_error("Queue write failed");out.close();if(!MoveFileExW(temp.c_str(),state.c_str(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH))fail("Save queue");}catch(const std::exception& e){problem=e.what();}
        {std::lock_guard<std::mutex> lock(mutex);error=problem;busy=false;}
    }}
};
std::unique_ptr<QueueWriter> queueWriter;
void save() {
    std::ostringstream out(std::ios::binary);out.write("ODMCPP03",8);uint32_t count=(uint32_t)jobs.size();out.write((char*)&count,4);out.write((char*)&maxActive,4);int limit=limitKiB;out.write((char*)&limit,4);int connections=connectionsPerFile;out.write((char*)&connections,4);
    for(auto& j:jobs) {
        std::array<std::wstring,5> fields;{std::lock_guard<std::mutex> lock(j->mutex);fields={j->url,j->path,j->schedule,j->etag,j->modified};}
        for(auto& field:fields)writeString(out,field);
        int status=j->status;if(status==Downloading)status=Paused;out.write((char*)&status,4);
        uint64_t done=j->done,total=j->total;out.write((char*)&done,8);out.write((char*)&total,8);out.write((char*)&j->addedAt,8);
    }
    queueWriter->submit(out.str());
}
void load() {
    if(!fs::exists(state))return;
    std::ifstream in(state,std::ios::binary);char magic[8];uint32_t count=0;int limit=0;
    if(!in.read(magic,8)||(std::string(magic,8)!="ODMCPP01"&&std::string(magic,8)!="ODMCPP02"&&std::string(magic,8)!="ODMCPP03")||!in.read((char*)&count,4)||count>10000||!in.read((char*)&maxActive,4)||!in.read((char*)&limit,4))throw std::runtime_error("Invalid queue header");
    if(std::string(magic,8)!="ODMCPP01"){int connections=4;if(!in.read((char*)&connections,4))throw std::runtime_error("Invalid connection setting");connectionsPerFile=std::clamp(connections,1,16);}
    maxActive=std::clamp(maxActive,1,8);limitKiB=std::max(0,limit);
    std::vector<std::shared_ptr<Job>> loaded;
    for(uint32_t i=0;i<count;++i) {
        auto j=std::make_shared<Job>();for(auto* s:{&j->url,&j->path,&j->schedule,&j->etag,&j->modified})*s=readString(in);
        int status=0;uint64_t done=0,total=0;
        if(!in.read((char*)&status,4)||status<0||status>Scheduled||!in.read((char*)&done,8)||!in.read((char*)&total,8))throw std::runtime_error("Invalid queue entry");
        j->addedAt=0;if(std::string(magic,8)=="ODMCPP03"&&!in.read((char*)&j->addedAt,8))throw std::runtime_error("Invalid download date");
        j->status=(status==Downloading||status==Scheduled)?Paused:status;j->schedule.clear();j->completionNotified=true;j->done=done;j->total=total;loaded.push_back(j);
    }
    jobs=std::move(loaded);
}
std::wstring defaultName(std::wstring url) {
    auto cut=url.find_first_of(L"?#");if(cut!=std::wstring::npos)url.resize(cut);auto slash=url.find_last_of(L'/');auto name=slash==std::wstring::npos?url:url.substr(slash+1);
    for(auto& c:name)if(c<32||std::wstring(L"<>:\"/\\|?*").find(c)!=std::wstring::npos)c=L'_';
    while(!name.empty()&&(name.back()==L'.'||name.back()==L' '))name.pop_back();if(name.empty())name=L"download.bin";if(name.size()>150)name.resize(150);
    auto stem=fs::path(name).stem().wstring();std::transform(stem.begin(),stem.end(),stem.begin(),::towupper);
    if(stem==L"CON"||stem==L"PRN"||stem==L"AUX"||stem==L"NUL"||(stem.size()==4&&(stem.substr(0,3)==L"COM"||stem.substr(0,3)==L"LPT")&&stem[3]>='1'&&stem[3]<='9'))name=L"_"+name;
    return name;
}
std::set<std::wstring> queuePaths;std::map<std::wstring,int> nextSuffix;
std::wstring pathKey(const fs::path& path){auto key=path.lexically_normal().wstring();std::transform(key.begin(),key.end(),key.begin(),::towlower);return key;}
void add(std::wstring url,fs::path folder,std::wstring schedule=L"") {
    if(url.size()>16384||url.find_first_of(L"\r\n")!=std::wstring::npos||url.find(L'\0')!=std::wstring::npos)throw std::runtime_error("Invalid URL");
    if(url.rfind(L"http://",0)!=0&&url.rfind(L"https://",0)!=0&&url.rfind(L"ftp://",0)!=0&&url.rfind(L"ftps://",0)!=0)throw std::runtime_error("Use an HTTP, HTTPS or FTP URL");
    if(jobs.size()>=10000)throw std::runtime_error("The queue is full. Clear finished downloads first.");
    if(folder.empty()||!folder.is_absolute())throw std::runtime_error("Choose an absolute download folder");auto name=needsVideoExtractor(url)?L"Video "+std::to_wstring(GetTickCount64())+L".mp4":defaultName(url);auto dest=folder/name;
    if(queuePaths.empty())for(auto& j:jobs){std::lock_guard<std::mutex> lock(j->mutex);queuePaths.insert(pathKey(j->path));}
    auto baseKey=pathKey(dest);int n=std::max(1,nextSuffix[baseKey]);
    while(queuePaths.count(pathKey(dest)))dest=folder/(fs::path(name).stem().wstring()+L" ("+std::to_wstring(n++)+L")"+fs::path(name).extension().wstring());
    nextSuffix[baseKey]=n;queuePaths.insert(pathKey(dest));
    auto j=std::make_shared<Job>();j->url=url;j->path=dest.wstring();j->preparePath=true;j->schedule.clear();j->status=Queued;jobs.push_back(j);
}
#include "browser_bridge.h"
#include "ui.h"
int WINAPI wWinMain(HINSTANCE instance,HINSTANCE,LPWSTR,int show) {
    int argc=0;auto argv=CommandLineToArgvW(GetCommandLineW(),&argc);
    try{loadNetworkOptions();}catch(...){LocalFree(argv);return 1;}
    if((argc>=2&&(std::wstring(argv[1])==L"--native-host"||std::wstring(argv[1]).rfind(L"chrome-extension://",0)==0))||(argc>=3&&std::wstring(argv[2])==L"opendownloadmanager@local")){LocalFree(argv);return nativeHost();}
    if(argc>=4&&std::wstring(argv[1])==L"--download") {
        auto j=std::make_shared<Job>();j->url=argv[2];j->path=fs::absolute(argv[3]).wstring();j->status=Downloading;
        const auto reportPath=j->path+L".result.txt";
        for(int i=4;i+1<argc;i+=2){auto key=std::wstring(argv[i]);if(key==L"--etag")j->etag=argv[i+1];else if(key==L"--pause-after")j->pauseAfter=std::stoull(argv[i+1]);else if(key==L"--connections")connectionsPerFile=std::clamp(std::stoi(argv[i+1]),1,16);else if(key==L"--tools-dir")toolsOverride=argv[i+1];else if(key==L"--limit")limitKiB=std::max(0,std::stoi(argv[i+1]));}
        worker(j);std::ofstream report{fs::path(reportPath)};report<<utf8(names[j->status])<<"\n"<<utf8(j->error)<<"\nPath: "<<utf8(j->path)<<"\nConnections: "<<j->lanes.load();LocalFree(argv);return j->status==Complete?0:j->status==Paused?2:1;
    }
    bool background=argc>=2&&std::wstring(argv[1])==L"--background";
    LocalFree(argv);
    HANDLE singleton=CreateMutexW(nullptr,TRUE,L"Local\\OpenDownloadManagerCpp");if(GetLastError()==ERROR_ALREADY_EXISTS){auto existing=FindWindowW(L"OpenDownloadManagerCpp",nullptr);if(existing&&!background){ShowWindow(existing,SW_RESTORE);SetForegroundWindow(existing);}CloseHandle(singleton);return 0;}
    try {
        wchar_t local[MAX_PATH];if(FAILED(SHGetFolderPathW(nullptr,CSIDL_LOCAL_APPDATA|CSIDL_FLAG_CREATE,nullptr,0,local)))throw std::runtime_error("Cannot locate local application data");
        wchar_t customData[32768]{};DWORD customLength=GetEnvironmentVariableW(L"ODM_DATA_DIR",customData,32768);
        auto folder=(customLength>0&&customLength<32768)?fs::path(customData):fs::path(local)/L"OpenDownloadManagerCpp";fs::create_directories(folder);state=folder/L"queue.dat";
        try{load();}catch(const std::exception&){auto backup=state;backup+=L".corrupt-"+std::to_wstring(GetTickCount64());fs::rename(state,backup);jobs.clear();MessageBoxW(nullptr,L"An unreadable queue was preserved in a backup file. Starting a new queue.",L"Open Download Manager",MB_OK|MB_ICONWARNING);}
        queueWriter=std::make_unique<QueueWriter>();
        CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);INITCOMMONCONTROLSEX cc{sizeof(cc),ICC_LISTVIEW_CLASSES};InitCommonControlsEx(&cc);
        WNDCLASSW cls{};cls.lpfnWndProc=procedure;cls.hInstance=instance;cls.lpszClassName=L"OpenDownloadManagerCpp";cls.hCursor=LoadCursorW(nullptr,IDC_ARROW);cls.hIcon=LoadIconW(instance,MAKEINTRESOURCEW(1));cls.hbrBackground=(HBRUSH)(COLOR_WINDOW+1);RegisterClassW(&cls);
        auto h=CreateWindowExW(0,cls.lpszClassName,L"Open Download Manager",WS_OVERLAPPEDWINDOW|WS_CLIPCHILDREN,CW_USEDEFAULT,CW_USEDEFAULT,1200,850,nullptr,nullptr,instance,nullptr);if(!h)fail("Create application window");ShowWindow(h,background?SW_HIDE:show);UpdateWindow(h);
        MSG msg;while(GetMessageW(&msg,nullptr,0,0)>0){if(!workflowDialogMessage(msg)&&!IsDialogMessageW(h,&msg)){TranslateMessage(&msg);DispatchMessageW(&msg);}}
        queueWriter.reset();CoUninitialize();CloseHandle(singleton);return 0;
    }catch(const std::exception& e){MessageBoxW(nullptr,utf16(e.what()).c_str(),L"Open Download Manager",MB_OK|MB_ICONERROR);CloseHandle(singleton);return 1;}
}
