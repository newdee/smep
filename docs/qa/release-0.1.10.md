# Release log — 0.1.10 (2026-10-22)

What is new since 0.1.9: the icon (`phase13.md`): a cinnabar seal with an M on paper, in the Windows executable (resource 1, embedded by `build.rs`), the macOS app (`smep.icns`, made by the release workflow) and the Linux desktop entry (`Icon=smep`, the PNG installed by "Open Markdown Files with smep" and shipped in the tarball as `smep.png`); the window carries `app_id` `smep`.

- Commit `Release 0.1.10` (`aae3fe2`) on top of the icon (`8187d09`). CI run 39705719764 for `aae3fe2`, all three jobs green: ubuntu 4 m 34 s, macos 5 m 29 s, windows 9 m 17 s. Tests: 103 + 4 (windows), 103 + 3 (ubuntu), 101 + 3 (macos), 0 failed.
- `cargo publish --locked` from `aae3fe2`: 24 files (`build.rs`, the `.ico` and the 256 px PNG are new), 605.9 KiB (170.5 KiB compressed), `Published smep v0.1.10`.
- Tag `v0.1.10` pushed; release workflow run 39706736931, all four jobs green: linux 13 m 24 s, windows 17 m 53 s, macos 30 m 04 s (the first run of the `iconutil` step: signed, notarization skipped), release 13 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.10-macos-universal.zip` (the `.app`, Developer ID signed, not notarized, with `smep.icns`) | 45,055,482 bytes |
| `smep-v0.1.10-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,893,852 bytes |
| `smep-v0.1.10-x86_64-linux.tar.gz` (`smep`, `smep.desktop`, `smep.png`) | 29,774,949 bytes |
| `smep-v0.1.10-x86_64-windows.zip` | 25,419,128 bytes |

The `.app` zip grew by 158,960 bytes over 0.1.9 and the Linux tarball by 27,214: the `.icns` and the PNG. Checked in the downloaded assets: `smep.app/Contents/Resources/smep.icns` is there (189,396 bytes) and `Info.plist` names it (`CFBundleIconFile` = `smep`); the tarball lists `smep`, `smep.desktop`, `smep.png`.

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` right after the release: run 39709691796 (68 s) bumped the formula and the cask to 0.1.10, audited, installed, tested and uninstalled both, and pushed `bbc591e smep 0.1.10`.

```sh
brew install --cask newdee/tap/smep   # smep.app + the smep command
brew install newdee/tap/smep          # the command alone
cargo install smep
```

The chain ran unattended from `release-0.1.10.ps1` (the 0.1.9 script with the tag changed; it waits for the release run of the tag itself). The working tree was clean for the whole run.
