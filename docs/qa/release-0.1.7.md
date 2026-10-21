# Release log — 0.1.7 (2026-10-21)

What is new since 0.1.6: the review round in `phase10.md`: `.html` files render as pages when reading, the rendered view scrolls from the keyboard, the Windows "Open Markdown Files with smep" no longer flashes a console, `smep notes.md` opens a new document when the file is not there yet, and the rendered document has the right-click theme menu.

- Commit `Release 0.1.7` (`5871af7`) on top of the review (`592d5ee`) and its QA record (`adcc837`). CI run 39548431911 for `5871af7`, all three jobs green: ubuntu 5 m 07 s, macos 5 m 11 s, windows 6 m 44 s. Tests: 102 + 3 (ubuntu), 100 + 3 (macos), 102 + 4 (windows), 0 failed.
- `cargo publish --locked` from `5871af7`: 21 files, 557.9 KiB (141.1 KiB compressed), `Published smep v0.1.7`.
- Tag `v0.1.7` pushed; release workflow run 39549207396, all four jobs green: linux 9 m 43 s, windows 13 m 18 s, macos 33 m 25 s, release 10 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.7-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 44,895,242 bytes |
| `smep-v0.1.7-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,892,861 bytes |
| `smep-v0.1.7-x86_64-linux.tar.gz` | 29,745,436 bytes |
| `smep-v0.1.7-x86_64-windows.zip` | 25,392,201 bytes |

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` right after the release: run 39552524069 (61 s) bumped the formula, audited, installed, ran `brew test`, checked both architectures and pushed `28010f3 smep 0.1.7` to the tap.

```sh
brew install newdee/tap/smep   # or: brew upgrade smep
cargo install smep
```

The chain ran unattended from `release-0.1.7.ps1` (session scratchpad, the 0.1.6 script with the tag changed): wait for CI, publish, tag, wait for the release workflow, trigger and wait for the tap. The working tree was clean for the whole run.
