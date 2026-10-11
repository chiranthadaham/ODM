// Lazily unpack the embedded video runtime into a versioned, verified local cache.
#include <setupapi.h>
#include <bcrypt.h>
#include <map>
std::mutex runtimeMutex;
fs::path cachedRuntime;
std::string fileSha256(const fs::path& path) {
    std::ifstream in(path,std::ios::binary);if(!in)return "";
    BCRYPT_ALG_HANDLE algorithm=nullptr;BCRYPT_HASH_HANDLE hash=nullptr;
    if(BCryptOpenAlgorithmProvider(&algorithm,BCRYPT_SHA256_ALGORITHM,nullptr,0)<0)throw std::runtime_error("Cannot initialize runtime verification");
    DWORD objectLength=0,result=0;BCryptGetProperty(algorithm,BCRYPT_OBJECT_LENGTH,(PUCHAR)&objectLength,sizeof(objectLength),&result,0);
    std::vector<unsigned char> object(objectLength),buffer(1024*1024);unsigned char digest[32];
    if(BCryptCreateHash(algorithm,&hash,object.data(),objectLength,nullptr,0,0)<0){BCryptCloseAlgorithmProvider(algorithm,0);throw std::runtime_error("Cannot verify runtime file");}
    bool okay=true;
    while(in){in.read((char*)buffer.data(),buffer.size());if(in.gcount()&&BCryptHashData(hash,buffer.data(),(ULONG)in.gcount(),0)<0){okay=false;break;}}
    if(!in.eof())okay=false;if(BCryptFinishHash(hash,digest,32,0)<0)okay=false;
    BCryptDestroyHash(hash);BCryptCloseAlgorithmProvider(algorithm,0);if(!okay)return "";
    static const char hex[]="0123456789abcdef";std::string out;for(auto byte:digest){out+=hex[byte>>4];out+=hex[byte&15];}return out;
}
struct CabinetContext {fs::path folder;std::map<std::wstring,std::string>* expected;DWORD error=0;};
UINT CALLBACK cabinetCallback(PVOID pointer,UINT event,UINT_PTR param1,UINT_PTR) {
    auto context=(CabinetContext*)pointer;
    if(event==SPFILENOTIFY_FILEINCABINET){
        auto info=(FILE_IN_CABINET_INFO_W*)param1;std::wstring name=info->NameInCabinet;
        if(name!=fs::path(name).filename().wstring()||!context->expected->count(name)){context->error=ERROR_INVALID_NAME;return FILEOP_ABORT;}
        auto path=(context->folder/name).wstring();if(path.size()>=MAX_PATH){context->error=ERROR_FILENAME_EXCED_RANGE;return FILEOP_ABORT;}
        wcscpy(info->FullTargetName,path.c_str());return FILEOP_DOIT;
    }
    if(event==SPFILENOTIFY_FILEEXTRACTED){auto info=(FILEPATHS_W*)param1;if(info->Win32Error){context->error=info->Win32Error;return info->Win32Error;}}
    if(event==SPFILENOTIFY_NEEDNEWCABINET){context->error=ERROR_INVALID_DATA;return ERROR_INVALID_DATA;}
    return NO_ERROR;
}
fs::path embeddedRuntime() {
    std::lock_guard<std::mutex> lock(runtimeMutex);if(!cachedRuntime.empty())return cachedRuntime;
    auto module=GetModuleHandleW(nullptr);auto indexResource=FindResourceW(module,MAKEINTRESOURCEW(102),MAKEINTRESOURCEW(10));auto cabResource=FindResourceW(module,MAKEINTRESOURCEW(101),MAKEINTRESOURCEW(10));
    if(!indexResource||!cabResource)return {};
    auto indexHandle=LoadResource(module,indexResource);auto indexData=(const char*)LockResource(indexHandle);auto indexLength=SizeofResource(module,indexResource);
    std::istringstream lines(std::string(indexData,indexLength));std::string version;std::getline(lines,version);if(!version.empty()&&version.back()=='\r')version.pop_back();
    if(version.size()!=20||version.find_first_not_of("0123456789abcdef")!=std::string::npos)throw std::runtime_error("Invalid bundled runtime index");
    std::map<std::wstring,std::string> expected;std::string digest,name;while(lines>>digest>>name){if(digest.size()!=64||name.find_first_of("/\\:")!=std::string::npos)throw std::runtime_error("Invalid bundled runtime filename");expected[utf16(name)]=digest;}
    if(expected.empty())throw std::runtime_error("Empty bundled runtime index");
    auto mutexName=L"Local\\OpenDownloadManagerRuntime-"+utf16(version);HANDLE mutex=CreateMutexW(nullptr,FALSE,mutexName.c_str());if(!mutex)fail("Lock video runtime cache");
    DWORD waited=WaitForSingleObject(mutex,120000);if(waited!=WAIT_OBJECT_0&&waited!=WAIT_ABANDONED){CloseHandle(mutex);throw std::runtime_error("Video runtime preparation is busy; try again shortly");}
    struct Unlock {HANDLE h;~Unlock(){ReleaseMutex(h);CloseHandle(h);}} unlock{mutex};
    wchar_t local[MAX_PATH];if(FAILED(SHGetFolderPathW(nullptr,CSIDL_LOCAL_APPDATA|CSIDL_FLAG_CREATE,nullptr,0,local)))throw std::runtime_error("Cannot locate video runtime cache");
    auto folder=fs::path(local)/L"OpenDownloadManagerCpp"/(L"tools-"+utf16(version));fs::create_directories(folder);
    auto valid=[&](){for(auto& entry:expected)if(fileSha256(folder/entry.first)!=entry.second)return false;return true;};
    if(!valid()){
        auto cabHandle=LoadResource(module,cabResource);auto data=(const char*)LockResource(cabHandle);DWORD bytes=SizeofResource(module,cabResource);if(!data||!bytes)throw std::runtime_error("Cannot read bundled video support");
        auto archive=folder/L"runtime.cab";{std::ofstream out(archive,std::ios::binary|std::ios::trunc);out.write(data,bytes);out.flush();if(!out)throw std::runtime_error("Not enough space to prepare video support");}
        CabinetContext context{folder,&expected,0};BOOL extracted=SetupIterateCabinetW(archive.c_str(),0,cabinetCallback,&context);DWORD extractError=GetLastError();std::error_code ec;fs::remove(archive,ec);
        if(!extracted||context.error)throw std::runtime_error("Cannot unpack video support (Windows error "+std::to_string(context.error?context.error:extractError)+")");
        if(!valid())throw std::runtime_error("Video support verification failed after unpacking");
    }
    cachedRuntime=folder;return folder;
}
