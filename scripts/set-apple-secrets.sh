#!/usr/bin/env bash
# Store the six Apple signing/notarization secrets on the smep repository.
# They are the same six magpie uses; GitHub cannot copy secrets between
# repositories, so this asks for each value once and never echoes it.
#
# Usage: scripts/set-apple-secrets.sh [path/to/DeveloperID.p12]
# Needs: gh (logged in), base64.
set -euo pipefail

repo="${SMEP_REPO:-newdee/smep}"
p12="${1:-}"

if [ -z "$p12" ]; then
  read -r -p "Path to the Developer ID Application .p12: " p12
fi
[ -f "$p12" ] || { echo "no such file: $p12" >&2; exit 1; }

ask_secret() { # name prompt
  local value
  read -r -s -p "$2: " value; echo
  [ -n "$value" ] || { echo "$1 must not be empty" >&2; exit 1; }
  printf '%s' "$value" | gh secret set "$1" --repo "$repo"
}

if base64 --help 2>&1 | grep -q -- '-w'; then
  base64 -w0 "$p12" | gh secret set APPLE_CERTIFICATE --repo "$repo"
else
  base64 -i "$p12" | tr -d '\n' | gh secret set APPLE_CERTIFICATE --repo "$repo"
fi

ask_secret APPLE_CERTIFICATE_PASSWORD "Password of the .p12"
ask_secret APPLE_SIGNING_IDENTITY   "Signing identity (Developer ID Application: Name (TEAMID))"
ask_secret APPLE_ID                 "Apple ID (email)"
ask_secret APPLE_PASSWORD           "App-specific password for notarization"
ask_secret APPLE_TEAM_ID            "Team ID"

echo "Set on $repo:"
gh secret list --repo "$repo"
echo "Now: gh workflow run release.yml --repo $repo -f tag=vX.Y.Z"
