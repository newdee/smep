# Store the six Apple signing/notarization secrets on the smep repository.
# Same six as magpie; GitHub cannot copy secrets between repositories, so
# this asks for each value once and never echoes it.
#
# Usage: scripts\set-apple-secrets.ps1 [-P12 path\to\DeveloperID.p12] [-Repo newdee/smep]
param(
    [string]$P12,
    [string]$Repo = "newdee/smep"
)
$ErrorActionPreference = "Stop"

if (-not $P12) { $P12 = Read-Host "Path to the Developer ID Application .p12" }
if (-not (Test-Path $P12)) { throw "no such file: $P12" }

function Set-HiddenSecret([string]$Name, [string]$Prompt) {
    $secure = Read-Host -AsSecureString $Prompt
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringUni(
        [Runtime.InteropServices.Marshal]::SecureStringToGlobalAllocUnicode($secure))
    if (-not $plain) { throw "$Name must not be empty" }
    $plain | gh secret set $Name --repo $Repo
}

[Convert]::ToBase64String([IO.File]::ReadAllBytes((Resolve-Path $P12))) |
    gh secret set APPLE_CERTIFICATE --repo $Repo

Set-HiddenSecret APPLE_CERTIFICATE_PASSWORD "Password of the .p12"
Set-HiddenSecret APPLE_SIGNING_IDENTITY   "Signing identity (Developer ID Application: Name (TEAMID))"
Set-HiddenSecret APPLE_ID                 "Apple ID (email)"
Set-HiddenSecret APPLE_PASSWORD           "App-specific password for notarization"
Set-HiddenSecret APPLE_TEAM_ID            "Team ID"

Write-Host "Set on ${Repo}:"
gh secret list --repo $Repo
Write-Host "Now: gh workflow run release.yml --repo $Repo -f tag=vX.Y.Z"
