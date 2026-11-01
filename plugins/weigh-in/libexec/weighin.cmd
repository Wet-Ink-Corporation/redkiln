@echo off
setlocal
rem The suite's shared plugin launcher for cmd (see the sh launcher beside it). Each plugin ships a
rem copy as libexec\BINARY.cmd, and the copy's own file name is the binary it runs.
rem The msvc build wins unless the gnu build is newer: a stale native build must not shadow a
rem fresh cross-build. The age test is xcopy /D /L, which lists the gnu build only when it is newer
rem and copies nothing; matching the file name keeps it independent of the display language.
rem Both tools are named by path, so a Unix find earlier on PATH is never picked up.
rem Then a BINARY.exe on PATH. With nothing to run, a hook call exits 0 (SessionStart says so in a
rem systemMessage); any other call exits 127. exit /b names its errorlevel, so the caller sees the
rem build's exit code. The message has no quotes, backslashes or cmd metacharacters, so the echoed
rem line is valid JSON as written.
set "N=%~n0"
set "D=%~dp0"
set "M=%D%x86_64-pc-windows-msvc\%N%.exe"
set "G=%D%x86_64-pc-windows-gnu\%N%.exe"
set "P="
if exist "%M%" set "P=%M%"
if not exist "%G%" goto run
if not defined P set "P=%G%"
if "%P%"=="%G%" goto run
"%SystemRoot%\System32\xcopy.exe" /D /L /Y "%G%" "%D%x86_64-pc-windows-msvc" <nul 2>nul | "%SystemRoot%\System32\find.exe" /i "%N%.exe" >nul && set "P=%G%"
:run
if not defined P goto path
"%P%" %*
exit /b %ERRORLEVEL%
:path
"%SystemRoot%\System32\where.exe" %N%.exe >nul 2>nul || goto nobinary
%N%.exe %*
exit /b %ERRORLEVEL%
:nobinary
if /i not "%~1"=="hook" goto missing
if /i "%~2"=="SessionStart" goto hint
exit /b 0
:hint
echo {"systemMessage":"%N% is off: no %N% build for Windows in this plugin's libexec and none on PATH. Install a release that ships one."}
exit /b 0
:missing
>&2 echo %N%: no %N% build under %D% and none on PATH
exit /b 127