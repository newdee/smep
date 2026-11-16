# Release log — 0.1.20 (2026-11-16)

What is new since 0.1.19 (`phase23.md`):

- Vim mode's `:s`: `:s/pattern/text/` on the cursor's line or a range (`%`, `N,M`, `.`, `$`, `'<,'>`, `+N` / `-N`), flags `g` `i` `I` `n` `e`, groups / `&` / case changes / `\r` in the replacement, one undo step; `:s` and `:&&` repeat it. `:` in Visual mode starts the line with `'<,'>`.
- `:set` for the run: `hlsearch`, `ignorecase`, `smartcase`, `wrapscan`, `shiftwidth` (also the editor's Tab), `number`, `wrap`; `:set opt?`, `:set`.
- Searches take Vim's patterns (magic, `\v`, `\V`, `\c` / `\C`), translated to the `regex` crate's syntax with CRLF-aware line anchors.

Released on the standing permission (2026-10-24).

- Commit `Release 0.1.20` (`ba6cc6b`) on top of Phase 23 (`e0afe0e`). CI run 43801180672, all three jobs green: ubuntu 5 m 36 s, macos 6 m 45 s, windows 7 m 43 s.
- `cargo publish --locked`: 35 files, 1.6 MiB (582.4 KiB compressed), `Published smep v0.1.20`.
- Tag `v0.1.20` pushed; release workflow run 43801997071, all four jobs green: linux 8 m 52 s, windows 20 m 19 s, macos 28 m 54 s, release 11 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.20-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 47,583,029 bytes |
| `smep-v0.1.20-macos-universal.tar.gz` | 47,421,589 bytes |
| `smep-v0.1.20-x86_64-linux.tar.gz` | 31,410,769 bytes |
| `smep-v0.1.20-x86_64-windows.zip` | 26,810,779 bytes |

## Homebrew tap

Run 43805041477 (95 s) bumped the formula and the cask, tested both and pushed `c7871fd smep 0.1.20`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```
