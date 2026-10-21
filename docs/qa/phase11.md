# Phase 11 — smep.app through Homebrew (2026-10-21)

Report: "smep in macos also start with terminal".

## Diagnosis

- **Common, not individual.** Every way of getting smep on a Mac except the release's `.app` zip installs a bare binary: `cargo install`, the Homebrew formula, the `.tar.gz`. A bare Mach-O is a command-line tool to macOS: Finder, Spotlight, the Dock and "Open With" start it through Terminal, which is the window the report describes. Windows had the same class of problem (console subsystem, Phase 8); Linux is covered by `smep.desktop` (`Terminal=false`).
- **Structural, not a boundary.** The binary cannot change what launches it; only an application bundle is launched directly, and only a bundle can claim `.md` (`defaults.rs` already says so). The fix is in distribution: the tap gets a cask that installs the release's `smep.app` and links its binary as the `smep` command, so one install covers both uses. The formula stays for people who want the command alone and for existing installs (`brew upgrade smep` keeps working; Homebrew treats a bare `brew install newdee/tap/smep` as the formula when both exist).
- **Risk of new bugs.** Formula and cask both own `bin/smep`: Homebrew refuses the second install ("already a Binary at ..."); a cask cannot name a formula in `conflicts_with` (the audit rejects `formula:`), so the refusal is the guard. The cask's app is quarantined by Homebrew and not yet notarized, so Gatekeeper refuses the first start from Finder once (the cask's caveats, the README and the tap README say how; `--no-quarantine` skips it). The tap's bump now has to move two files to one version: the workflow checks both agree before it tests anything.

No smep code path changed; the only source change is the "Open Markdown Files with smep" message, which now names the cask.

## Changes

- Tap (`newdee/homebrew-tap`): `Casks/smep.rb` (universal `.app` zip, `app` + `binary`, zap of the settings dir, caveats); `scripts/bump-smep.sh` bumps the formula and the cask to the same release; `.github/workflows/smep.yml` audits, installs, tests and uninstalls both on a Mac; all three tap workflows rebase before publishing a bump (a concurrent push had rejected the keepane 0.43.1 bump) and use `actions/checkout@v5`.
- smep: README (Install, default-app section), `docs/dev.md`, `src/defaults.rs` doc and message.

## Rounds

Each round = review from one perspective + the tap workflow on a real macOS runner (audit, install, `smep --version`, `lipo`, uninstall) and, for the smep side, the full test suite.

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A | mechanism: first run of the cask on a Mac (run 39603627360) | audit failed 49 s in | `conflicts_with formula:` is not a cask key ("Valid keys are: :cask") → removed |
| B | same, rerun (39603861035, 63 s) | green: app moved to `/Applications`, binary linked, `--version` = 0.1.7, both archs, zap clean | `depends_on macos: ">= :monterey"` deprecated → `:monterey` |
| C | static consistency over the workflow and docs (39604180841, 68 s) | green, deprecation gone | unread `version=` step output; `checkout@v4` Node 20 warning in all three workflows → both fixed |
| D | concurrency: the external keepane bump that ran alongside (39604326052, 39604772862) | its publish was rejected twice, non-fast-forward against my pushes | `git pull --rebase` before the publish push in all three workflows |
| 1 (clean) | the push after D: smep 39604871025 (77 s), cask 39604870997 (70 s), formula 39604871090 (60/61 s); dispatched formula 39604927460 published `4a7c912 keepane 0.43.1` through the new rebase step | all green | none |
| 2 (clean) | bump script edge cases locally (Git Bash, `shasum` shim over `sha256sum`): cask at 0.1.6 with formula at 0.1.7, then formula at 0.1.6 with cask at 0.1.7 | each path downloads only its own archive and restores the committed file byte for byte (`git diff` empty); both current → "already at 0.1.7", `changed=false` | none |
| 3 (clean) | no-change path on the runner: `workflow_dispatch` of smep.yml (run 39605333005, 8 s) | bump prints "formula and cask already at 0.1.7", `run=false`, every later step skipped | none |

smep side: `cargo test` locally 102 + 4 passed, 0 failed; CI run 39604094318 for `6e32829` green: ubuntu 5 m 35 s (102 + 3), windows 8 m 16 s (102 + 4), macos 5 m 25 s (100 + 3), 0 failed; the macOS job compiles the changed `defaults.rs` message.

## Left

- Notarization still needs `APPLE_ID` + `APPLE_PASSWORD` on newdee/smep; until then the first start of the cask's app needs Open Anyway, and the tap workflow only warns on `spctl` ("rejected, source=Unnotarized Developer ID"). Once it passes, the warning branch no longer runs and nothing else changes.
- Something outside this session dispatches `formula.yml` on the tap (two runs at 13:04 UTC that I did not start, probably keepane's release workflow); harmless now that publishing rebases.
- `smep notes.md` from a terminal still holds that terminal until the window closes, as on Linux; `open -a smep notes.md` returns at once with the app installed.
