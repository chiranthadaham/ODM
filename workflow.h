// Modeless dialogs keep the queue, network progress and browser messages responsive.
constexpr UINT TrayMessage=WM_APP+70, HandoffDecision=WM_APP+71;
HWND addDialog=nullptr,downloadDialog=nullptr,settingsDialog=nullptr,completionDialog=nullptr,networkDialog=nullptr,renewDialog=nullptr;
std::shared_ptr<Job> renewingJob;
struct DownloadChoice {std::wstring url;DWORD id=0;std::wstring filename,userAgent,referer;bool verified=true,startRequested=false;};
std::deque<DownloadChoice> downloadChoices;
std::map<HWND,std::shared_ptr<Job>> progressWindows;
std::map<DWORD,int> handoffDecisions;
std::deque<std::shared_ptr<Job>> completionChoices;
NOTIFYICONDATAW tray{};bool trayInstalled=false,scrollToNewest=true;
void update();
void showApp(){ShowWindow(window,SW_RESTORE);SetForegroundWindow(window);}
HWND dialogControl(HWND parent,const wchar_t* cls,const wchar_t* caption,int id,DWORD style,int x,int y,int w,int h){
    auto c=CreateWindowExW(0,cls,caption,WS_CHILD|WS_VISIBLE|style|(wcscmp(cls,L"BUTTON")==0?BS_OWNERDRAW:0),x,y,w,h,parent,(HMENU)(INT_PTR)id,GetModuleHandleW(nullptr),nullptr);
    SendMessageW(c,WM_SETFONT,(WPARAM)font,TRUE);if(wcscmp(cls,L"EDIT")==0)styleEdit(c);return c;
}
void installTray(){
    tray={};tray.cbSize=sizeof(tray);tray.hWnd=window;tray.uID=1;tray.uFlags=NIF_MESSAGE|NIF_ICON|NIF_TIP;tray.uCallbackMessage=TrayMessage;
    tray.hIcon=LoadIconW(GetModuleHandleW(nullptr),MAKEINTRESOURCEW(1));wcscpy_s(tray.szTip,L"Open Download Manager — downloads continue in the tray");
    trayInstalled=Shell_NotifyIconW(NIM_ADD,&tray)!=FALSE;
}
void trayNotice(const std::wstring& title,const std::wstring& message){
    if(!trayInstalled)return;tray.uFlags=NIF_INFO;wcsncpy_s(tray.szInfoTitle,title.c_str(),_TRUNCATE);wcsncpy_s(tray.szInfo,message.c_str(),_TRUNCATE);tray.dwInfoFlags=NIIF_INFO;Shell_NotifyIconW(NIM_MODIFY,&tray);
}
LRESULT CALLBACK workflowProcedure(HWND h,UINT msg,WPARAM wp,LPARAM lp);
HWND makeDialog(const wchar_t* title,int width,int height,bool independent=false){
    static bool registered=false;if(!registered){WNDCLASSW c{};c.lpfnWndProc=workflowProcedure;c.hInstance=GetModuleHandleW(nullptr);c.lpszClassName=L"ODMWorkflow";c.hCursor=LoadCursorW(nullptr,IDC_ARROW);c.hIcon=LoadIconW(c.hInstance,MAKEINTRESOURCEW(1));c.hbrBackground=(HBRUSH)(COLOR_WINDOW+1);RegisterClassW(&c);registered=true;}
    RECT owner{};GetWindowRect(window,&owner);if(independent){POINT cursor{};GetCursorPos(&cursor);MONITORINFO monitor{sizeof(monitor)};GetMonitorInfoW(MonitorFromPoint(cursor,MONITOR_DEFAULTTONEAREST),&monitor);owner.left=monitor.rcWork.left+(monitor.rcWork.right-monitor.rcWork.left-width)/2-80;owner.top=monitor.rcWork.top+(monitor.rcWork.bottom-monitor.rcWork.top-height)/2-100;}auto result=CreateWindowExW(WS_EX_DLGMODALFRAME|(independent?WS_EX_APPWINDOW:0),L"ODMWorkflow",title,WS_OVERLAPPED|WS_CAPTION|WS_SYSMENU|WS_CLIPCHILDREN,owner.left+80,owner.top+100,width,height,independent?nullptr:window,nullptr,GetModuleHandleW(nullptr),nullptr);modernWindow(result);return result;
}
void showProgress(const std::shared_ptr<Job>& j){
    for(auto& entry:progressWindows)if(entry.second==j){ShowWindow(entry.first,SW_SHOW);return;}
    auto h=makeDialog(L"Download progress",650,275,true);progressWindows[h]=j;
    std::wstring path;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;}
    dialogControl(h,L"STATIC",fs::path(path).filename().c_str(),501,SS_ENDELLIPSIS,22,20,590,26);
    SendMessageW(GetDlgItem(h,501),WM_SETFONT,(WPARAM)mediumFont,TRUE);
    dialogControl(h,L"STATIC",L"Starting download…",502,SS_ENDELLIPSIS,22,58,590,38);
    dialogControl(h,L"STATIC",L"Waiting for the server",503,SS_ENDELLIPSIS,22,148,590,26);
    dialogControl(h,L"BUTTON",L"Pause",504,WS_TABSTOP,212,190,120,32);
    dialogControl(h,L"BUTTON",L"Cancel download",505,WS_TABSTOP,342,190,145,32);
    dialogControl(h,L"BUTTON",L"Hide",506,WS_TABSTOP,497,190,115,32);
    ShowWindow(h,SW_SHOW);SetForegroundWindow(h);
}
struct ProgressEstimate {ULONGLONG tick=0;uint64_t bytes=0;double rate=0,observed=0;bool assembly=false;};
std::map<HWND,ProgressEstimate> progressEstimates;
std::wstring remainingTime(uint64_t seconds){
    if(seconds<60)return std::to_wstring(std::max<uint64_t>(1,seconds))+L" sec";
    auto minutes=(seconds+59)/60;if(minutes<60)return std::to_wstring(minutes)+L" min";
    auto hours=minutes/60;minutes%=60;if(hours<24)return std::to_wstring(hours)+L" hr"+(minutes?L" "+std::to_wstring(minutes)+L" min":L"");
    return std::to_wstring(hours/24)+L" days "+std::to_wstring(hours%24)+L" hr";
}
void updateProgressWindows(){
    for(auto it=progressWindows.begin();it!=progressWindows.end();){auto h=it->first;auto j=it->second;++it;
        if(j->status==Complete||closing){DestroyWindow(h);continue;}
        std::wstring path,error;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;error=j->error;}
        auto set=[&](int id,const std::wstring& value){auto c=GetDlgItem(h,id);if(text(c)!=value)SetWindowTextW(c,value.c_str());};
        set(501,fs::path(path).filename().wstring());set(502,error.empty()?names[j->status]:error);
        bool assembly=j->assembling.load();auto total=j->total.load(),done=assembly?j->assembled.load():j->done.load(),speed=j->speed.load();
        auto& estimate=progressEstimates[h];auto now=GetTickCount64();
        if(!estimate.tick||estimate.assembly!=assembly||done<estimate.bytes||j->status!=Downloading){estimate={now,done,0,0,assembly};}
        else if(now-estimate.tick>=500){double elapsed=(now-estimate.tick)/1000.0;double sample=(done-estimate.bytes)/elapsed;
            double weight=elapsed/(4.0+elapsed);estimate.rate=estimate.observed?estimate.rate+(sample-estimate.rate)*weight:sample;estimate.observed+=elapsed;estimate.tick=now;estimate.bytes=done;}
        std::wstring value=size(done)+L" / "+(total?size(total):L"unknown size")+L"    ·    "+size(assembly?(uint64_t)estimate.rate:speed)+L"/s";
        if(j->status==Downloading&&total&&done<total){if(estimate.observed>=2&&estimate.rate>=1)value+=L"    \u00b7    About "+remainingTime((uint64_t)std::ceil((total-done)/estimate.rate))+L" remaining";else value+=L"    \u00b7    Estimating\u2026";}
        set(503,value);set(504,j->status==Paused||j->status==Error?L"Resume":L"Pause");EnableWindow(GetDlgItem(h,504),j->status!=Canceled);EnableWindow(GetDlgItem(h,505),j->status!=Canceled);
        RECT r{22,108,612,138};InvalidateRect(h,&r,FALSE);
    }
}
void showAddDownload(){if(addDialog){SetForegroundWindow(addDialog);return;}addDialog=makeDialog(L"New download",700,285,false);
 dialogControl(addDialog,L"STATIC",L"Link",610,0,24,18,590,22);dialogControl(addDialog,L"EDIT",text(urlBox).c_str(),600,ES_AUTOHSCROLL|WS_TABSTOP,24,44,566,34);SendMessageW(GetDlgItem(addDialog,600),EM_SETCUEBANNER,FALSE,(LPARAM)L"Paste a download link");dialogControl(addDialog,L"BUTTON",L"Paste",602,WS_TABSTOP,600,44,60,34);
 dialogControl(addDialog,L"STATIC",L"Save to",611,0,24,96,590,22);dialogControl(addDialog,L"EDIT",text(folderBox).c_str(),601,ES_AUTOHSCROLL|WS_TABSTOP,24,122,566,34);dialogControl(addDialog,L"BUTTON",L"Browse",603,WS_TABSTOP,600,122,60,34);
 dialogControl(addDialog,L"BUTTON",L"Cancel",IDCANCEL,WS_TABSTOP,420,186,100,36);dialogControl(addDialog,L"BUTTON",L"Download",604,BS_DEFPUSHBUTTON|WS_TABSTOP,530,186,130,36);ShowWindow(addDialog,SW_SHOW);SetForegroundWindow(addDialog);SetFocus(GetDlgItem(addDialog,600));}
