# Download tests

These files are development tests, kept separate from the application source. They are not needed to run or distribute ODM.

From the repository root, after building the app:

```powershell
python .\tests\test_downloads.py .\OpenDownloadManager.exe
python .\tests\test_segments.py .\OpenDownloadManager.exe
python .\tests\test_http_errors.py .\OpenDownloadManager.exe
python .\tests\test_optimizations.py .\OpenDownloadManager.exe
```

Python 3 is required. Tests use local HTTP servers and temporary download folders; no public website downloads are needed. `test_ui_helpers.py` contains optional UI test support.