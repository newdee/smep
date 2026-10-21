# Release log — 0.1.8 (2026-10-21)

What is new since 0.1.7: the Mac install is the Homebrew cask (`phase11.md`): `brew install --cask newdee/tap/smep` puts `smep.app` in `/Applications` and links its binary as the `smep` command, so Finder, Spotlight, the Dock and "Open With" start smep without a Terminal window and it can claim `.md`. Only docs and the "Open Markdown Files with smep" message changed in the crate.

- Commit `Release 0.1.8` (`98364cd`) on top of the cask docs (`6e32829`) and the Phase 11 record (`82a7abd`). CI run 39606398441 for `98364cd`, all three jobs green: ubuntu 4 m 51 s, macos 6 m 49 s, windows 11 m 49 s. Tests: 102 + 4 (windows), 102 + 3 (ubuntu), 100 + 3 (macos), 0 failed.
- `cargo publish --locked` from `98364cd`: 21 files, 558.7 KiB (141.4 KiB compressed), `Published smep v0.1.8`.
- Tag `v0.1.8` pushed (`da726d0`); release workflow run 39608530674, all four jobs green: linux 11 m 02 s, windows 22 m 15 s, macos 33 m 14 s (signed, notarization skipped), release 10 s. Published, not a draft. (GitHub's job list kept showing the macos job as `in_progress` after the run had completed; every step of the job reports success and the release job that needs it ran at 14:03:26 UTC.)

| Artifact | Size |
|---|---|
| `smep-v0.1.8-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 44,894,741 bytes |
| `smep-v0.1.8-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,892,452 bytes |
| `smep-v0.1.8-x86_64-linux.tar.gz` | 29,746,400 bytes |
| `smep-v0.1.8-x86_64-windows.zip` | 25,392,355 bytes |

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` after the release: run 39613582386 (1 m 40 s) bumped the formula (`c6a77ea4…`, the tar.gz) and the cask (`a37ffa78…`, the zip) to 0.1.8, audited both, installed, tested and uninstalled both, and pushed `e9bcef1 smep 0.1.8` to the tap. The cask step warns, as expected until notarization, that `spctl` rejects the app ("Unnotarized Developer ID").

```sh
brew install --cask newdee/tap/smep   # smep.app + the smep command
brew install newdee/tap/smep          # the command alone
cargo install smep
```

## Script note

The release script picked the release run 30 s after the tag push; GitHub started the run for `v0.1.8` about two minutes after the push, so the script watched the finished 0.1.7 run, saw no 0.1.8 release and triggered the tap too early (run 39608344432, nothing to bump). `release-0.1.8.ps1` now polls for a run whose branch is the tag (up to five minutes, dispatching the workflow for the tag after three), and a resume script finished this chain: watch the run, then the tap. The working tree was clean until the tag was pushed; the README commit `ef56e39` (Gatekeeper steps for a downloaded `smep.app`) followed the tag.
