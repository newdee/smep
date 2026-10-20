# Release log — 0.1.5 (2026-10-20)

What is new since 0.1.4: `smep.exe` is a GUI program, so Windows opens it without a terminal window (`phase8.md`).

- Commit `Release 0.1.5` (`353ae87`) on top of the fix (`5684a0d`). CI run 39453565196 for `353ae87`, all three jobs green: ubuntu 5 m 56 s, macos 7 m 49 s, windows 16 m 46 s. Tests: 97 + 3 (ubuntu), 95 + 3 (macos), 97 + 4 (windows; the PE subsystem test is Windows-only).
- `cargo publish --locked` from `353ae87`: 21 files, 536.0 KiB (136.7 KiB compressed), `Published smep v0.1.5` (the tests directory is in the package now).
- Tag `v0.1.5` pushed; release workflow run 39456438741, all four jobs green: linux 9 m 31 s, windows 22 m 10 s, macos 31 m 01 s, release 14 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.5-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 44,883,991 bytes |
| `smep-v0.1.5-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,881,649 bytes |
| `smep-v0.1.5-x86_64-linux.tar.gz` | 29,737,492 bytes |
| `smep-v0.1.5-x86_64-windows.zip` | 25,386,489 bytes |

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` right after the release: run 39461094972 (46 s) bumped the formula, audited, installed, ran `brew test`, checked both architectures and pushed `6890001 smep 0.1.5` to the tap.

```sh
brew install newdee/tap/smep   # or: brew upgrade smep
cargo install smep
```

The chain ran unattended from `release-0.1.5.ps1` (session scratchpad): wait for CI, publish, tag, wait for the release workflow, trigger and wait for the tap. The working tree was kept clean during the run (work in progress stashed), since `cargo publish` refuses a dirty tree and `git tag` takes the current HEAD.
