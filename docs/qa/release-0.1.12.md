# Release log — 0.1.12 (2026-10-22)

What is new since 0.1.11 (`phase15.md`): the document is set the way dfine-blog sets an article (code on a board with a cinnabar tab and a Copy button, a cinnabar top edge on dark paper, tables ruled across; the page and the PDF add the h3 / h4 marks, the washed quote, the faded break, framed images); documents open in tabs, and a file opened while smep runs lands in a tab of the running window (one smep per user and config directory, `src/instance.rs`); an outline of the headings beside the document jumps to a section.

- Commit `Release 0.1.12` (`9317643`) on top of Phase 15 (`dd83a6f`, `54af7ef`). CI run 39811869501, all three jobs green: windows 9 m 26 s (123 + 5), macos 6 m 44 s (122 + 4), ubuntu 28 m 08 s (124 + 4), 0 failed.
- `cargo publish --locked` from `9317643`: 25 files, 711.2 KiB (195.2 KiB compressed), `Published smep v0.1.12`.
- Tag `v0.1.12` pushed; release workflow run 39816532204, all four jobs green: linux 8 m 49 s, windows 20 m 54 s, macos 27 m 20 s, release 23 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.12-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 46,654,722 bytes |
| `smep-v0.1.12-macos-universal.tar.gz` | 46,493,010 bytes |
| `smep-v0.1.12-x86_64-linux.tar.gz` | 30,942,009 bytes |
| `smep-v0.1.12-x86_64-windows.zip` | 26,302,446 bytes |

## Homebrew tap

The release script's session ended before its tap step, so it never dispatched `smep.yml`. The tap still reached 0.1.12 the same day: `2d686ee smep 0.1.12` (2026-10-22 17:39 UTC), from the tap's own scheduled bump. Formula and cask are both at 0.1.12.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```
