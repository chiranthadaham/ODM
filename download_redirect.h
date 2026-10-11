// Follow bounded, declarative HTML download redirects. Never execute page scripts.
std::wstring downloadRedirect(HINTERNET request,const std::wstring& original){
    std::string body;char buffer[8192];
    while(body.size()<262144){DWORD n=0;if(!WinHttpReadData(request,buffer,sizeof(buffer),&n))fail("Read download redirect");if(!n)break;body.append(buffer,n);}
    const std::regex meta(R"(<meta\b[^>]*>)",std::regex::icase);
    const std::regex attribute(R"attr(([a-zA-Z_-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))attr");
    for(std::sregex_iterator i(body.begin(),body.end(),meta),end;i!=end;++i){
        auto tag=i->str();std::string equiv,content;
        for(std::sregex_iterator a(tag.begin(),tag.end(),attribute);a!=end;++a){auto key=(*a)[1].str();std::transform(key.begin(),key.end(),key.begin(),::tolower);auto value=(*a)[2].matched?(*a)[2].str():(*a)[3].matched?(*a)[3].str():(*a)[4].str();if(key=="http-equiv")equiv=value;if(key=="content")content=value;}
        std::transform(equiv.begin(),equiv.end(),equiv.begin(),::tolower);if(equiv!="refresh")continue;
        std::smatch match;if(!std::regex_search(content,match,std::regex(R"(;\s*url\s*=\s*(.+))",std::regex::icase)))continue;
        auto target=match[1].str();for(size_t at=0;(at=target.find("&amp;",at))!=std::string::npos;)target.replace(at,5,"&");
        while(!target.empty()&&(target.back()==' '||target.back()=='\''||target.back()=='"'))target.pop_back();while(!target.empty()&&(target.front()==' '||target.front()=='\''||target.front()=='"'))target.erase(0,1);
        if(target.empty()||target.size()>16384||target.find_first_of("\r\n")!=std::string::npos)continue;
        // Resolve against the actual HTTP redirect destination, not the initial publisher URL.
        DWORD bytes=0;WinHttpQueryOption(request,WINHTTP_OPTION_URL,nullptr,&bytes);std::wstring base(bytes/sizeof(wchar_t),0);
        if(!bytes||!WinHttpQueryOption(request,WINHTTP_OPTION_URL,base.data(),&bytes))base=original;
        wchar_t combined[32768]{};DWORD size=32768;if(FAILED(UrlCombineW(base.c_str(),utf16(target).c_str(),combined,&size,0)))continue;
        std::wstring url=combined;if(url.rfind(L"http://",0)!=0&&url.rfind(L"https://",0)!=0)continue;
        if(base.rfind(L"https://",0)==0&&url.rfind(L"http://",0)==0)continue;
        if(url!=original)return url;
    }
    return L"";
}
