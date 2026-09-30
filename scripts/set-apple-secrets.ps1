# Store the six Apple signing/notarization secrets on the smep repository.
# Same six as magpie; GitHub cannot copy secrets between repositories, so
# this takes each value from the local certificate folder where it can and
# asks (hidden input) for the rest. Nothing is echoed or written to disk.
#
# Usage: scripts\set-apple-secrets.ps1 [-CertsDir ~\.certs] [-P12 path.p12] [-Repo newdee/smep] [-NoPrompt]
#
# From -CertsDir it takes, when present:
#   p12-b64.txt          -> APPLE_CERTIFICATE (base64 of the .p12)
#   p12-password.txt     -> APPLE_CERTIFICATE_PASSWORD
#   */*.p12              -> APPLE_CERTIFICATE when p12-b64.txt is absent, and
#                           APPLE_SIGNING_IDENTITY + APPLE_TEAM_ID read out of
#                           the certificate itself (its subject is the identity)
#   any *.txt            -> lines like `APPLE_ID: value`, `Apple ID = value`,
#                           `app-specific password: value`, `Team ID: value`
# -NoPrompt sets what it found and lists what is still missing; without it
# the missing values are asked for one by one.
param(
    [string]$CertsDir = (Join-Path $HOME ".certs"),
    [string]$P12,
    [string]$Repo = "newdee/smep",
    [switch]$NoPrompt
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
# Labels people write in a notes file, by secret. Case-insensitive.
$labels = @{
    APPLE_ID       = 'APPLE_ID|apple\s*id|apple\s*账号|apple\s*帐号'
    APPLE_PASSWORD = 'APPLE_PASSWORD|app[-\s]*specific[-\s]*password|app\s*password|应用专用密码|专用密码'
    APPLE_TEAM_ID  = 'APPLE_TEAM_ID|team\s*id|团队\s*id'
    APPLE_SIGNING_IDENTITY = 'APPLE_SIGNING_IDENTITY|signing\s*identity|identity|签名身份'
}
$values = @{}
$sources = @{}

function Read-Trimmed([string]$Path) { (Get-Content -Raw -LiteralPath $Path).Trim() }
function Found([string]$Name, [string]$Value, [string]$Source) {
    if ($Value -and -not $values[$Name]) { $script:values[$Name] = $Value; $script:sources[$Name] = $Source }
}

if (Test-Path $CertsDir) {
    $b64 = Join-Path $CertsDir "p12-b64.txt"
    $pw = Join-Path $CertsDir "p12-password.txt"
    if (Test-Path $b64) { Found APPLE_CERTIFICATE (Read-Trimmed $b64) "p12-b64.txt" }
    if (Test-Path $pw) { Found APPLE_CERTIFICATE_PASSWORD (Read-Trimmed $pw) "p12-password.txt" }
    if (-not $P12) {
        $P12 = Get-ChildItem $CertsDir -Recurse -Filter "*.p12" -File | Select-Object -First 1 -ExpandProperty FullName
    }
    foreach ($file in Get-ChildItem $CertsDir -Recurse -Filter "*.txt" -File) {
        if ($file.Name -in "p12-b64.txt", "p12-password.txt") { continue }
        foreach ($line in Get-Content -LiteralPath $file.FullName) {
            foreach ($name in $labels.Keys) {
                if ($line -match ('^\s*(?:' + $labels[$name] + ')\s*[:=：]\s*(.+?)\s*$')) {
                    Found $name $matches[1] $file.Name
                }
            }
        }
    }
}

if (-not $values.APPLE_CERTIFICATE) {
    if (-not $P12 -and -not $NoPrompt) { $P12 = Read-Host "Path to the Developer ID Application .p12" }
    if ($P12 -and (Test-Path $P12)) {
        Found APPLE_CERTIFICATE ([Convert]::ToBase64String([IO.File]::ReadAllBytes((Resolve-Path $P12)))) (Split-Path $P12 -Leaf)
    }
}

# The certificate's subject is the signing identity, and the team id sits
# in its parentheses (and in the OU); no need to type either.
if ($P12 -and (Test-Path $P12) -and $values.APPLE_CERTIFICATE_PASSWORD -and
    (-not $values.APPLE_SIGNING_IDENTITY -or -not $values.APPLE_TEAM_ID)) {
    try {
        $cert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new(
            (Resolve-Path $P12).Path, $values.APPLE_CERTIFICATE_PASSWORD)
        $cn = $cert.GetNameInfo([System.Security.Cryptography.X509Certificates.X509NameType]::SimpleName, $false)
        if ($cn -match '^Developer ID Application: .+ \(([A-Z0-9]{10})\)$') {
            Found APPLE_SIGNING_IDENTITY $cn "the certificate's subject"
            Found APPLE_TEAM_ID $matches[1] "the certificate's subject"
        } elseif ($cn) {
            Write-Warning "the certificate's subject is not a Developer ID Application identity: $cn"
        }
    } catch {
        Write-Warning "could not open the .p12 with the password from p12-password.txt: $($_.Exception.Message)"
    }
}

foreach ($name in $names) {
    if ($values[$name]) { Write-Host "$name`: from $($sources[$name])"; continue }
    if ($NoPrompt) { Write-Host "$name`: MISSING (not set)"; continue }
    $secure = Read-Host -AsSecureString $prompts[$name]
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringUni(
        [Runtime.InteropServices.Marshal]::SecureStringToGlobalAllocUnicode($secure))
    if (-not $plain) { throw "$name must not be empty" }
    $values[$name] = $plain
}

foreach ($name in $names) {
    if (-not $values[$name]) { continue }
    $values[$name] | gh secret set $name --repo $Repo
    if ($LASTEXITCODE -ne 0) { throw "gh secret set $name failed" }
}

Write-Host "Set on ${Repo}:"
gh secret list --repo $Repo
$missing = $names | Where-Object { -not $values[$_] }
if ($missing) {
    Write-Host "Still missing: $($missing -join ', '). Signing needs the first three; notarization needs all six."
    Write-Host "Run again without -NoPrompt to type them."
} else {
    Write-Host "Now: gh workflow run release.yml --repo $Repo -f tag=vX.Y.Z"
}
