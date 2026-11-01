@echo off
setlocal
rem Weigh-In launcher for cmd: runs the prebuilt binary from libexec\TARGET\.
rem The msvc build wins unless the gnu build is newer, as in the sh launcher, B-05: a stale
rem native build must not shadow a fresh cross-build, since they share one ledger and versions
rem differ in journal mode. Falls back to a weighin.exe on PATH.
rem The age test is xcopy /D /L: it lists the gnu build only when that is newer than the msvc
rem one, and copies nothing. Matching the file name rather than the File-count summary keeps it
rem independent of the Windows display language; both tools are named by path so a Unix find
rem earlier on PATH, as Git for Windows can put there, is never picked up.
rem The binary runs outside any parenthesised block, and exit /b names its errorlevel, so the
rem caller sees the binary's exit code.
set "D=%~dp0"
set "M=%D%x86_64-pc-windows-msvc\weighin.exe"
set "G=%D%x86_64-pc-windows-gnu\weighin.exe"
set "P="
if exist "%M%" set "P=%M%"
if not exist "%G%" goto run
if not defined P set "P=%G%"
if "%P%"=="%G%" goto run
"%SystemRoot%\System32\xcopy.exe" /D /L /Y "%G%" "%D%x86_64-pc-windows-msvc" <nul 2>nul | "%SystemRoot%\System32\find.exe" /i "weighin.exe" >nul && set "P=%G%"
:run
if not defined P goto path
"%P%" %*
exit /b %ERRORLEVEL%
:path
weighin.exe %*
exit /b %ERRORLEVEL%
