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
$assets = Join-Path $repo 'assets'
if (Test-Path $assets) {
    $destAssets = Join-Path $InstallDir 'assets'
    # remove first so re-runs don't nest assets\assets via Copy-Item dir merge
    if (Test-Path $destAssets) { Remove-Item $destAssets -Recurse -Force }
    Copy-Item $assets $destAssets -Recurse -Force
}

# 3. generate a multi-resolution shortcut icon (.ico) - best effort; shortcuts
#    still work without it. each size is rendered natively so the shell never
#    has to downscale a single 256px frame (which looked muddy at 16-32px)
$icoPath = Join-Path $InstallDir 'termie.ico'
try {
    Add-Type -AssemblyName System.Drawing
    $sizes = 16, 24, 32, 48, 64, 128, 256
    $pngs = @()
    foreach ($sz in $sizes) {
        $bmp = New-Object System.Drawing.Bitmap($sz, $sz, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $g.Clear([System.Drawing.Color]::Transparent)
        $s = [double]$sz
        # charcoal rounded square (flat instrument greyscale, no frame)
        $inset = $s * 0.06; $rad = $s * 0.225; $d = 2 * $rad
        $x0 = $inset; $y0 = $inset; $x1 = $s - $inset; $y1 = $s - $inset
        $path = New-Object System.Drawing.Drawing2D.GraphicsPath
        $path.AddArc($x0, $y0, $d, $d, 180, 90)
        $path.AddArc($x1 - $d, $y0, $d, $d, 270, 90)
        $path.AddArc($x1 - $d, $y1 - $d, $d, $d, 0, 90)
        $path.AddArc($x0, $y1 - $d, $d, $d, 90, 90)
        $path.CloseFigure()
        $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 26, 26, 26))), $path)
        # optically centered ">_" prompt mark in near-white; coords are
        # normalized (0..1) with a small optical nudge so it reads centered
        $nx = -0.014; $ny = -0.034
        $pt = { param($fx, $fy) New-Object System.Drawing.PointF([single](($fx + $nx) * $s), [single](($fy + $ny) * $s)) }
        $w = [Math]::Max(2.0, 0.085 * $s)
        $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 244, 244, 244)), ([single]$w)
        $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
        $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
        $pen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
        $chev = [System.Drawing.PointF[]]@( (& $pt 0.31 0.34), (& $pt 0.52 0.50), (& $pt 0.31 0.66) )
        $g.DrawLines($pen, $chev)
        $g.DrawLine($pen, (& $pt 0.55 0.66), (& $pt 0.735 0.66))
        $pen.Dispose(); $g.Dispose()
        $ms = New-Object System.IO.MemoryStream
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $pngs += , ($ms.ToArray())
        $bmp.Dispose()
    }
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

# 6. refresh the shell icon cache so the new icon shows immediately (the shell
#    caches shortcut icons and would otherwise keep showing the old one)
try { & ie4uinit.exe -ClearIconCache 2>$null; & ie4uinit.exe -show 2>$null } catch {}

Write-Host "==> installed to $InstallDir" -ForegroundColor Green
Write-Host "    launch from the Start Menu / desktop, or run 'termie'"
