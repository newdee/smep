# Release log — 0.1.18 (2026-11-16)

What is new since 0.1.17 (`phase21.md`):

- Vim mode's command line: `:`, `/` and `?` open a single-line input at the foot of the pane (an input method and paste work in it). `:w`, `:q` (refused while there are unsaved changes), `:q!`, `:wq`, `:x`, `:N`, `:$`; `:q` on the last tab closes the window.
- Searches: literal, case-sensitive, wrapping round the document; `n`, `N`, `*`, `#`; as motions after an operator (`d/foo`) and in Visual mode. `/` no longer opens the find bar (Ctrl+F still does).

Released on the user's request and the standing permission (2026-10-24).

- Commit `Release 0.1.18` (`6417013`) on top of Phase 21 (`7d56ba7`). CI run 43748829401, all three jobs green: ubuntu 5 m 43 s, windows 7 m 57 s, macos 8 m 19 s.
- `cargo publish --locked` failed once at the upload: `[35] SSL connect error (schannel: failed to receive handshake, SSL/TLS connection failed)`. crates.io's API showed 0.1.17 as the latest and no tag had been made, so the chain was run again unchanged: 33 files, 1.5 MiB (559.3 KiB compressed), `Published smep v0.1.18`.
- Tag `v0.1.18` pushed; release workflow run 43749811426, all four jobs green: linux 12 m 44 s, windows 15 m 4 s, macos 30 m 15 s, release 15 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.18-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 47,469,270 bytes |
| `smep-v0.1.18-macos-universal.tar.gz` | 47,307,888 bytes |
| `smep-v0.1.18-x86_64-linux.tar.gz` | 31,356,099 bytes |
| `smep-v0.1.18-x86_64-windows.zip` | 26,751,667 bytes |

## Homebrew tap

Run 43752782267 (77 s) bumped the formula and the cask, tested both and pushed `1f4b0fa smep 0.1.18`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```
