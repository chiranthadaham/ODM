// Optional transport for FTP, explicit proxies and authenticated file downloads.
// Uses Windows' curl; credentials travel through a private stdin pipe, never arguments.
#include <wincrypt.h>
struct NetworkOptions {std::wstring proxy,proxyUser,proxyPassword,serverOrigin,serverUser,serverPassword;};
NetworkOptions networkOptions;std::mutex networkMutex;
NetworkOptions networkSnapshot(){std::lock_guard<std::mutex> lock(networkMutex);return networkOptions;}
std::wstring urlOrigin(const std::wstring& value){auto end=value.find(L"://");if(end==std::wstring::npos)return L"";auto cut=value.find_first_of(L"/?#",end+3);auto origin=value.substr(0,cut);std::transform(origin.begin(),origin.end(),origin.begin(),::towlower);return origin;}
fs::path networkSettingsPath(){wchar_t custom[32768]{};auto n=GetEnvironmentVariableW(L"ODM_DATA_DIR",custom,32768);if(n&&n<32768)return fs::path(custom)/L"network.dat";wchar_t local[MAX_PATH]{};SHGetFolderPathW(nullptr,CSIDL_LOCAL_APPDATA|CSIDL_FLAG_CREATE,nullptr,0,local);return fs::path(local)/L"OpenDownloadManagerCpp/network.dat";}
void validateNetworkOptions(const NetworkOptions& value){
 for(auto& s:{value.proxy,value.proxyUser,value.proxyPassword,value.serverOrigin,value.serverUser,value.serverPassword})if(s.size()>2048||s.find_first_of(L"\r\n")!=std::wstring::npos||s.find(L'\0')!=std::wstring::npos)throw std::runtime_error("Invalid connection setting");
 if(!value.proxy.empty()){bool valid=false;for(auto prefix:{L"http://",L"https://",L"socks4://",L"socks4a://",L"socks5://",L"socks5h://"})if(value.proxy.rfind(prefix,0)==0)valid=true;if(!valid||value.proxy.find(L'@')!=std::wstring::npos)throw std::runtime_error("Use an HTTP/HTTPS or SOCKS proxy address; enter credentials separately");}
 if(!value.serverUser.empty()&&(value.serverOrigin.empty()||urlOrigin(value.serverOrigin)!=value.serverOrigin))throw std::runtime_error("Enter the exact server origin, for example https://example.com");
}
void storeNetworkOptions(const NetworkOptions& value){
 validateNetworkOptions(value);auto data=nlohmann::json{{"proxy",utf8(value.proxy)},{"proxyUser",utf8(value.proxyUser)},{"proxyPassword",utf8(value.proxyPassword)},{"serverOrigin",utf8(value.serverOrigin)},{"serverUser",utf8(value.serverUser)},{"serverPassword",utf8(value.serverPassword)}}.dump();
 DATA_BLOB input{(DWORD)data.size(),(BYTE*)data.data()},encrypted{};if(!CryptProtectData(&input,L"Open Download Manager connection settings",nullptr,nullptr,nullptr,CRYPTPROTECT_UI_FORBIDDEN,&encrypted))fail("Protect connection settings");
 auto path=networkSettingsPath();fs::create_directories(path.parent_path());auto temporary=path;temporary+=L".tmp";std::ofstream file(temporary,std::ios::binary|std::ios::trunc);file.write((char*)encrypted.pbData,encrypted.cbData);LocalFree(encrypted.pbData);file.close();if(!file)throw std::runtime_error("Cannot save connection settings");if(!MoveFileExW(temporary.c_str(),path.c_str(),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH))fail("Save connection settings");std::lock_guard<std::mutex> lock(networkMutex);networkOptions=value;
}
void loadNetworkOptions(){
 auto path=networkSettingsPath();if(!fs::exists(path))return;auto length=fs::file_size(path);if(length>65536)throw std::runtime_error("Invalid connection settings file");std::ifstream file(path,std::ios::binary);std::string data((std::istreambuf_iterator<char>(file)),{});DATA_BLOB input{(DWORD)data.size(),(BYTE*)data.data()},plain{};
 if(!CryptUnprotectData(&input,nullptr,nullptr,nullptr,nullptr,CRYPTPROTECT_UI_FORBIDDEN,&plain))fail("Read protected connection settings");std::string decoded((char*)plain.pbData,plain.cbData);SecureZeroMemory(plain.pbData,plain.cbData);LocalFree(plain.pbData);auto json=nlohmann::json::parse(decoded);NetworkOptions value{utf16(json.value("proxy","")),utf16(json.value("proxyUser","")),utf16(json.value("proxyPassword","")),utf16(json.value("serverOrigin","")),utf16(json.value("serverUser","")),utf16(json.value("serverPassword",""))};validateNetworkOptions(value);std::lock_guard<std::mutex> lock(networkMutex);networkOptions=value;
}

HINTERNET openHttpSession(const std::wstring& userAgent){
 auto value=networkSnapshot();auto proxy=value.proxy.rfind(L"http://",0)==0?value.proxy.substr(7):L"";
 return WinHttpOpen(userAgent.empty()?L"Mozilla/5.0 (Windows NT 10.0; Win64; x64) OpenDownloadManager/2.6":userAgent.c_str(),proxy.empty()?WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY:WINHTTP_ACCESS_TYPE_NAMED_PROXY,proxy.empty()?WINHTTP_NO_PROXY_NAME:proxy.c_str(),WINHTTP_NO_PROXY_BYPASS,0);
}
void sendHttpRequest(HINTERNET request,const std::wstring& headers){
 auto options=networkSnapshot();
 for(int retry=0;retry<4;++retry){
  if(!WinHttpSendRequest(request,headers.c_str(),(DWORD)-1,WINHTTP_NO_REQUEST_DATA,0,0,0)||!WinHttpReceiveResponse(request,nullptr))fail("HTTP request");
  DWORD status=0,length=sizeof(status);if(!WinHttpQueryHeaders(request,WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER,WINHTTP_HEADER_NAME_BY_INDEX,&status,&length,WINHTTP_NO_HEADER_INDEX))fail("HTTP status");if(status!=401&&status!=407)return;
  DWORD supported=0,first=0,target=0;if(!WinHttpQueryAuthSchemes(request,&supported,&first,&target))return;
  auto user=status==407?options.proxyUser:options.serverUser;auto password=status==407?options.proxyPassword:options.serverPassword;if(user.empty())return;
  if(status==401){DWORD size=0;WinHttpQueryOption(request,WINHTTP_OPTION_URL,nullptr,&size);std::wstring url(size/sizeof(wchar_t),L'\0');if(!size||!WinHttpQueryOption(request,WINHTTP_OPTION_URL,url.data(),&size))return;url.resize(wcslen(url.c_str()));if(urlOrigin(url)!=options.serverOrigin)return;}
  DWORD scheme=0;for(DWORD candidate:{WINHTTP_AUTH_SCHEME_NEGOTIATE,WINHTTP_AUTH_SCHEME_DIGEST,WINHTTP_AUTH_SCHEME_NTLM,WINHTTP_AUTH_SCHEME_BASIC})if(supported&candidate){scheme=candidate;break;}if(!scheme||!WinHttpSetCredentials(request,status==407?WINHTTP_AUTH_TARGET_PROXY:WINHTTP_AUTH_TARGET_SERVER,scheme,user.c_str(),password.c_str(),nullptr))return;
 }
}
