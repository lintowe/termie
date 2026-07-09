# termie uninstaller - removes the installed copy, shortcuts, and PATH entry.
# usage:  powershell -ExecutionPolicy Bypass -File uninstall.ps1
[CmdletBinding()]
param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'Programs\termie')
)
$ErrorActionPreference = 'Continue'
Write-Host "==> uninstalling termie" -ForegroundColor Cyan

# stop any running instance launched from the install dir
Get-Process termie -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($InstallDir, [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object { $_.Kill() }
Start-Sleep -Milliseconds 200

# shortcuts
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\termie.lnk'
$desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'termie.lnk'
foreach ($lnk in @($startMenu, $desktop)) {
    if (Test-Path $lnk) { Remove-Item $lnk -Force; Write-Host "    removed $lnk" }
}

# install dir
if (Test-Path $InstallDir) {
    try { Remove-Item $InstallDir -Recurse -Force; Write-Host "    removed $InstallDir" }
    catch { Write-Warning "could not remove $InstallDir ($_) - is termie still running?" }
}

# registry: the "Open in termie" verb + App Paths entry (paired with install.ps1)
foreach ($key in @('HKCU:\Software\Classes\Directory\shell\termie',
                   'HKCU:\Software\Classes\Directory\Background\shell\termie',
                   'HKCU:\Software\Microsoft\Windows\CurrentVersion\App Paths\termie.exe')) {
    if (Test-Path $key) { Remove-Item $key -Recurse -Force; Write-Host "    removed $key" }
}

# default-terminal delegation: restore the previous pair if it points at termie
$termieClsid = '{D6F7E8A1-3C52-4B0F-9E6A-71B2C0A4F3D9}'
$clsidKey = "HKCU:\Software\Classes\CLSID\$termieClsid"
$startup = 'HKCU:\Console\%%Startup'
$current = (Get-ItemProperty -Path $startup -Name DelegationTerminal -ErrorAction SilentlyContinue).DelegationTerminal
if ($current -eq $termieClsid) {
    $zero = '{00000000-0000-0000-0000-000000000000}'
    $pc = (Get-ItemProperty -Path $clsidKey -Name PrevDelegationConsole -ErrorAction SilentlyContinue).PrevDelegationConsole
    $pt = (Get-ItemProperty -Path $clsidKey -Name PrevDelegationTerminal -ErrorAction SilentlyContinue).PrevDelegationTerminal
    Set-ItemProperty -Path $startup -Name DelegationConsole -Value ($(if ($pc) { $pc } else { $zero }))
    Set-ItemProperty -Path $startup -Name DelegationTerminal -Value ($(if ($pt) { $pt } else { $zero }))
    Write-Host "    restored default terminal delegation"
}
if (Test-Path $clsidKey) { Remove-Item $clsidKey -Recurse -Force; Write-Host "    removed $clsidKey" }

# PATH entry
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($userPath) {
    $kept = ($userPath.Split(';') | Where-Object { $_ -and ($_.TrimEnd('\') -ine $InstallDir.TrimEnd('\')) })
    [Environment]::SetEnvironmentVariable('Path', ($kept -join ';'), 'User')
}

Write-Host "==> uninstalled" -ForegroundColor Green
