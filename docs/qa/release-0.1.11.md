# Release log — 0.1.11 (2026-10-22)

What is new since 0.1.10: one design for the window, the HTML page and the PDF (`phase14.md`): a rule under h1 and h2, a framed table with a header row of ink on paper, framed code at 0.9 of the body; on the page and in print a washed quote and alternating rows; `smep --css` / `smep --typst` print the stylesheet and the Typst preamble, `export.css` / `export.typ` in the config directory replace them; a PDF prints in the light theme of the preview's family; a document's local images show in the preview; gpui-kit 0.8 (gpui-pre 0.3.9).

- Commit `Release 0.1.11` (`0198b1d`) on top of Phase 14 (`e1db949`, `4ffd9ab`). CI run 39739127995, all three jobs green: ubuntu 5 m 27 s, macos 6 m 12 s, windows 9 m 20 s (the runners' caches warm after the 0.8 upgrade's first run). Tests: 112 + 5 (windows), 112 + 4 (ubuntu), 110 + 4 (macos), 0 failed.
- `cargo publish --locked` from `0198b1d`: 24 files, 638.0 KiB (176.8 KiB compressed), `Published smep v0.1.11`.
- Tag `v0.1.11` pushed; release workflow run 39740336899, all four jobs green: linux 9 m 10 s, windows 13 m 19 s, macos 18 m 00 s, release 11 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.11-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 46,379,514 bytes |
| `smep-v0.1.11-macos-universal.tar.gz` (the bare universal binary) | 46,217,878 bytes |
| `smep-v0.1.11-x86_64-linux.tar.gz` | 30,808,842 bytes |
| `smep-v0.1.11-x86_64-windows.zip` | 26,129,257 bytes |

Against 0.1.10 the `.app` zip grew by 1,324,032 bytes, the Linux tarball by 1,033,893 and the Windows zip by 710,129: gpui-pre 0.3.3 → 0.3.9 and gpui-base 0.8.

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` right after the release: run 39742497640 (60 s) bumped the formula and the cask to 0.1.11, audited, installed, tested and uninstalled both, and pushed `dd8058b smep 0.1.11`.

```sh
brew upgrade --cask smep        # smep.app + the smep command
brew upgrade smep               # the command alone
cargo install smep
```

The chain ran unattended from `release-0.1.11.ps1` (the 0.1.10 script with the tag changed). The working tree was clean for the whole run.
