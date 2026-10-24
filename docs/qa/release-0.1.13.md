# Release log — 0.1.13 (2026-10-24)

What is new since 0.1.12 (`phase16.md`): reading shows the document the way a PDF reader shows a page. The text sits in a 44 rem column on 800 px of paper, with a soft shadow, standing on the paper a shade darker. The leading is the blog's 1.85. The wheel scrolls the page from anywhere in the pane. The block view keeps the same column and leading.

Released without asking, as the user asked on 2026-10-24 ("以后做完都自动发包，不用等我确认").

- Commit `Release 0.1.13` (`8fbe70c`) on top of Phase 16 (`7486941`, `aef6632`). CI run 40058932013, all three jobs green: ubuntu 6 m 08 s, macos 7 m 25 s, windows 9 m 39 s. Tests: 127 + 4 (ubuntu), 125 + 4 (macos), 126 + 5 (windows), 0 failed.
- `cargo publish --locked` from `8fbe70c`: 25 files, 717.6 KiB (196.6 KiB compressed), `Published smep v0.1.13`.
- Tag `v0.1.13` pushed; release workflow run 40059689399, all four jobs green: windows 12 m 49 s, linux 13 m 10 s, macos 25 m 33 s, release 15 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.13-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 46,654,806 bytes |
| `smep-v0.1.13-macos-universal.tar.gz` | 46,493,279 bytes |
| `smep-v0.1.13-x86_64-linux.tar.gz` | 30,943,854 bytes |
| `smep-v0.1.13-x86_64-windows.zip` | 26,305,710 bytes |

## Homebrew tap

Run 40061477543 (65 s) bumped the formula and the cask, tested both and pushed `f5ecf71 smep 0.1.13`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```
