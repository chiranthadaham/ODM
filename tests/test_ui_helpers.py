"""Drive the real Save/Start dialogs for older download regression scenarios."""
import ctypes,threading,time
from ctypes import wintypes as w
def start_dialog_driver():
    def run():
        u=ctypes.WinDLL('user32');u.FindWindowW.argtypes=[w.LPCWSTR,w.LPCWSTR];u.FindWindowW.restype=w.HWND
        u.GetDlgItem.argtypes=[w.HWND,ctypes.c_int];u.GetDlgItem.restype=w.HWND
        u.PostMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
        while True:
            h=u.FindWindowW('ODMWorkflow','New download — Save and Start')
            if h:u.PostMessageW(h,0x111,203,0)
            h=u.FindWindowW('ODMWorkflow','Download complete')
            if h:u.PostMessageW(h,0x111,2,0)
            time.sleep(.1)
    threading.Thread(target=run,daemon=True).start()
