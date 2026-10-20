# Release log — 0.1.4 (2026-10-20)

What is new since 0.1.3: the Paper look (keepane's `paper` / `paper-dark`, light or dark with the system; `phase6.md`) and "Open Markdown Files with smep" in the File menu (`phase7.md`).

- Version bump `Release 0.1.4`, then the macOS build fix `9ca565e` on top (the tag points at `9ca565e`). CI run 39427813145 for `9ca565e`, all three jobs green: ubuntu 4 m 46 s, macos 24 m 35 s, windows 36 m 51 s; 97 tests each.
- `cargo publish --locked` from `9ca565e`: 20 files, 531.4 KiB (134.9 KiB compressed), `Published smep v0.1.4`.
- Tag `v0.1.4` pushed; release workflow run 39433335622, all four jobs green: linux 13 m 06 s, windows 21 m 16 s, macos 27 m 45 s, release 15 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.4-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 44,883,951 bytes |
| `smep-v0.1.4-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,881,628 bytes |
| `smep-v0.1.4-x86_64-linux.tar.gz` | 29,737,339 bytes |
| `smep-v0.1.4-x86_64-windows.zip` | 25,381,534 bytes |

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` right after the release: run 39437485810 (macOS runner, 77 s) bumped the formula to 0.1.4, audited it, installed it, ran `brew test` (`smep --version` → `smep 0.1.4`), checked both architectures, uninstalled, and pushed `02fc25c smep 0.1.4` to the tap.

```sh
brew install newdee/tap/smep   # or: brew upgrade smep
cargo install smep
```

The whole chain ran unattended from one script (`release-0.1.4.ps1` in the session scratchpad): wait for CI, publish, tag, wait for the release workflow, trigger and wait for the tap.
