std::future<std::vector<std::wstring>> importTask;
std::future<void> exportTask;
std::deque<std::wstring> pendingImport;
fs::path importFolder;int importFailed=0;
bool fileTasksDone(){return !importTask.valid()&&!exportTask.valid()&&pendingImport.empty();}
void processFileTasks(){
    if(exportTask.valid()&&exportTask.wait_for(std::chrono::seconds(0))==std::future_status::ready)exportTask.get();
    if(importTask.valid()&&importTask.wait_for(std::chrono::seconds(0))==std::future_status::ready){auto lines=importTask.get();for(auto& line:lines)pendingImport.push_back(std::move(line));}
    bool changed=false;for(int n=0;n<256&&!pendingImport.empty();++n){auto line=std::move(pendingImport.front());pendingImport.pop_front();try{add(line,importFolder);changed=true;}catch(...){++importFailed;}}
    if(changed&&pendingImport.empty())save();
}
// Window handlers never wait for network/disk workers or hold download locks across Windows calls.
std::vector<std::array<std::wstring,8>> renderedRows;
bool shutdownSaved=false;
void openWithShell(std::wstring path){std::thread([path=std::move(path)]{CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);ShellExecuteW(nullptr,L"open",path.c_str(),nullptr,nullptr,SW_SHOWNORMAL);CoUninitialize();}).detach();}
// Native Win32 interface: calm light surfaces, custom buttons, status pills and progress bars.
constexpr COLORREF background=RGB(242,245,250),navy=RGB(20,31,52),blue=RGB(0,122,255),ink=RGB(30,41,59),muted=RGB(100,116,139),border=RGB(219,226,237);
HFONT titleFont,smallFont,mediumFont;HBRUSH backgroundBrush,whiteBrush;HIMAGELIST rowImages;
HWND connectionsBox,videoHint,emptyHint;std::wstring queueSummary;
std::wstring size(uint64_t value) {double v=(double)value;const wchar_t* units[]={L"B",L"KiB",L"MiB",L"GiB",L"TiB"};int i=0;while(v>=1024&&i<4){v/=1024;++i;}wchar_t b[80];swprintf(b,80,L"%.1f %ls",v,units[i]);return b;}
std::vector<int> selected() {std::vector<int> result;for(int i=-1;(i=ListView_GetNextItem(list,i,LVNI_SELECTED))!=-1;)if(i<(int)jobs.size())result.push_back(i);return result;}
void ensureSmoothDrawing(){static struct Runtime{ULONG_PTR token=0;Runtime(){Gdiplus::GdiplusStartupInput input;Gdiplus::GdiplusStartup(&token,&input,nullptr);}~Runtime(){if(token)Gdiplus::GdiplusShutdown(token);}} runtime;}
Gdiplus::Color smoothColor(COLORREF c){return Gdiplus::Color(255,GetRValue(c),GetGValue(c),GetBValue(c));}
void roundPath(Gdiplus::GraphicsPath& path,float x,float y,float w,float h,float radius){float d=radius*2;path.AddArc(x,y,d,d,180,90);path.AddArc(x+w-d,y,d,d,270,90);path.AddArc(x+w-d,y+h-d,d,d,0,90);path.AddArc(x,y+h-d,d,d,90,90);path.CloseFigure();}
void rounded(HDC dc,RECT rect,COLORREF fill,COLORREF stroke,int radius=24){ensureSmoothDrawing();Gdiplus::Graphics g(dc);g.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);Gdiplus::GraphicsPath path;roundPath(path,(float)rect.left+.5f,(float)rect.top+.5f,(float)(rect.right-rect.left-1),(float)(rect.bottom-rect.top-1),radius/2.f);Gdiplus::SolidBrush brush(smoothColor(fill));Gdiplus::Pen pen(smoothColor(stroke),1.f);g.FillPath(&brush,&path);g.DrawPath(&pen,&path);}
void label(HDC dc,const std::wstring& value,RECT rect,HFONT face,COLORREF color,UINT flags=DT_LEFT|DT_SINGLELINE|DT_VCENTER|DT_END_ELLIPSIS) {auto old=SelectObject(dc,face);SetTextColor(dc,color);SetBkMode(dc,TRANSPARENT);DrawTextW(dc,value.c_str(),(int)value.size(),&rect,flags);SelectObject(dc,old);}
void paintRoundedEdit(HWND h){RECT r;GetWindowRect(h,&r);int w=r.right-r.left,height=r.bottom-r.top;HDC dc=GetWindowDC(h);RECT client;GetClientRect(h,&client);POINT origin{0,0};ClientToScreen(h,&origin);ExcludeClipRect(dc,origin.x-r.left,origin.y-r.top,origin.x-r.left+client.right,origin.y-r.top+client.bottom);RECT outer{0,0,w,height};auto white=CreateSolidBrush(RGB(255,255,255));FillRect(dc,&outer,white);DeleteObject(white);rounded(dc,outer,RGB(255,255,255),GetFocus()==h?blue:border,16);ReleaseDC(h,dc);}
LRESULT CALLBACK roundedEditProc(HWND h,UINT msg,WPARAM wp,LPARAM lp,UINT_PTR,DWORD_PTR){if(msg==WM_NCCALCSIZE){auto result=DefSubclassProc(h,msg,wp,lp);RECT* r=wp?&((NCCALCSIZE_PARAMS*)lp)->rgrc[0]:(RECT*)lp;r->top+=6;r->bottom-=6;r->left+=1;r->right-=1;return result;}if(msg==WM_NCPAINT){paintRoundedEdit(h);return 0;}auto result=DefSubclassProc(h,msg,wp,lp);if(msg==WM_SETFOCUS||msg==WM_KILLFOCUS)RedrawWindow(h,nullptr,nullptr,RDW_INVALIDATE|RDW_FRAME);if(msg==WM_NCDESTROY)RemoveWindowSubclass(h,roundedEditProc,1);return result;}
void styleEdit(HWND h){SetWindowSubclass(h,roundedEditProc,1,0);SendMessageW(h,EM_SETMARGINS,EC_LEFTMARGIN|EC_RIGHTMARGIN,MAKELPARAM(10,10));SetWindowPos(h,nullptr,0,0,0,0,SWP_NOMOVE|SWP_NOSIZE|SWP_NOZORDER|SWP_FRAMECHANGED);}
HWND control(const wchar_t* cls,const wchar_t* caption,int id,DWORD style,int x,int y,int w,int h) {bool edit=std::wstring(cls)==L"EDIT",button=std::wstring(cls)==L"BUTTON";auto c=CreateWindowExW(0,cls,caption,WS_CHILD|WS_VISIBLE|style|(button?BS_OWNERDRAW:0),x,y,w,h,window,(HMENU)(INT_PTR)id,GetModuleHandleW(nullptr),nullptr);SendMessageW(c,WM_SETFONT,(WPARAM)font,TRUE);if(edit)styleEdit(c);return c;}
void modernWindow(HWND h){
    static auto library=LoadLibraryW(L"dwmapi.dll");using SetAttribute=HRESULT(WINAPI*)(HWND,DWORD,LPCVOID,DWORD);static auto set=library?(SetAttribute)GetProcAddress(library,"DwmSetWindowAttribute"):nullptr;
    DWORD corner=2;COLORREF caption=RGB(255,255,255),captionText=ink;
    if(set){set(h,33,&corner,sizeof(corner));set(h,35,&caption,sizeof(caption));set(h,36,&captionText,sizeof(captionText));}
}
HWND buttonTips=nullptr;
bool iconButton(int id){return id==122||id==105||id==102||id==120||(id>=112&&id<=116);}
void tip(HWND button,const wchar_t* value,bool replace=false){if(!buttonTips)return;TOOLINFOW info{};info.cbSize=sizeof(info);info.uFlags=TTF_IDISHWND|TTF_SUBCLASS;info.hwnd=window;info.uId=(UINT_PTR)button;info.lpszText=(LPWSTR)value;SendMessageW(buttonTips,replace?TTM_UPDATETIPTEXTW:TTM_ADDTOOLW,0,(LPARAM)&info);}
void updateActionButton(){auto button=GetDlgItem(window,120);if(!button)return;bool pause=false,can=false;for(int i:selected()){int state=jobs[i]->status;if(state==Downloading||state==Queued){pause=true;can=true;}if(state==Paused||state==Error||state==Scheduled)can=true;}const wchar_t* caption=pause?L"Pause":L"Resume";if(text(button)!=caption){SetWindowTextW(button,caption);tip(button,caption,true);InvalidateRect(button,nullptr,FALSE);}EnableWindow(button,can&&!closing);auto ids=selected();EnableWindow(GetDlgItem(window,116),!closing&&!ids.empty()&&!jobs[ids.front()]->active);}
void drawButtonIcon(HDC dc,RECT box,int id,COLORREF color,bool pause){
 ensureSmoothDrawing();if(id==602)id=105;if(id==603)id=102;Gdiplus::Graphics g(dc);g.SetSmoothingMode(Gdiplus::SmoothingModeAntiAlias);g.SetPixelOffsetMode(Gdiplus::PixelOffsetModeHighQuality);g.TranslateTransform((box.left+box.right)/2.f-12.f,(box.top+box.bottom)/2.f-12.f);
 Gdiplus::Pen pen(smoothColor(color),1.65f);pen.SetStartCap(Gdiplus::LineCapRound);pen.SetEndCap(Gdiplus::LineCapRound);pen.SetLineJoin(Gdiplus::LineJoinRound);Gdiplus::SolidBrush brush(smoothColor(color));
 auto line=[&](float x,float y,float a,float b){g.DrawLine(&pen,x,y,a,b);};
 auto rect=[&](float x,float y,float w,float h,float r,bool fill=false){Gdiplus::GraphicsPath p;roundPath(p,x,y,w,h,r);if(fill)g.FillPath(&brush,&p);else g.DrawPath(&pen,&p);};
 if(id==122){line(12,5,12,19);line(5,12,19,12);}
 else if(id==120){if(pause){rect(6,5,4,14,1,true);rect(14,5,4,14,1,true);}else{Gdiplus::GraphicsPath p;p.AddLine(7.5f,5.f,18.f,11.f);p.AddBezier(18.f,11.f,19.f,11.5f,19.f,12.5f,18.f,13.f);p.AddLine(18.f,13.f,7.5f,19.f);p.AddBezier(7.5f,19.f,6.7f,19.5f,6.f,19.f,6.f,18.f);p.AddLine(6.f,18.f,6.f,6.f);p.AddBezier(6.f,6.f,6.f,5.f,6.7f,4.5f,7.5f,5.f);p.CloseFigure();g.FillPath(&brush,&p);}}
 else if(id==112)rect(6,6,12,12,2.3f,true);
 else if(id==102||id==113){Gdiplus::GraphicsPath p;p.AddLine(4.f,6.f,9.f,6.f);p.AddLine(9.f,6.f,11.f,8.f);p.AddLine(11.f,8.f,20.f,8.f);p.AddBezier(20.f,8.f,21.f,8.f,21.f,9.f,21.f,10.f);p.AddLine(21.f,10.f,21.f,18.f);p.AddBezier(21.f,18.f,21.f,19.f,20.f,19.f,19.f,19.f);p.AddLine(19.f,19.f,4.f,19.f);p.AddBezier(4.f,19.f,3.f,19.f,3.f,18.f,3.f,17.f);p.AddLine(3.f,17.f,3.f,8.f);p.AddBezier(3.f,8.f,3.f,6.f,3.f,6.f,4.f,6.f);g.DrawPath(&pen,&p);}
 else if(id==105){rect(5,5,14,16,2);Gdiplus::SolidBrush back(Gdiplus::Color(255,255,255,255));g.FillRectangle(&back,8.f,3.f,8.f,5.f);rect(8,3,8,5,1.5f);line(9,12,15,12);line(9,16,14,16);}
 else if(id==116){line(4,6,20,6);line(9,3,15,3);Gdiplus::GraphicsPath p;p.AddLine(6.f,6.f,7.f,19.f);p.AddBezier(7.f,19.f,7.f,20.f,8.f,21.f,9.f,21.f);p.AddLine(9.f,21.f,15.f,21.f);p.AddBezier(15.f,21.f,16.f,21.f,17.f,20.f,17.f,19.f);p.AddLine(17.f,19.f,18.f,6.f);g.DrawPath(&pen,&p);line(10,10,10.5f,17);line(14,10,13.5f,17);}
 else if(id==114||id==115){Gdiplus::GraphicsPath page;page.AddLine(6.f,2.f,14.f,2.f);page.AddLine(14.f,2.f,20.f,8.f);page.AddLine(20.f,8.f,20.f,20.f);page.AddBezier(20.f,20.f,20.f,21.f,19.f,22.f,18.f,22.f);page.AddLine(18.f,22.f,6.f,22.f);page.AddBezier(6.f,22.f,5.f,22.f,4.f,21.f,4.f,20.f);page.AddLine(4.f,20.f,4.f,4.f);page.AddBezier(4.f,4.f,4.f,3.f,5.f,2.f,6.f,2.f);g.DrawPath(&pen,&page);line(14,2,14,8);line(14,8,20,8);
  if(id==114){Gdiplus::GraphicsPath chain;chain.AddBezier(11.f,12.f,9.f,10.f,6.f,12.f,7.f,15.f);chain.AddBezier(7.f,15.f,7.5f,17.f,9.5f,18.f,11.f,16.f);chain.StartFigure();chain.AddBezier(13.f,12.f,15.f,10.f,18.f,12.f,17.f,15.f);chain.AddBezier(17.f,15.f,16.5f,17.f,14.5f,18.f,13.f,16.f);g.DrawPath(&pen,&chain);line(10,14,14,14);}
  else{line(12,18,12,11);line(9,14,12,11);line(12,11,15,14);}
 }
}
LRESULT flatButton(DRAWITEMSTRUCT* d){
    if(d->CtlType!=ODT_BUTTON)return FALSE;bool primary=d->CtlID==604||d->CtlID==104||d->CtlID==203||d->CtlID==314||d->CtlID==401||d->CtlID==504,pressed=(d->itemState&ODS_SELECTED)!=0,disabled=(d->itemState&ODS_DISABLED)!=0;
    bool icon=(GetParent(d->hwndItem)==window&&iconButton(d->CtlID))||d->CtlID==602||d->CtlID==603;auto surface=CreateSolidBrush(GetParent(d->hwndItem)==window&&(d->CtlID==122||d->CtlID==120||d->CtlID==121||(d->CtlID>=112&&d->CtlID<=116))?background:RGB(255,255,255));FillRect(d->hDC,&d->rcItem,surface);DeleteObject(surface);
    COLORREF fill=disabled?RGB(239,242,246):primary?(pressed?RGB(0,103,220):blue):(pressed?RGB(229,237,247):RGB(255,255,255));RECT box=d->rcItem;InflateRect(&box,-1,-1);rounded(d->hDC,box,fill,primary?fill:RGB(229,233,240),20);
    if(icon)drawButtonIcon(d->hDC,box,d->CtlID,disabled?RGB(164,173,187):ink,text(d->hwndItem)==L"Pause");else label(d->hDC,text(d->hwndItem),box,font,disabled?muted:primary?RGB(255,255,255):ink,DT_CENTER|DT_VCENTER|DT_SINGLELINE);
    if(d->itemState&ODS_FOCUS){InflateRect(&box,-4,-4);DrawFocusRect(d->hDC,&box);}return TRUE;
}
std::wstring fileDialog(bool saveFile) {wchar_t path[32768]{};OPENFILENAMEW ofn{};ofn.lStructSize=sizeof(ofn);ofn.hwndOwner=window;ofn.lpstrFilter=L"Text files\0*.txt\0All files\0*.*\0";ofn.lpstrFile=path;ofn.nMaxFile=32768;ofn.lpstrDefExt=L"txt";ofn.Flags=OFN_EXPLORER|OFN_NOCHANGEDIR|(saveFile?OFN_OVERWRITEPROMPT:OFN_FILEMUSTEXIST);return (saveFile?GetSaveFileNameW(&ofn):GetOpenFileNameW(&ofn))?path:L"";}
#include "workflow.h"
std::wstring downloadType(const std::wstring& path){
 auto ext=fs::path(path).extension().wstring();std::transform(ext.begin(),ext.end(),ext.begin(),::towlower);ext=L"|"+ext+L"|";
 for(auto group:{std::pair<const wchar_t*,const wchar_t*>{L"Video",L"|.mp4|.mkv|.webm|.avi|.mov|.ts|.3gp|.flv|.m4v|.mpg|.mpeg|.wmv|"},{L"Audio",L"|.mp3|.m4a|.aac|.flac|.wav|.ogg|.opus|.wma|.aiff|"},{L"Document",L"|.pdf|.doc|.docx|.xls|.xlsx|.ppt|.pptx|.txt|.csv|.rtf|.epub|"},{L"Archive",L"|.zip|.rar|.7z|.gz|.tar|.bz2|.xz|.iso|"},{L"Image",L"|.png|.jpg|.jpeg|.gif|.svg|.webp|.bmp|.heic|.tiff|"},{L"Application",L"|.exe|.msi|.msix|.apk|.appx|"}})if(std::wstring(group.second).find(ext)!=std::wstring::npos)return group.first;
 return L"Other";
}
std::wstring downloadDate(uint64_t addedAt){
 if(!addedAt)return L"—";
 FILETIME utc{DWORD(addedAt),DWORD(addedAt>>32)},local{};SYSTEMTIME time{};
 if(!FileTimeToLocalFileTime(&utc,&local)||!FileTimeToSystemTime(&local,&time))return L"—";
 wchar_t date[128]{},clock[128]{};
 if(!GetDateFormatEx(LOCALE_NAME_USER_DEFAULT,DATE_SHORTDATE,&time,nullptr,date,128,nullptr)||!GetTimeFormatEx(LOCALE_NAME_USER_DEFAULT,0,&time,nullptr,clock,128))return L"—";
 return std::wstring(date)+L"  "+clock;
}
int sortColumn=-1;bool sortAscending=true;
void sortDownloads(int column,bool ascending){
 if(column<0||column>=8)return;
 auto ids=selected();std::vector<std::shared_ptr<Job>> selection;for(int id:ids)selection.push_back(jobs[id]);
 int focus=ListView_GetNextItem(list,-1,LVNI_FOCUSED);auto focused=focus>=0&&focus<(int)jobs.size()?jobs[focus]:nullptr;
 // Freeze changing worker values before comparing, so the ordering stays consistent.
 struct Row{std::shared_ptr<Job> job;std::wstring file,type;int status,lanes;uint64_t done,total,speed,addedAt;};
 std::vector<Row> rows;rows.reserve(jobs.size());
 for(auto& j:jobs){std::wstring path;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;}auto file=fs::path(path).filename().wstring();std::transform(file.begin(),file.end(),file.begin(),::towlower);rows.push_back({j,file,downloadType(path),j->status.load(),j->lanes.load(),j->done.load(),j->total.load(),j->speed.load(),j->addedAt});}
 std::stable_sort(rows.begin(),rows.end(),[column,ascending](const Row& a,const Row& b){
  auto compare=[](const auto& x,const auto& y){return x<y?-1:x>y?1:0;};int order=0;
  switch(column){
   case 0:order=compare(a.file,b.file);break;
   case 1:order=compare(a.status==Complete,b.status==Complete);if(!order)order=compare(a.status,b.status);break;
   case 2:{long double x=a.total?std::min(1.L,(long double)a.done/a.total):0.L,y=b.total?std::min(1.L,(long double)b.done/b.total):0.L;order=compare(x,y);break;}
   case 3:order=compare(a.done,b.done);if(!order)order=compare(a.total,b.total);break;
   case 4:order=compare(a.speed,b.speed);break;
   case 5:order=compare(a.lanes,b.lanes);break;
   case 6:order=compare(a.type,b.type);break;case 7:order=compare(a.addedAt,b.addedAt);break;
  }return ascending?order<0:order>0;
 });
 ListView_SetItemState(list,-1,0,LVIS_SELECTED|LVIS_FOCUSED);
 for(size_t i=0;i<rows.size();++i)jobs[i]=rows[i].job;
 renderedRows.clear();int focusedIndex=-1;
 for(int i=0;i<(int)jobs.size();++i){if(std::find(selection.begin(),selection.end(),jobs[i])!=selection.end())ListView_SetItemState(list,i,LVIS_SELECTED,LVIS_SELECTED);if(jobs[i]==focused){focusedIndex=i;ListView_SetItemState(list,i,LVIS_FOCUSED,LVIS_FOCUSED);}}
 ListView_SetSelectionMark(list,focusedIndex);sortColumn=column;sortAscending=ascending;
 auto header=ListView_GetHeader(list);for(int i=0;i<8;++i){HDITEMW item{};item.mask=HDI_FORMAT;if(Header_GetItem(header,i,&item)){item.fmt&=~(HDF_SORTUP|HDF_SORTDOWN);if(i==column)item.fmt|=ascending?HDF_SORTUP:HDF_SORTDOWN;Header_SetItem(header,i,&item);}}
 scrollToNewest=false;save();update();if(focusedIndex>=0)ListView_EnsureVisible(list,focusedIndex,FALSE);
}
void organizeDownloads(bool byType){sortDownloads(byType?6:1,true);}
void layout() {
    RECT r;GetClientRect(window,&r);int w=r.right,h=r.bottom;
    for(int id:{100,101,102,104,105,140,141,143})ShowWindow(GetDlgItem(window,id),SW_HIDE);
    int toolbarIds[]={122,120,112,113,114,115,116};for(int i=0;i<7;++i)MoveWindow(GetDlgItem(window,toolbarIds[i]),24+i*56,20,44,36,TRUE);
    MoveWindow(GetDlgItem(window,123),424,20,122,36,TRUE);MoveWindow(GetDlgItem(window,121),w-134,20,110,36,TRUE);
    int listHeight=std::max(100,h-224);MoveWindow(list,24,90,w-48,listHeight,TRUE);SetWindowRgn(list,CreateRoundRectRgn(0,0,w-48,listHeight,20,20),TRUE);
    MoveWindow(emptyHint,(w-420)/2,90+listHeight/2-14,420,60,TRUE);
    MoveWindow(GetDlgItem(window,117),40,h-109,120,34,TRUE);MoveWindow(GetDlgItem(window,118),w-136,h-109,96,34,TRUE);
    MoveWindow(detail,28,h-42,w-56,24,TRUE);
    ListView_SetColumnWidth(list,0,std::max(220,w-858));InvalidateRect(window,nullptr,FALSE);
}
void update() {
    updateActionButton();processFileTasks();
    if(!trayInstalled&&!closing){static int attempts=0;if(++attempts<=12)installTray();}
    for(auto it=workers.begin();it!=workers.end();)if(!it->job->active){if(it->thread.joinable())it->thread.join();it=workers.erase(it);}else ++it;
    ShowWindow(emptyHint,jobs.empty()?SW_SHOW:SW_HIDE);
    int active=0,complete=0,queued=0;uint64_t totalSpeed=0;
    for(auto& j:jobs)if(j->active)++active;
    for(auto& j:jobs){
        if(!closing&&j->resumeRequested&&!j->active){j->resumeRequested=false;j->status=Queued;}
        if(!closing&&j->status==Queued&&!j->active&&active<maxActive){j->stop=false;j->status=Downloading;j->active=true;try{workers.reserve(workers.size()+1);workers.push_back({j,std::thread(worker,j)});++active;}catch(...){j->active=false;j->status=Error;throw;}}
        if(j->status==Complete&&!j->completionNotified){j->completionNotified=true;completionChoices.push_back(j);}
        if(j->status==Complete)++complete;if(j->status==Queued||j->status==Scheduled)++queued;totalSpeed+=j->speed;
    }
    updateProgressWindows();showNextCompletion();
    int firstChanged=(int)jobs.size(),lastChanged=-1;
    for(int i=0;i<(int)jobs.size();++i){auto& j=jobs[i];std::wstring path,schedule;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;schedule=j->schedule;}
        auto name=fs::path(path).filename().wstring();
        uint64_t total=j->total,done=j->done,speed=j->speed;
        std::wstring progress=total?std::to_wstring((uint64_t)std::min(100.0,(double)done*100.0/(double)total))+L"%":L"Waiting";
        std::array<std::wstring,8> columns={name,names[j->status],progress,size(done)+L" / "+(total?size(total):L"?"),speed?size(speed)+L"/s":L"—",std::to_wstring(j->lanes.load()),downloadType(path),downloadDate(j->addedAt)};
        if(i>=(int)renderedRows.size())renderedRows.resize(i+1);
        for(int col=0;col<8;++col)if(renderedRows[i][col]!=columns[col]){renderedRows[i][col]=columns[col];firstChanged=std::min(firstChanged,i);lastChanged=i;}
    }
    if(ListView_GetItemCount(list)!=(int)jobs.size())ListView_SetItemCountEx(list,(int)jobs.size(),LVSICF_NOINVALIDATEALL|LVSICF_NOSCROLL);
    if(scrollToNewest&&!jobs.empty()){ListView_EnsureVisible(list,(int)jobs.size()-1,FALSE);scrollToNewest=false;}
    if(lastChanged>=firstChanged)ListView_RedrawItems(list,firstChanged,lastChanged);
    auto summary=std::to_wstring(active)+L" active     "+std::to_wstring(queued)+L" queued     "+std::to_wstring(complete)+L" complete     "+size(totalSpeed)+L"/s";
    if(summary!=queueSummary){queueSummary=summary;RECT r;GetClientRect(window,&r);r.top=56;r.bottom=90;InvalidateRect(window,&r,FALSE);}
    std::wstring description;
    auto choice=selected();if(!choice.empty()){auto& j=jobs[choice.front()];{std::lock_guard<std::mutex> lock(j->mutex);description=j->error.empty()?j->url+L"  →  "+j->path:j->error;}if(j->active&&j->stop)description=j->resumeRequested?L"Waiting to resume…":L"Pausing download…";}
    else description=L"";
    if(!fileTasksDone())description=L"Processing your URL list in the background…";
    else if(importFailed)description=std::to_wstring(importFailed)+L" URLs could not be imported. Check their format and queue capacity.";
    if(queueWriter&&!queueWriter->failure().empty())description=L"Queue could not be saved: "+utf16(queueWriter->failure());
    if(closing){description=L"Finishing active transfers safely…";if(workers.empty()&&fileTasksDone()){if(!shutdownSaved){save();shutdownSaved=true;}if(queueWriter->idle()){KillTimer(window,1);DestroyWindow(window);return;}}}
    if(text(detail)!=description)SetWindowTextW(detail,description.c_str());
}
void paintWindow(HWND h,HDC provided=nullptr) {PAINTSTRUCT ps;HDC dc=provided?provided:BeginPaint(h,&ps);RECT r;GetClientRect(h,&r);FillRect(dc,&r,backgroundBrush);

    label(dc,L"Download queue",{28,56,300,90},mediumFont,ink);label(dc,queueSummary,{300,56,r.right-28,90},smallFont,muted,DT_RIGHT|DT_SINGLELINE|DT_VCENTER);
    rounded(dc,{24,r.bottom-122,r.right-24,r.bottom-62},RGB(255,255,255),border);if(!provided)EndPaint(h,&ps);
}
LRESULT drawList(NMLVCUSTOMDRAW* draw) {
    if(draw->nmcd.dwDrawStage==CDDS_PREPAINT)return CDRF_NOTIFYITEMDRAW;
    if(draw->nmcd.dwDrawStage==CDDS_ITEMPREPAINT){draw->clrText=ink;draw->clrTextBk=(draw->nmcd.dwItemSpec%2)?RGB(249,251,254):RGB(255,255,255);return CDRF_NOTIFYSUBITEMDRAW;}
    if(draw->nmcd.dwDrawStage==(CDDS_ITEMPREPAINT|CDDS_SUBITEM)){
        int row=(int)draw->nmcd.dwItemSpec,col=draw->iSubItem;if(row<0||row>=(int)jobs.size())return CDRF_DODEFAULT;
        auto& j=jobs[row];RECT rect{};ListView_GetSubItemRect(list,row,col,LVIR_BOUNDS,&rect);bool selectedRow=(ListView_GetItemState(list,row,LVIS_SELECTED)&LVIS_SELECTED)!=0;
        if(col==1||col==2){auto brush=CreateSolidBrush(selectedRow?RGB(226,237,255):((row%2)?RGB(249,251,254):RGB(255,255,255)));FillRect(draw->nmcd.hdc,&rect,brush);DeleteObject(brush);
            if(col==1){int status=j->status;COLORREF fill=RGB(232,238,246),textColor=muted;if(status==Complete){fill=RGB(220,247,235);textColor=RGB(21,128,83);}if(status==Downloading){fill=RGB(224,235,255);textColor=blue;}if(status==Error){fill=RGB(255,230,229);textColor=RGB(190,49,49);}if(status==Paused||status==Scheduled){fill=RGB(255,243,212);textColor=RGB(157,104,15);}RECT badge{rect.left+6,rect.top+6,rect.right-6,rect.bottom-6};rounded(draw->nmcd.hdc,badge,fill,fill,10);label(draw->nmcd.hdc,names[status],badge,smallFont,textColor,DT_CENTER|DT_VCENTER|DT_SINGLELINE);}
            else {auto total=j->total.load(),done=j->done.load();double ratio=total?std::min(1.0,(double)done/total):0;RECT track{rect.left+10,rect.top+12,rect.right-10,rect.bottom-12};rounded(draw->nmcd.hdc,track,RGB(225,232,243),RGB(225,232,243),8);RECT fill=track;fill.right=fill.left+(int)((track.right-track.left)*ratio);if(fill.right>fill.left)rounded(draw->nmcd.hdc,fill,j->status==Complete?RGB(25,155,107):blue,j->status==Complete?RGB(25,155,107):blue,8);std::wstring value=total?std::to_wstring((int)(ratio*100))+L"%":L"—";label(draw->nmcd.hdc,value,rect,smallFont,ink,DT_CENTER|DT_SINGLELINE|DT_VCENTER);}
            return CDRF_SKIPDEFAULT;
        }
    }
    return CDRF_DODEFAULT;
}
LRESULT CALLBACK procedure(HWND h,UINT message,WPARAM wp,LPARAM lp) {
    static UINT taskbarCreated=RegisterWindowMessageW(L"TaskbarCreated");if(message==taskbarCreated){trayInstalled=false;installTray();return 0;}
    try {switch(message){
    case WM_CREATE:{
        // Firefox may run at a lower integrity level than an app started by an administrator.
        ChangeWindowMessageFilterEx(h,WM_COPYDATA,MSGFLT_ALLOW,nullptr);ChangeWindowMessageFilterEx(h,WM_APP+71,MSGFLT_ALLOW,nullptr);ChangeWindowMessageFilterEx(h,WM_APP+73,MSGFLT_ALLOW,nullptr);
        window=h;modernWindow(h);auto makeFont=[](int px,int weight){return CreateFontW(-px,0,0,0,weight,FALSE,FALSE,FALSE,DEFAULT_CHARSET,OUT_DEFAULT_PRECIS,CLIP_DEFAULT_PRECIS,CLEARTYPE_QUALITY,DEFAULT_PITCH,L"Segoe UI");};font=makeFont(16,FW_NORMAL);titleFont=makeFont(27,FW_SEMIBOLD);smallFont=makeFont(13,FW_NORMAL);mediumFont=makeFont(17,FW_SEMIBOLD);backgroundBrush=CreateSolidBrush(background);whiteBrush=CreateSolidBrush(RGB(255,255,255));
        control(L"STATIC",L"Link",140,0,44,164,60,24);urlBox=control(L"EDIT",L"",100,ES_AUTOHSCROLL|WS_TABSTOP,108,158,750,30);SendMessageW(urlBox,EM_SETCUEBANNER,FALSE,(LPARAM)L"Paste a download URL or YouTube video link");control(L"BUTTON",L"Paste link",105,WS_TABSTOP,900,157,108,32);
        control(L"STATIC",L"Save to",141,0,44,206,60,24);wchar_t home[MAX_PATH];SHGetFolderPathW(nullptr,CSIDL_PROFILE,nullptr,0,home);auto download=fs::path(home)/L"Downloads";folderBox=control(L"EDIT",download.c_str(),101,ES_AUTOHSCROLL|WS_TABSTOP,108,200,750,30);control(L"BUTTON",L"Browse…",102,WS_TABSTOP,900,199,108,32);
        videoHint=control(L"STATIC",L"Files and YouTube videos supported",143,0,44,247,650,24);SendMessageW(videoHint,WM_SETFONT,(WPARAM)smallFont,TRUE);control(L"BUTTON",L"Download",104,WS_TABSTOP,900,241,160,34);
        buttonTips=CreateWindowExW(WS_EX_TOPMOST,TOOLTIPS_CLASSW,nullptr,WS_POPUP|TTS_ALWAYSTIP|TTS_NOPREFIX,CW_USEDEFAULT,CW_USEDEFAULT,CW_USEDEFAULT,CW_USEDEFAULT,h,nullptr,GetModuleHandleW(nullptr),nullptr);
        tip(GetDlgItem(h,105),L"Paste link");tip(GetDlgItem(h,102),L"Choose save folder");
        const wchar_t* captions[]={L"Resume",L"Stop download",L"Open folder",L"Import URLs",L"Export list",L"Remove selected download"};int ids[]={120,112,113,114,115,116};for(int i=0;i<6;++i){auto button=control(L"BUTTON",captions[i],ids[i],WS_TABSTOP,24+i*58,308,48,36);tip(button,captions[i]);}
        control(L"BUTTON",L"New download",122,WS_TABSTOP,24,20,44,36);tip(GetDlgItem(h,122),L"New download");
        control(L"BUTTON",L"Clear all",121,WS_TABSTOP,950,220,110,36);
        control(L"BUTTON",L"Settings",117,WS_TABSTOP,44,645,120,32);control(L"BUTTON",L"About",118,WS_TABSTOP,1000,645,96,32);
        control(L"BUTTON",L"Organize",123,WS_TABSTOP,424,20,122,36);
        list=control(WC_LISTVIEWW,L"",130,LVS_REPORT|LVS_OWNERDATA|LVS_SHOWSELALWAYS|WS_TABSTOP,24,390,1000,300);ListView_SetExtendedListViewStyle(list,LVS_EX_FULLROWSELECT|LVS_EX_DOUBLEBUFFER);SetWindowTheme(list,L"Explorer",nullptr);ListView_SetBkColor(list,RGB(255,255,255));ListView_SetTextBkColor(list,RGB(255,255,255));
        rowImages=ImageList_Create(1,38,ILC_COLOR32,1,1);ListView_SetImageList(list,rowImages,LVSIL_SMALL);
        const wchar_t* titles[]={L"File",L"Status",L"Progress",L"Received / Total",L"Speed",L"Connections",L"Type",L"Date added"};int widths[]={300,118,125,180,110,100,120,205};for(int i=0;i<8;++i){LVCOLUMNW col{};col.mask=LVCF_TEXT|LVCF_WIDTH;col.pszText=(LPWSTR)titles[i];col.cx=widths[i];ListView_InsertColumn(list,i,&col);}
        emptyHint=control(L"STATIC",L"No downloads yet\nPaste a file or YouTube link above.",160,SS_CENTER,400,500,420,60);SendMessageW(emptyHint,WM_SETFONT,(WPARAM)mediumFont,TRUE);
        detail=control(L"STATIC",L"",144,SS_ENDELLIPSIS|SS_NOPREFIX,28,720,1000,36);SendMessageW(detail,WM_SETFONT,(WPARAM)smallFont,TRUE);SetTimer(h,1,150,nullptr);installTray();layout();update();return 0;
    }
    case WM_PAINT:paintWindow(h);return 0;
    case WM_PRINTCLIENT:paintWindow(h,(HDC)wp);return 0;
    case WM_ERASEBKGND:return 1;
    case WM_SIZE:if(list)layout();return 0;
    case WM_GETMINMAXINFO:((MINMAXINFO*)lp)->ptMinTrackSize={1060,760};return 0;
    case WM_CTLCOLOREDIT:SetTextColor((HDC)wp,ink);SetBkColor((HDC)wp,RGB(255,255,255));return (LRESULT)whiteBrush;
    case WM_CTLCOLORSTATIC:{int id=GetDlgCtrlID((HWND)lp);SetTextColor((HDC)wp,(id==143||id==160)?muted:ink);SetBkMode((HDC)wp,TRANSPARENT);return (LRESULT)(id==144?backgroundBrush:whiteBrush);}
    case WM_DRAWITEM:return flatButton((DRAWITEMSTRUCT*)lp);
    case WM_NOTIFY:{auto n=(NMHDR*)lp;if(n->hwndFrom==list&&n->code==LVN_COLUMNCLICK){auto col=((NMLISTVIEW*)lp)->iSubItem;sortDownloads(col,col==sortColumn?!sortAscending:true);return 0;}if(n->hwndFrom==list&&n->code==LVN_ITEMCHANGED){updateActionButton();return 0;}if(n->hwndFrom==list&&n->code==LVN_GETDISPINFOW){auto info=(NMLVDISPINFOW*)lp;int row=info->item.iItem,col=info->item.iSubItem;if((info->item.mask&LVIF_TEXT)&&info->item.pszText&&info->item.cchTextMax>0){const wchar_t* value=(row>=0&&row<(int)renderedRows.size()&&col>=0&&col<8)?renderedRows[row][col].c_str():L"";wcsncpy_s(info->item.pszText,(size_t)info->item.cchTextMax,value,_TRUNCATE);}return 0;}if(n->hwndFrom==list&&n->code==NM_CUSTOMDRAW)return drawList((NMLVCUSTOMDRAW*)lp);if(n->hwndFrom==list&&n->code==NM_DBLCLK){auto ids=selected();if(!ids.empty()&&jobs[ids.front()]->status==Complete){auto& j=jobs[ids.front()];std::wstring path;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;}openWithShell(path);}return 0;}break;}
    case WM_APP+73:finishHandoffCheck((DWORD)wp,lp==1);return TRUE;
    case WM_COPYDATA:{if(closing)return FALSE;auto data=(COPYDATASTRUCT*)lp;if(data->dwData!=0x4f444d33||data->cbData<2||data->cbData>65536)return FALSE;try{auto request=nlohmann::json::parse((const char*)data->lpData,(const char*)data->lpData+data->cbData);requestDownload(utf16(request.at("url").get<std::string>()),request.value("promptId",(DWORD)0),utf16(request.value("filename",std::string())),!request.value("checking",false),utf16(request.value("userAgent",std::string())),utf16(request.value("referer",std::string())));return TRUE;}catch(...){return FALSE;}}
    case HandoffDecision:{auto it=handoffDecisions.find((DWORD)wp);if(it==handoffDecisions.end())return 3;int result=it->second;if(result)handoffDecisions.erase(it);return result;}
    case TrayMessage:if(lp==WM_LBUTTONUP||lp==WM_LBUTTONDBLCLK){showApp();return 0;}if(lp==WM_RBUTTONUP||lp==WM_CONTEXTMENU){auto menu=CreatePopupMenu();AppendMenuW(menu,MF_STRING,180,L"Open Download Manager");AppendMenuW(menu,MF_STRING,117,L"Settings");AppendMenuW(menu,MF_SEPARATOR,0,nullptr);AppendMenuW(menu,MF_STRING,119,L"Exit");POINT pt;GetCursorPos(&pt);SetForegroundWindow(h);int id=TrackPopupMenu(menu,TPM_RETURNCMD|TPM_RIGHTBUTTON,pt.x,pt.y,0,h,nullptr);DestroyMenu(menu);if(id==180)showApp();else if(id)SendMessageW(h,WM_COMMAND,id,0);return 0;}break;
    case WM_CONTEXTMENU:if((HWND)wp==list){POINT pt{(short)LOWORD(lp),(short)HIWORD(lp)};if(pt.x==-1){RECT r;GetWindowRect(list,&r);pt={r.left+30,r.top+40};}else{auto local=pt;ScreenToClient(list,&local);LVHITTESTINFO hit{};hit.pt=local;int row=ListView_HitTest(list,&hit);if(row>=0&&!ListView_GetItemState(list,row,LVIS_SELECTED)){ListView_SetItemState(list,-1,0,LVIS_SELECTED);ListView_SetItemState(list,row,LVIS_SELECTED|LVIS_FOCUSED,LVIS_SELECTED|LVIS_FOCUSED);}}
        auto ids=selected();if(ids.empty())return 0;auto j=jobs[ids.front()];auto menu=CreatePopupMenu();AppendMenuW(menu,MF_STRING|(j->status==Complete?0:MF_GRAYED),170,L"Open file");AppendMenuW(menu,MF_STRING,113,L"Open folder");AppendMenuW(menu,MF_SEPARATOR,0,nullptr);AppendMenuW(menu,MF_STRING,110,L"Resume / Retry");AppendMenuW(menu,MF_STRING,111,L"Pause");AppendMenuW(menu,MF_STRING,112,L"Cancel download");AppendMenuW(menu,MF_SEPARATOR,0,nullptr);AppendMenuW(menu,MF_STRING,171,L"Copy download link");AppendMenuW(menu,MF_STRING|((j->active||j->status==Complete)?MF_GRAYED:0),173,L"Renew download link...");AppendMenuW(menu,MF_STRING|(j->active?MF_GRAYED:0),172,L"Remove from list");SetForegroundWindow(h);int id=TrackPopupMenu(menu,TPM_RETURNCMD|TPM_RIGHTBUTTON,pt.x,pt.y,0,h,nullptr);DestroyMenu(menu);PostMessageW(h,WM_NULL,0,0);if(id)SendMessageW(h,WM_COMMAND,id,0);return 0;}break;
    case WM_TIMER:update();if(!closing){static int ticks=0;if(++ticks%28==0)save();}return 0;
    case WM_COMMAND:{if(closing)return 0;int id=LOWORD(wp);
        if(id==100&&HIWORD(wp)==EN_CHANGE){SetWindowTextW(videoHint,isYouTube(text(urlBox))?L"YouTube detected · video + audio":L"Files and YouTube videos supported");return 0;}
        if(id==105&&OpenClipboard(h)){auto data=GetClipboardData(CF_UNICODETEXT);if(data){auto p=(wchar_t*)GlobalLock(data);if(p){SetWindowTextW(urlBox,p);GlobalUnlock(data);}}CloseClipboard();SetFocus(urlBox);}
        if(id==102){BROWSEINFOW b{};b.hwndOwner=h;b.lpszTitle=L"Select download folder";b.ulFlags=BIF_RETURNONLYFSDIRS|BIF_NEWDIALOGSTYLE;auto pid=SHBrowseForFolderW(&b);if(pid){wchar_t p[MAX_PATH];if(SHGetPathFromIDListW(pid,p))SetWindowTextW(folderBox,p);CoTaskMemFree(pid);}}
        if(id==122){showAddDownload();return 0;}
        if(id==173){auto ids=selected();if(!ids.empty())showRenewLink(jobs[ids.front()]);return 0;}
        if(id==123){auto menu=CreatePopupMenu();AppendMenuW(menu,MF_STRING,190,L"By status (incomplete / complete)");AppendMenuW(menu,MF_STRING,191,L"By file type");RECT rect;GetWindowRect(GetDlgItem(h,123),&rect);int choice=TrackPopupMenu(menu,TPM_RETURNCMD,rect.left,rect.bottom,0,h,nullptr);DestroyMenu(menu);if(choice)organizeDownloads(choice==191);return 0;}
        if(id==190||id==191){organizeDownloads(id==191);return 0;}
        if(id==104){requestDownload(text(urlBox));SetWindowTextW(urlBox,L"");}
        if(id==117)showSettings();
        if(id==119){SendMessageW(h,WM_APP+72,0,0);return 0;}
        if(id==120){for(int i:selected()){auto& j=jobs[i];int state=j->status;if(state==Downloading||state==Queued){j->resumeRequested=false;j->status=Paused;j->stop=true;}else if(state==Paused||state==Error||state==Scheduled){showProgress(j);j->schedule.clear();if(j->active)j->resumeRequested=true;else j->status=Queued;}}save();updateActionButton();return 0;}
        if(id==110||id==111||id==112){for(int i:selected()){auto& j=jobs[i];if(id==110&&(j->status==Paused||j->status==Error||j->status==Scheduled)){showProgress(j);j->schedule.clear();if(j->active)j->resumeRequested=true;else j->status=Queued;}if(id==111&&j->status!=Complete&&j->status!=Canceled){j->resumeRequested=false;j->status=Paused;j->stop=true;}if(id==112&&j->status!=Complete){j->resumeRequested=false;j->status=Canceled;j->stop=true;}}save();}
        if(id==170){auto ids=selected();if(!ids.empty()&&jobs[ids.front()]->status==Complete){std::wstring path;{std::lock_guard<std::mutex> lock(jobs[ids.front()]->mutex);path=jobs[ids.front()]->path;}openWithShell(path);}}
        if(id==171){auto ids=selected();if(!ids.empty()&&OpenClipboard(h)){auto value=jobs[ids.front()]->url;auto memory=GlobalAlloc(GMEM_MOVEABLE,(value.size()+1)*sizeof(wchar_t));if(memory){auto ptr=GlobalLock(memory);if(ptr){memcpy(ptr,value.c_str(),(value.size()+1)*sizeof(wchar_t));GlobalUnlock(memory);EmptyClipboard();if(!SetClipboardData(CF_UNICODETEXT,memory))GlobalFree(memory);}else GlobalFree(memory);}CloseClipboard();}}
        if(id==172){auto ids=selected();ListView_SetItemCountEx(list,0,0);for(auto it=ids.rbegin();it!=ids.rend();++it)if(!jobs[*it]->active)jobs.erase(jobs.begin()+*it);renderedRows.clear();queuePaths.clear();nextSuffix.clear();save();update();}
        if(id==113){auto ids=selected();if(!ids.empty()){auto& j=jobs[ids[0]];std::wstring path;{std::lock_guard<std::mutex> lock(j->mutex);path=j->path;}openWithShell(fs::path(path).parent_path().wstring());}}
        if(id==114){if(importTask.valid()||!pendingImport.empty())throw std::runtime_error("An import is already in progress");auto path=fileDialog(false);if(!path.empty()){importFolder=text(folderBox);importFailed=0;importTask=std::async(std::launch::async,[path]{std::ifstream in(fs::path(path),std::ios::binary);if(!in)throw std::runtime_error("Cannot open URL list");std::vector<std::wstring> lines;std::string line;size_t bytes=0;while(std::getline(in,line)){bytes+=line.size();if(bytes>10*1024*1024||lines.size()>=10000)throw std::runtime_error("URL list exceeds the 10 MB or 10,000 URL limit");if(!line.empty()&&line.back()=='\r')line.pop_back();if(line.rfind("\xEF\xBB\xBF",0)==0)line.erase(0,3);if(line.empty()||line[0]=='#')continue;if(line.size()>16384)throw std::runtime_error("A URL in the list is too long");lines.push_back(utf16(line));}if(!in.eof())throw std::runtime_error("Cannot read URL list");return lines;});}}
        if(id==115){if(exportTask.valid())throw std::runtime_error("An export is already in progress");auto path=fileDialog(true);if(!path.empty()){std::string bytes;for(auto& j:jobs)bytes+=utf8(j->url)+"\r\n";exportTask=std::async(std::launch::async,[path,bytes=std::move(bytes)]{std::ofstream out(fs::path(path),std::ios::binary);if(!out)throw std::runtime_error("Cannot write URL list");out.write(bytes.data(),bytes.size());out.flush();if(!out)throw std::runtime_error("URL export failed");});}}
        if(id==116){auto ids=selected();if(!ids.empty()){auto index=ids.front();if(!jobs[index]->active){jobs.erase(jobs.begin()+index);renderedRows.clear();queuePaths.clear();nextSuffix.clear();ListView_SetItemCountEx(list,0,0);save();update();}}return 0;}
        if(id==121){ListView_SetItemCountEx(list,0,0);jobs.erase(std::remove_if(jobs.begin(),jobs.end(),[](auto& j){return !j->active;}),jobs.end());ListView_SetItemCountEx(list,0,0);renderedRows.clear();queuePaths.clear();nextSuffix.clear();save();update();}
        if(id==118)MessageBoxW(h,L"Open Download Manager 2.6.0\n\nNative C++ application · MIT license\nParallel HTTP/HTTPS downloads with validated resume\nYouTube support by yt-dlp, Deno and FFmpeg\n\nVideo support is bundled into this executable.\nDouble-click a completed download to open it.",L"About Open Download Manager",MB_OK|MB_ICONINFORMATION);
        return 0;
    }
    case WM_CLOSE:if(!closing){if(!trayInstalled)installTray();ShowWindow(h,SW_HIDE);trayNotice(L"Running in the tray",L"Downloads continue. Right-click the tray icon to open the app or exit.");return 0;}SendMessageW(h,WM_APP+72,0,0);return 0;
    case WM_APP+72:if(!closing){closing=true;for(auto& choice:downloadChoices)if(choice.id)handoffDecisions[choice.id]=2;downloadChoices.clear();for(auto dlg:{addDialog,downloadDialog,settingsDialog,completionDialog})if(dlg)DestroyWindow(dlg);downloadDialog=settingsDialog=completionDialog=nullptr;for(HWND child=GetWindow(h,GW_CHILD);child;child=GetWindow(child,GW_HWNDNEXT)){wchar_t cls[64];GetClassNameW(child,cls,64);if(wcscmp(cls,L"Button")==0||wcscmp(cls,L"Edit")==0)EnableWindow(child,FALSE);}
        for(auto it=progressWindows.begin();it!=progressWindows.end();){auto dlg=it->first;++it;DestroyWindow(dlg);}
        for(auto& j:jobs){j->resumeRequested=false;if(j->active){int expected=Downloading;j->status.compare_exchange_strong(expected,Paused);j->stop=true;}}SetWindowTextW(h,L"Open Download Manager — Finishing downloads");}return 0;
    case WM_DESTROY:if(trayInstalled)Shell_NotifyIconW(NIM_DELETE,&tray);DeleteObject(font);DeleteObject(titleFont);DeleteObject(smallFont);DeleteObject(mediumFont);DeleteObject(backgroundBrush);DeleteObject(whiteBrush);if(rowImages)ImageList_Destroy(rowImages);PostQuitMessage(0);return 0;
    }}catch(const std::exception& e){MessageBoxW(h,utf16(e.what()).c_str(),L"Open Download Manager",MB_OK|MB_ICONERROR);}
    return DefWindowProcW(h,message,wp,lp);
}


