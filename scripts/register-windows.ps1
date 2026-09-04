# Make smep show up in Windows' "Open with" list for .md/.markdown/.html
# (current user only, no admin). Windows keeps the *default* choice under
# its own protection: after running this, pick smep once via
# Settings > Apps > Default apps, or right-click a file > Open with > Always.
#
# Usage: scripts\register-windows.ps1 [-Exe path\to\smep.exe] [-Remove]
param(
    [string]$Exe = (Join-Path $env:USERPROFILE ".cargo\bin\smep.exe"),
    [switch]$Remove
)
$ErrorActionPreference = "Stop"
$progId = "smep.markdown"
$classes = "HKCU:\Software\Classes"

if ($Remove) {
    Remove-Item "$classes\$progId" -Recurse -Force -ErrorAction SilentlyContinue
    foreach ($ext in ".md", ".markdown", ".html", ".htm") {
        Remove-ItemProperty "$classes\$ext\OpenWithProgids" -Name $progId -ErrorAction SilentlyContinue
    }
    Write-Host "smep removed from Open with"
    exit 0
}

if (-not (Test-Path $Exe)) { throw "smep.exe not found at $Exe (pass -Exe)" }
$Exe = (Resolve-Path $Exe).Path

New-Item -Path "$classes\$progId\shell\open\command" -Force | Out-Null
Set-ItemProperty -Path "$classes\$progId" -Name "(default)" -Value "Markdown document (smep)"
Set-ItemProperty -Path "$classes\$progId" -Name "FriendlyTypeName" -Value "Markdown document"
New-Item -Path "$classes\$progId\DefaultIcon" -Force | Out-Null
Set-ItemProperty -Path "$classes\$progId\DefaultIcon" -Name "(default)" -Value "`"$Exe`",0"
Set-ItemProperty -Path "$classes\$progId\shell\open\command" -Name "(default)" -Value "`"$Exe`" `"%1`""

foreach ($ext in ".md", ".markdown", ".html", ".htm") {
    New-Item -Path "$classes\$ext\OpenWithProgids" -Force | Out-Null
    New-ItemProperty -Path "$classes\$ext\OpenWithProgids" -Name $progId -Value "" -PropertyType String -Force | Out-Null
}

Write-Host "smep registered for .md .markdown .html .htm (Open with). Now set it as default:"
Write-Host "  Settings > Apps > Default apps > Choose defaults by file type > .md > smep"
