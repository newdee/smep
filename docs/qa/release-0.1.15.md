# Release log — 0.1.15 (2026-11-14)

What is new since 0.1.14 (`phase18.md`):

- Book and Book Dark, the default (System follows the mode in Book): the document set as a book sets it, on screen, in the HTML page and in the PDF. Newsreader is built in (five static weights, about 600 KB); CJK text takes the system's Song face (Noto Serif SC, Source Han Serif, Songti SC, SimSun). 18 px at the default size, leading 1.78, a 38 em column (medium), semibold headings without rules or marks, code on a flat tint. Paper and the other themes keep the blog's design.
- A PDF no longer loses a `;` (or fails on `.x`, `(`, `[`) right after emphasis, a link, code, a footnote or an image.

Released without asking (standing permission, 2026-10-24).

- Commit `Release 0.1.15` (`f929706`) on top of Phase 18 (`4d8afff`). CI run 43561140361, all three jobs green: ubuntu 5 m 18 s, macos 5 m 57 s, windows 8 m 48 s.
- `cargo publish --locked` from `f929706`: 31 files (25 before, the five Newsreader faces and their licence added), 1.3 MiB (525.4 KiB compressed), `Published smep v0.1.15`.
- Tag `v0.1.15` pushed; release workflow run 43561940524, all four jobs green: linux 13 m 26 s, macos 18 m 33 s, windows 20 m 19 s, release 19 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.15-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 47,348,750 bytes |
| `smep-v0.1.15-macos-universal.tar.gz` | 47,187,438 bytes |
| `smep-v0.1.15-x86_64-linux.tar.gz` | 31,286,765 bytes |
| `smep-v0.1.15-x86_64-windows.zip` | 26,673,969 bytes |

Each artifact is about 0.4 MB larger than 0.1.14's: the bundled fonts.

## Homebrew tap

Run 43563674424 (61 s) bumped the formula and the cask, tested both and pushed `b853916 smep 0.1.15`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```

## A slip during the release

While the chain ran, a failed PowerShell edit meant for another worktree wrote an empty `src/outline.rs` into this tree (.NET resolved a relative path against the process directory, not the shell's). It appeared after `cargo publish` had packaged (31 files, none of them that one) and was deleted at once; the tag and the release build from the committed tree. From then on every .NET file call takes an absolute path.
