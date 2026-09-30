# Store the six Apple signing/notarization secrets on the smep repository.
# Same six as magpie; GitHub cannot copy secrets between repositories, so
# this takes each value from the local certificate folder where it can and
# asks (hidden input) for the rest. Nothing is echoed or written to disk.
#
# Usage: scripts\set-apple-secrets.ps1 [-CertsDir ~\.certs] [-P12 path.p12] [-Repo newdee/smep]
#
# From -CertsDir it reads, when present:
#   p12-b64.txt        -> APPLE_CERTIFICATE        (base64 of the .p12)
#   p12-password.txt   -> APPLE_CERTIFICATE_PASSWORD
#   */DeveloperID*.p12 -> APPLE_CERTIFICATE when p12-b64.txt is absent
#   any *.txt with lines like `APPLE_ID: value` or `APPLE_TEAM_ID=value`
#                      -> that secret
param(
    [string]$CertsDir = (Join-Path $HOME ".certs"),
    [string]$P12,
    [string]$Repo = "newdee/smep"
)
$ErrorActionPreference = "Stop"

$names = "APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD", "APPLE_SIGNING_IDENTITY",
         "APPLE_ID", "APPLE_PASSWORD", "APPLE_TEAM_ID"
$prompts = @{
    APPLE_CERTIFICATE_PASSWORD = "Password of the .p12"
    APPLE_SIGNING_IDENTITY     = "Signing identity (Developer ID Application: Name (TEAMID))"
    APPLE_ID                   = "Apple ID (email)"
    APPLE_PASSWORD             = "App-specific password for notarization"
    APPLE_TEAM_ID              = "Team ID"
}
$values = @{}

function Read-Trimmed([string]$Path) { (Get-Content -Raw -LiteralPath $Path).Trim() }

if (Test-Path $CertsDir) {
    $b64 = Join-Path $CertsDir "p12-b64.txt"
    $pw = Join-Path $CertsDir "p12-password.txt"
    if (Test-Path $b64) { $values.APPLE_CERTIFICATE = Read-Trimmed $b64; Write-Host "APPLE_CERTIFICATE: from p12-b64.txt" }
    if (Test-Path $pw) { $values.APPLE_CERTIFICATE_PASSWORD = Read-Trimmed $pw; Write-Host "APPLE_CERTIFICATE_PASSWORD: from p12-password.txt" }
    if (-not $P12) {
        $P12 = Get-ChildItem $CertsDir -Recurse -Filter "*.p12" -File | Select-Object -First 1 -ExpandProperty FullName
    }
    # Values written down next to the certificate, one per line, `NAME: value`.
    foreach ($file in Get-ChildItem $CertsDir -Recurse -Filter "*.txt" -File) {
        foreach ($line in Get-Content -LiteralPath $file.FullName) {
            if ($line -match '^\s*(APPLE_[A-Z_]+)\s*[:=]\s*(.+?)\s*$' -and $names -contains $matches[1] -and -not $values[$matches[1]]) {
                $values[$matches[1]] = $matches[2]
                Write-Host "$($matches[1]): from $($file.Name)"
            }
        }
    }
}

if (-not $values.APPLE_CERTIFICATE) {
    if (-not $P12) { $P12 = Read-Host "Path to the Developer ID Application .p12" }
    if (-not (Test-Path $P12)) { throw "no such file: $P12" }
    $values.APPLE_CERTIFICATE = [Convert]::ToBase64String([IO.File]::ReadAllBytes((Resolve-Path $P12)))
    Write-Host "APPLE_CERTIFICATE: from $P12"
}

foreach ($name in $names) {
    if ($values[$name]) { continue }
    $secure = Read-Host -AsSecureString $prompts[$name]
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringUni(
        [Runtime.InteropServices.Marshal]::SecureStringToGlobalAllocUnicode($secure))
    if (-not $plain) { throw "$name must not be empty" }
    $values[$name] = $plain
}

foreach ($name in $names) {
    $values[$name] | gh secret set $name --repo $Repo
    if ($LASTEXITCODE -ne 0) { throw "gh secret set $name failed" }
}

Write-Host "Set on ${Repo}:"
gh secret list --repo $Repo
Write-Host "Now: gh workflow run release.yml --repo $Repo -f tag=vX.Y.Z"