void showNextDownload(){
    if(downloadDialog||downloadChoices.empty()||closing)return;
    downloadDialog=makeDialog(L"New download — Save and Start",690,270,true);
    auto& choice=downloadChoices.front();auto name=needsVideoExtractor(choice.url)?L"Video.mp4":choice.filename.empty()?defaultName(choice.url):defaultName(L"https://local/"+choice.filename);
    dialogControl(downloadDialog,L"STATIC",L"Download link",200,0,20,18,640,22);
    dialogControl(downloadDialog,L"EDIT",choice.url.c_str(),204,ES_READONLY|ES_AUTOHSCROLL,20,44,640,36);
    dialogControl(downloadDialog,L"STATIC",L"Save file as",205,0,20,85,640,22);
    auto destination=fs::path(text(folderBox))/name;
    dialogControl(downloadDialog,L"EDIT",destination.c_str(),201,ES_AUTOHSCROLL|WS_TABSTOP,20,111,525,36);
    dialogControl(downloadDialog,L"BUTTON",L"Browse…",202,WS_TABSTOP,555,111,105,36);
    dialogControl(downloadDialog,L"STATIC",needsVideoExtractor(choice.url)?L"The video title determines the final filename; this selects its folder.":L"Choose a location, then start. Existing files are preserved.",206,0,20,151,640,24);
    if(!choice.verified)SetWindowTextW(GetDlgItem(downloadDialog,206),L"Checking website access… You can choose where to save now.");
    dialogControl(downloadDialog,L"BUTTON",L"Start download",203,BS_DEFPUSHBUTTON|WS_TABSTOP,390,185,150,32);
    dialogControl(downloadDialog,L"BUTTON",L"Cancel",IDCANCEL,WS_TABSTOP,550,185,110,32);
    ShowWindow(downloadDialog,SW_SHOW);SetForegroundWindow(downloadDialog);SetFocus(GetDlgItem(downloadDialog,203));
}
void requestDownload(std::wstring url,DWORD id=0,std::wstring filename=L"",bool verified=true,std::wstring userAgent=L"",std::wstring referer=L""){
    if(closing||downloadChoices.size()>=100)throw std::runtime_error("Too many pending download prompts. Finish or cancel a prompt first.");
    auto a=url.find_first_not_of(L" \r\n\t"),b=url.find_last_not_of(L" \r\n\t");if(a!=std::wstring::npos)url=url.substr(a,b-a+1);
    if(url.size()>16384||url.find_first_of(L"\r\n")!=std::wstring::npos||url.find(L'\0')!=std::wstring::npos||(url.rfind(L"https://",0)!=0&&url.rfind(L"http://",0)!=0&&url.rfind(L"ftp://",0)!=0&&url.rfind(L"ftps://",0)!=0))throw std::runtime_error("Use an HTTP, HTTPS or FTP download link");
    if(id)ShowWindow(window,SW_HIDE);downloadChoices.push_back({std::move(url),id,std::move(filename),std::move(userAgent),std::move(referer),verified,false});if(id)handoffDecisions[id]=0;showNextDownload();
}
void finishDownloadChoice(int decision){
    auto choice=downloadChoices.front();if(choice.id)handoffDecisions[choice.id]=decision;downloadChoices.pop_front();auto h=downloadDialog;downloadDialog=nullptr;DestroyWindow(h);showNextDownload();
}
void finishHandoffCheck(DWORD id,bool success){
    for(auto it=downloadChoices.begin();it!=downloadChoices.end();++it)if(it->id==id){
        if(!success){if(it==downloadChoices.begin())finishDownloadChoice(3);else{downloadChoices.erase(it);handoffDecisions[id]=3;}return;}
        it->verified=true;
        if(it==downloadChoices.begin()&&downloadDialog){SetWindowTextW(GetDlgItem(downloadDialog,206),L"Choose a location, then start. Existing files are preserved.");EnableWindow(GetDlgItem(downloadDialog,203),TRUE);SetWindowTextW(GetDlgItem(downloadDialog,203),L"Start download");if(it->startRequested)PostMessageW(downloadDialog,WM_COMMAND,203,0);}
        return;
    }
}
void showConnectionSettings(){
 if(networkDialog){SetForegroundWindow(networkDialog);return;}networkDialog=makeDialog(L"Connection settings",700,570);auto value=networkSnapshot();
 const wchar_t* labels[]={L"Proxy address",L"Proxy username",L"Proxy password",L"Server origin",L"Server username",L"Server password"};std::wstring values[]={value.proxy,value.proxyUser,value.proxyPassword,value.serverOrigin,value.serverUser,value.serverPassword};
 for(int i=0;i<6;++i){dialogControl(networkDialog,L"STATIC",labels[i],350+i,0,24,30+i*58,146,24);dialogControl(networkDialog,L"EDIT",values[i].c_str(),330+i,ES_AUTOHSCROLL|WS_TABSTOP|((i==2||i==5)?ES_PASSWORD:0),180,24+i*58,470,36);}
 dialogControl(networkDialog,L"STATIC",L"Proxy: http://host:port or socks5h://host:port (blank = system default).\nServer origin: https://example.com or ftp://host:port, without a trailing slash.\nBasic, Digest and NTLM are negotiated automatically.\nSaved credentials are encrypted for your Windows account.",356,0,24,382,630,90);
 dialogControl(networkDialog,L"BUTTON",L"Save",338,BS_DEFPUSHBUTTON|WS_TABSTOP,420,486,110,34);dialogControl(networkDialog,L"BUTTON",L"Cancel",IDCANCEL,WS_TABSTOP,540,486,110,34);ShowWindow(networkDialog,SW_SHOW);
}
void showRenewLink(const std::shared_ptr<Job>& j){
 if(j->active||j->status==Complete)throw std::runtime_error("Pause the download before replacing its link");if(renewDialog){SetForegroundWindow(renewDialog);return;}renewingJob=j;renewDialog=makeDialog(L"Renew download link",700,240);std::wstring url;{std::lock_guard<std::mutex> lock(j->mutex);url=j->url;}
 dialogControl(renewDialog,L"STATIC",L"Paste a fresh link for the same file",360,0,24,20,620,24);dialogControl(renewDialog,L"EDIT",url.c_str(),361,ES_AUTOHSCROLL|WS_TABSTOP,24,52,630,36);dialogControl(renewDialog,L"STATIC",L"Partial data is reused only when the server validates it.",362,0,24,104,630,26);dialogControl(renewDialog,L"BUTTON",L"Renew and resume",363,BS_DEFPUSHBUTTON|WS_TABSTOP,370,150,170,34);dialogControl(renewDialog,L"BUTTON",L"Cancel",IDCANCEL,WS_TABSTOP,550,150,105,34);ShowWindow(renewDialog,SW_SHOW);
}
void showSettings(){
    if(settingsDialog){ShowWindow(settingsDialog,SW_RESTORE);SetForegroundWindow(settingsDialog);return;}
    settingsDialog=makeDialog(L"Settings",510,310);
    const wchar_t* labels[]={L"Simultaneous downloads (1–8)",L"Connections per file (1–16)",L"Per-download limit, KiB/s (0 = unlimited)"};int values[]={maxActive,connectionsPerFile.load(),limitKiB.load()};
    for(int i=0;i<3;++i){dialogControl(settingsDialog,L"STATIC",labels[i],320+i,0,20,25+i*49,340,24);dialogControl(settingsDialog,L"EDIT",std::to_wstring(values[i]).c_str(),310+i,ES_NUMBER|ES_AUTOHSCROLL|WS_TABSTOP,370,22+i*49,100,36);}
    dialogControl(settingsDialog,L"STATIC",L"Connection changes apply as active segments finish.\nSpeed limits apply while downloads are running.",323,0,20,178,450,42);
    dialogControl(settingsDialog,L"BUTTON",L"Connections...",315,WS_TABSTOP,20,231,175,32);
    dialogControl(settingsDialog,L"BUTTON",L"Save settings",314,BS_DEFPUSHBUTTON|WS_TABSTOP,220,231,135,32);dialogControl(settingsDialog,L"BUTTON",L"Cancel",IDCANCEL,WS_TABSTOP,365,231,105,32);
    ShowWindow(settingsDialog,SW_SHOW);SetForegroundWindow(settingsDialog);
}
void showNextCompletion(){
    if(completionDialog||completionChoices.empty()||closing)return;
    auto j=completionChoices.front();std::wstring path;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;}
    completionDialog=makeDialog(L"Download complete",640,205,true);
    dialogControl(completionDialog,L"STATIC",L"Your download is ready",400,0,20,20,590,26);
    dialogControl(completionDialog,L"EDIT",path.c_str(),404,ES_READONLY|ES_AUTOHSCROLL,20,58,590,36);
    dialogControl(completionDialog,L"BUTTON",L"Open file",401,WS_TABSTOP,210,115,120,32);dialogControl(completionDialog,L"BUTTON",L"Open folder",402,WS_TABSTOP,340,115,130,32);dialogControl(completionDialog,L"BUTTON",L"Close",IDCANCEL,BS_DEFPUSHBUTTON|WS_TABSTOP,480,115,130,32);
    ShowWindow(completionDialog,SW_SHOWNOACTIVATE);trayNotice(L"Download complete",fs::path(path).filename().wstring());
}
bool workflowDialogMessage(MSG& msg){for(auto& entry:progressWindows)if(IsDialogMessageW(entry.first,&msg))return true;for(auto h:{addDialog,downloadDialog,settingsDialog,completionDialog,networkDialog,renewDialog})if(h&&IsDialogMessageW(h,&msg))return true;return false;}
LRESULT CALLBACK workflowProcedure(HWND h,UINT msg,WPARAM wp,LPARAM lp){
    try{
        if(msg==WM_DRAWITEM)return flatButton((DRAWITEMSTRUCT*)lp);
        if(msg==WM_CTLCOLORSTATIC||msg==WM_CTLCOLOREDIT){SetTextColor((HDC)wp,ink);SetBkColor((HDC)wp,RGB(255,255,255));return (LRESULT)whiteBrush;}
        if(msg==WM_ERASEBKGND)return 1;
        if(msg==WM_PAINT){PAINTSTRUCT ps;auto screen=BeginPaint(h,&ps);RECT r;GetClientRect(h,&r);auto dc=CreateCompatibleDC(screen);auto bitmap=CreateCompatibleBitmap(screen,r.right,r.bottom);auto old=SelectObject(dc,bitmap);FillRect(dc,&r,whiteBrush);auto it=progressWindows.find(h);if(it!=progressWindows.end()){
            auto total=it->second->total.load(),done=it->second->assembling.load()?it->second->assembled.load():it->second->done.load();RECT track{22,110,612,136};rounded(dc,track,RGB(233,239,249),RGB(233,239,249),16);
            RECT fill=track;if(total)fill.right=fill.left+(int)(590*std::min(1.0,double(done)/total));else{int offset=(GetTickCount64()/20)%450;fill.left+=offset;fill.right=fill.left+140;}
            if(fill.right>fill.left)rounded(dc,fill,blue,blue,16);if(total)label(dc,std::to_wstring((int)std::min(100.0,double(done)*100/total))+L"%",track,smallFont,ink,DT_CENTER|DT_SINGLELINE|DT_VCENTER);
        }BitBlt(screen,ps.rcPaint.left,ps.rcPaint.top,ps.rcPaint.right-ps.rcPaint.left,ps.rcPaint.bottom-ps.rcPaint.top,dc,ps.rcPaint.left,ps.rcPaint.top,SRCCOPY);SelectObject(dc,old);DeleteObject(bitmap);DeleteDC(dc);EndPaint(h,&ps);return 0;}
        if(msg==WM_DESTROY){progressWindows.erase(h);progressEstimates.erase(h);return 0;}
        if(msg==WM_COMMAND){int id=LOWORD(wp);
            auto progress=progressWindows.find(h);if(progress!=progressWindows.end()){auto j=progress->second;
                if(id==504){if(j->status==Paused||j->status==Error){if(j->active)j->resumeRequested=true;else{j->stop=false;j->status=Queued;}}else if(j->status!=Complete&&j->status!=Canceled){j->resumeRequested=false;j->status=Paused;j->stop=true;}save();update();return 0;}
                if(id==505){j->resumeRequested=false;j->status=Canceled;j->stop=true;save();update();return 0;}
                if(id==506||id==IDCANCEL){DestroyWindow(h);return 0;}
            }
            if(h==addDialog){
                if(id==602){SendMessageW(window,WM_COMMAND,105,0);SetWindowTextW(GetDlgItem(h,600),text(urlBox).c_str());SetFocus(GetDlgItem(h,600));return 0;}
                if(id==603){SetWindowTextW(folderBox,text(GetDlgItem(h,601)).c_str());SendMessageW(window,WM_COMMAND,102,0);SetWindowTextW(GetDlgItem(h,601),text(folderBox).c_str());return 0;}
                if(id==604){SetWindowTextW(urlBox,text(GetDlgItem(h,600)).c_str());SetWindowTextW(folderBox,text(GetDlgItem(h,601)).c_str());requestDownload(text(urlBox));SetWindowTextW(urlBox,L"");addDialog=nullptr;DestroyWindow(h);return 0;}
                if(id==IDCANCEL){addDialog=nullptr;DestroyWindow(h);return 0;}
            }
            if(h==downloadDialog){
                if(id==202){wchar_t path[32768]{};auto existing=text(GetDlgItem(h,201));wcsncpy_s(path,existing.c_str(),_TRUNCATE);OPENFILENAMEW ofn{};ofn.lStructSize=sizeof(ofn);ofn.hwndOwner=h;ofn.lpstrFilter=L"All files\0*.*\0";ofn.lpstrFile=path;ofn.nMaxFile=32768;ofn.Flags=OFN_EXPLORER|OFN_NOCHANGEDIR|OFN_PATHMUSTEXIST|OFN_OVERWRITEPROMPT;if(GetSaveFileNameW(&ofn))SetWindowTextW(GetDlgItem(h,201),path);return 0;}
                if(id==203){if(!downloadChoices.front().verified){downloadChoices.front().startRequested=true;EnableWindow(GetDlgItem(h,203),FALSE);SetWindowTextW(GetDlgItem(h,203),L"Checking access…");return 0;}fs::path path(text(GetDlgItem(h,201)));if(!path.is_absolute()||path.filename().empty()||path.filename()==L"."||path.filename()==L"..")throw std::runtime_error("Choose an absolute folder and a filename.");
                    auto name=path.filename().wstring();if(name!=defaultName(L"https://local/"+name))throw std::runtime_error("Choose a valid Windows filename.");
                    add(downloadChoices.front().url,path.parent_path());jobs.back()->userAgent=downloadChoices.front().userAgent;jobs.back()->referer=downloadChoices.front().referer;jobs.back()->path=path.wstring();queuePaths.insert(pathKey(path));SetWindowTextW(folderBox,path.parent_path().c_str());scrollToNewest=true;showProgress(jobs.back());save();finishDownloadChoice(1);update();return 0;}
                if(id==IDCANCEL){finishDownloadChoice(2);return 0;}
            }
            if(h==networkDialog){
                if(id==338){NetworkOptions value{text(GetDlgItem(h,330)),text(GetDlgItem(h,331)),text(GetDlgItem(h,332)),text(GetDlgItem(h,333)),text(GetDlgItem(h,334)),text(GetDlgItem(h,335))};storeNetworkOptions(value);networkDialog=nullptr;DestroyWindow(h);return 0;}
                if(id==IDCANCEL){networkDialog=nullptr;DestroyWindow(h);return 0;}
            }
            if(h==renewDialog){
                if(id==363){auto url=text(GetDlgItem(h,361));if(url.size()>16384||url.find_first_of(L"\r\n")!=std::wstring::npos||(url.rfind(L"http://",0)!=0&&url.rfind(L"https://",0)!=0&&url.rfind(L"ftp://",0)!=0&&url.rfind(L"ftps://",0)!=0))throw std::runtime_error("Enter an HTTP, HTTPS or FTP URL");auto j=renewingJob;if(j->active)throw std::runtime_error("Wait for the download to finish pausing");{std::lock_guard<std::mutex> lock(j->mutex);j->url=url;j->error.clear();j->userAgent.clear();j->referer.clear();}j->stop=false;j->completionNotified=false;j->status=Queued;renewDialog=nullptr;renewingJob.reset();DestroyWindow(h);showProgress(j);save();update();return 0;}
                if(id==IDCANCEL){renewDialog=nullptr;renewingJob.reset();DestroyWindow(h);return 0;}
            }
            if(h==settingsDialog){
                if(id==315){showConnectionSettings();return 0;}
                if(id==314){int values[3];for(int i=0;i<3;++i){auto v=text(GetDlgItem(h,310+i));size_t used=0;values[i]=std::stoi(v,&used);if(used!=v.size())throw std::runtime_error("Enter whole numbers in Settings.");}
                    if(values[0]<1||values[0]>8||values[1]<1||values[1]>16||values[2]<0||values[2]>100000000)throw std::runtime_error("Use 1–8 downloads, 1–16 connections, and a nonnegative speed limit.");maxActive=values[0];connectionsPerFile=values[1];limitKiB=values[2];save();settingsDialog=nullptr;DestroyWindow(h);return 0;}
                if(id==IDCANCEL){settingsDialog=nullptr;DestroyWindow(h);return 0;}
            }
            if(h==completionDialog&&(id==401||id==402||id==IDCANCEL)){if(id!=IDCANCEL){auto j=completionChoices.front();std::wstring path;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;}openWithShell(id==401?path:fs::path(path).parent_path().wstring());}completionChoices.pop_front();completionDialog=nullptr;DestroyWindow(h);showNextCompletion();return 0;}
        }
        if(msg==WM_CLOSE){SendMessageW(h,WM_COMMAND,IDCANCEL,0);return 0;}
    }catch(const std::exception& e){MessageBoxW(h,utf16(e.what()).c_str(),L"Open Download Manager",MB_OK|MB_ICONERROR);return 0;}
    return DefWindowProcW(h,msg,wp,lp);
}

