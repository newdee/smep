# Release log — 0.1.0 (2026-09-04)

- `cargo publish --locked` from commit `51b97a9`: 15 files, 316.2 KiB (82.1 KiB compressed), verification build passed, `Published smep v0.1.0`. crates.io reports max_version 0.1.0 (created 2026-09-04T06:16:17Z).
- Tag `v0.1.0` at `51b97a9`, pushed.
- Release workflow (`.github/workflows/release.yml`, added after the tag, run by hand with `-f tag=v0.1.0`): run 33843906316, all four jobs green.

| Job | Wall time | Artifact | Size |
|---|---|---|---|
| linux | 7 m 10 s | `smep-v0.1.0-x86_64-linux.tar.gz` | 14,170,600 bytes |
| windows | 12 m 38 s | `smep-v0.1.0-x86_64-windows.zip` | 8,321,405 bytes |
| macos (universal) | 15 m 07 s | `smep-v0.1.0-macos-universal.zip` | 13,377,261 bytes |
| release | 9 s | draft GitHub release `v0.1.0` with the three files | |

- The macOS app is **unsigned**: the certificate import, sign and notarize steps were skipped because the six Apple secrets are not set on this repository yet. The release stays a draft until that is done and the workflow re-run.
- First attempt of the workflow failed to parse: `secrets` is not allowed in a step-level `if`. Fixed by reading the presence checks into the job env (`98eded1`).
- Annotation: `actions/download-artifact@v4` targets Node 20; bump when a newer major is confirmed.
