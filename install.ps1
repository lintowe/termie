# termie installer - builds release, copies to %LOCALAPPDATA%\Programs\termie,
# generates an icon, and creates Start Menu + Desktop shortcuts.
# usage:  powershell -ExecutionPolicy Bypass -File install.ps1
#         (flags: -NoBuild  -NoPath  -InstallDir <path>)
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\termie'),
    [switch]$NoBuild,
    [switch]$NoPath
)
$ErrorActionPreference = 'Stop'
$repo = $PSScriptRoot
Write-Host "==> installing termie" -ForegroundColor Cyan

# 1. build the release binary
if (-not $NoBuild) {
    Write-Host "    building (cargo build --release)..."
    Push-Location $repo
    try { & cargo build --release } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw "cargo build --release failed" }
}
$exe = Join-Path $repo 'target\release\termie.exe'
if (-not (Test-Path $exe)) { throw "release binary not found: $exe (build first)" }

# 2. copy exe + assets (termie loads fonts from <exe dir>\assets\fonts)
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$destExe = Join-Path $InstallDir 'termie.exe'
try {
    Copy-Item $exe $destExe -Force
} catch {
    throw "could not write $destExe - is termie running? close it and re-run. ($_)"
}
# modern ConPTY host beside the exe: the inbox conhost strips sixel, the
# current OpenConsole passes it through (termie prefers a sideloaded pair)
& pwsh -NoProfile -File (Join-Path $repo 'setup\fetch-conpty.ps1') -Dest $InstallDir

$assets = Join-Path $repo 'assets'
if (Test-Path $assets) {
    $destAssets = Join-Path $InstallDir 'assets'
    # remove first so re-runs don't nest assets\assets via Copy-Item dir merge
    if (Test-Path $destAssets) { Remove-Item $destAssets -Recurse -Force }
    Copy-Item $assets $destAssets -Recurse -Force
}

# 3. generate a multi-resolution shortcut icon (.ico) from assets/icon.png -
#    best effort; shortcuts still work without it. each size is its own native
#    frame (downscaled from the 1024 master with high-quality resampling) so the
#    shell picks a real frame instead of downscaling a single one badly
$icoPath = Join-Path $InstallDir 'termie.ico'
try {
    Add-Type -AssemblyName System.Drawing
    $sizes = 16, 24, 32, 48, 64, 128, 256
    $master = New-Object System.Drawing.Bitmap((Join-Path $repo 'assets\icon.png'))
    $pngs = @()
    foreach ($sz in $sizes) {
        $bmp = New-Object System.Drawing.Bitmap($sz, $sz, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        $g.Clear([System.Drawing.Color]::Transparent)
        $g.DrawImage($master, [System.Drawing.Rectangle]::new([int]0, [int]0, [int]$sz, [int]$sz))
        $g.Dispose()
        $ms = New-Object System.IO.MemoryStream
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $pngs += , ($ms.ToArray())
        $bmp.Dispose()
    }
    $master.Dispose()
    # pack the native PNG frames into a multi-image ICO container
    $fs = [System.IO.File]::Create($icoPath)
    $bw = New-Object System.IO.BinaryWriter($fs)
    $bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $b = $sizes[$i] -band 0xFF
        $bw.Write([byte]$b); $bw.Write([byte]$b); $bw.Write([byte]0); $bw.Write([byte]0)
        $bw.Write([uint16]1); $bw.Write([uint16]32)
        $bw.Write([uint32]$pngs[$i].Length); $bw.Write([uint32]$offset)
        $offset += $pngs[$i].Length
    }
    foreach ($p in $pngs) { $bw.Write($p) }
    $bw.Flush(); $fs.Close()
} catch {
    Write-Warning "icon generation failed ($_); shortcuts will use the default icon"
    $icoPath = $null
}

# 4. Start Menu + Desktop shortcuts
$ws = New-Object -ComObject WScript.Shell
function New-TermieShortcut([string]$lnk) {
    $sc = $ws.CreateShortcut($lnk)
    $sc.TargetPath = $destExe
    $sc.WorkingDirectory = $env:USERPROFILE
    $sc.Description = 'termie - terminal'
    if ($icoPath) { $sc.IconLocation = $icoPath }
    $sc.Save()
}
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\termie.lnk'
New-TermieShortcut $startMenu
$desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'termie.lnk'
New-TermieShortcut $desktop

# 5. add install dir to the user PATH (so `termie` works from any shell)
if (-not $NoPath) {
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $userPath) { $userPath = '' }
    $has = $userPath.Split(';') | Where-Object { $_.TrimEnd('\') -ieq $InstallDir.TrimEnd('\') }
    if (-not $has) {
        [Environment]::SetEnvironmentVariable('Path', ($userPath.TrimEnd(';') + ';' + $InstallDir), 'User')
        Write-Host "    added to user PATH (restart shells to pick it up)"
    }
}

# 6. "Open in termie" Explorer context-menu verb (right-click a folder or its
#    background) plus an App Paths entry so Win+R / ShellExecute resolve `termie`.
#    HKCU only, so this needs no elevation. %V is the clicked folder
$cmd = '"' + $destExe + '" --cwd "%V"'
foreach ($root in @('HKCU:\Software\Classes\Directory\shell\termie',
                    'HKCU:\Software\Classes\Directory\Background\shell\termie')) {
    New-Item -Path $root -Force | Out-Null
    Set-ItemProperty -Path $root -Name '(default)' -Value 'Open in termie'
    if ($icoPath) { Set-ItemProperty -Path $root -Name 'Icon' -Value $icoPath }
    New-Item -Path "$root\command" -Force | Out-Null
    Set-ItemProperty -Path "$root\command" -Name '(default)' -Value $cmd
}
$appPaths = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\termie.exe'
New-Item -Path $appPaths -Force | Out-Null
Set-ItemProperty -Path $appPaths -Name '(default)' -Value $destExe
Write-Host "    registered 'Open in termie' context menu + App Paths"

# 7. refresh the shell icon cache so the new icon shows immediately (the shell
#    caches shortcut icons and would otherwise keep showing the old one)
try { & ie4uinit.exe -ClearIconCache 2>$null; & ie4uinit.exe -show 2>$null } catch {}

Write-Host "==> installed to $InstallDir" -ForegroundColor Green
Write-Host "    launch from the Start Menu / desktop, or run 'termie'"
