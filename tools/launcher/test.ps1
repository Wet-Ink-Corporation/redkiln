# Tests for the cmd launcher. Run on Windows: pwsh tools/launcher/test.ps1 (CI runs it).
# A copy of cmd.exe stands in for a build: `demo.exe /c exit 3` exits 3.
$ErrorActionPreference = 'Stop'
$failed = $false
function Check($name, $expected, $actual) {
  if ("$expected" -eq "$actual") { Write-Output "ok   $name" }
  else { Write-Output "FAIL ${name}: expected [$expected], got [$actual]"; $script:failed = $true }
}
$work = Join-Path ([IO.Path]::GetTempPath()) ("launcher-" + [guid]::NewGuid())
$libexec = Join-Path $work 'libexec'
New-Item -ItemType Directory -Path $libexec | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'launcher.cmd') (Join-Path $libexec 'demo.cmd')
$demo = Join-Path $libexec 'demo.cmd'
$env:PATH = "$env:SystemRoot\System32"
try {
  $out = & $demo hook PreToolUse
  Check 'a hook with no build exits 0 silently' '0:' "${LASTEXITCODE}:$out"
  $out = & $demo hook SessionStart
  $message = ($out | ConvertFrom-Json).systemMessage
  Check 'SessionStart with no build warns and exits 0' '0:True' "${LASTEXITCODE}:$($message -like 'demo is off:*')"
  & $demo status 2>$null
  Check 'a plain call with no build exits 127' 127 $LASTEXITCODE

  $msvc = Join-Path $libexec 'x86_64-pc-windows-msvc'
  New-Item -ItemType Directory -Path $msvc | Out-Null
  Copy-Item "$env:SystemRoot\System32\cmd.exe" (Join-Path $msvc 'demo.exe')
  & $demo /c exit 3
  Check "runs the msvc build and passes its exit code through" 3 $LASTEXITCODE

  # hostname.exe as the msvc build exits nonzero on these arguments; cmd.exe as gnu exits 5.
  Copy-Item -Force "$env:SystemRoot\System32\hostname.exe" (Join-Path $msvc 'demo.exe')
  $gnu = Join-Path $libexec 'x86_64-pc-windows-gnu'
  New-Item -ItemType Directory -Path $gnu | Out-Null
  Copy-Item "$env:SystemRoot\System32\cmd.exe" (Join-Path $gnu 'demo.exe')
  (Get-Item (Join-Path $msvc 'demo.exe')).LastWriteTime = (Get-Date).AddDays(-1)
  (Get-Item (Join-Path $gnu 'demo.exe')).LastWriteTime = Get-Date
  & $demo /c exit 5 2>$null
  Check 'a newer gnu build wins' 5 $LASTEXITCODE
} finally {
  Remove-Item -Recurse -Force $work
}
# Exit explicitly: otherwise the last native command's code (the build's 5 above) leaks out.
if ($failed) { exit 1 }
exit 0
