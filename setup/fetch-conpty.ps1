# fetch the up-to-date ConPTY host (conpty.dll + OpenConsole.exe) that termie
# sideloads next to its exe. the inbox conhost strips DCS/APC it doesn't
# understand, so sixel through the stock CreatePseudoConsole never reaches the
# terminal; the current OpenConsole passes it through. pinned + hash-verified.
#
#   pwsh -File setup\fetch-conpty.ps1 -Dest target\release
param(
    [Parameter(Mandatory)] [string]$Dest
)
$ErrorActionPreference = 'Stop'

$version = '1.24.260512001'
$pkgHash = 'F889A9272A8B257DC6D5BE7525626FDB0F7CA6B5CE7E13093FC4BC979D24F484'
$dllHash = 'C46DCD04F52B97F6A8CF53E8F547C85A821660BED18DE2B3344AFCD4A8389AD6'
$exeHash = '47828C3FE080212F69DFDB39AB3673170FCC7445924C76FE003CEFD18247DD5D'

New-Item -ItemType Directory -Force $Dest | Out-Null
$dll = Join-Path $Dest 'conpty.dll'
$exe = Join-Path $Dest 'OpenConsole.exe'
if ((Test-Path $dll) -and (Test-Path $exe) -and
    (Get-FileHash $dll).Hash -eq $dllHash -and (Get-FileHash $exe).Hash -eq $exeHash) {
    Write-Host "    conpty $version already present"
    return
}

$work = Join-Path ([IO.Path]::GetTempPath()) "conpty-$version"
Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
New-Item -ItemType Directory $work | Out-Null
$zip = Join-Path $work 'pkg.zip'
Write-Host "    downloading Microsoft.Windows.Console.ConPTY $version..."
Invoke-WebRequest "https://www.nuget.org/api/v2/package/Microsoft.Windows.Console.ConPTY/$version" `
    -OutFile $zip -UseBasicParsing
if ((Get-FileHash $zip).Hash -ne $pkgHash) { throw 'conpty package hash mismatch' }
Expand-Archive $zip (Join-Path $work 'x')

Copy-Item (Join-Path $work 'x\runtimes\win-x64\native\conpty.dll') $dll -Force
Copy-Item (Join-Path $work 'x\build\native\runtimes\x64\OpenConsole.exe') $exe -Force
if ((Get-FileHash $dll).Hash -ne $dllHash) { throw 'conpty.dll hash mismatch' }
if ((Get-FileHash $exe).Hash -ne $exeHash) { throw 'OpenConsole.exe hash mismatch' }
Remove-Item -Recurse -Force $work
Write-Host "    conpty $version installed to $Dest"
