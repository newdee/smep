# Release log — 0.1.1 (2026-09-04)

What is new since 0.1.0: files opened from the OS (macOS `open_urls`, Linux desktop entry, Windows ProgId script), the frameless window with title-bar menus, the three view modes with full screen, and in-place editing in the rendered view. QA logs: `phase2e.md`, `phase3a.md`, `phase3b.md`.

- Version bump commit `111a9aa` ("Release 0.1.1": `Cargo.toml`, `Cargo.lock`, README status line). 59 tests pass locally; CI run 33878875667 — see below.
- `cargo publish --locked` from `111a9aa`: 16 files, 366.4 KiB (92.7 KiB compressed), verification build passed, `Published smep v0.1.1`. crates.io reports max_version 0.1.1.
- Tag `v0.1.1` at `111a9aa`, pushed; release workflow run 33878931004 started by the tag.

## CI for the release commit

Run 33878875667 for `111a9aa`, all three jobs green: ubuntu 1 m 48 s, windows 3 m 03 s, macos 1 m 41 s.

## Release workflow

Run 33878931004 (started by the tag), all four jobs green.

| Job | Wall time | Artifact | Size |
|---|---|---|---|
| linux | 8 m 31 s | `smep-v0.1.1-x86_64-linux.tar.gz` | 14,322,917 bytes |
| windows | 10 m 30 s | `smep-v0.1.1-x86_64-windows.zip` | 8,509,912 bytes |
| macos (universal) | 1 m 39 s (cache hit) | `smep-v0.1.1-macos-universal.zip` | 13,673,685 bytes |
| release | 7 s | draft GitHub release `v0.1.1` with the three files | |

- The macOS app is still **unsigned**: "Import the Developer ID certificate", "Sign" and "Notarize and staple" were skipped because the six Apple secrets are not set on the repository. The release stays a draft, like `v0.1.0`; set the secrets (`scripts/set-apple-secrets.*`) and re-run `gh workflow run release.yml -f tag=v0.1.1`, which re-uploads over the draft.
- Sizes are within 2 % of 0.1.0 (14,170,600 / 8,321,405 / 13,377,261).
