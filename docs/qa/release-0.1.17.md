# Release log — 0.1.17 (2026-11-16)

What is new since 0.1.16 (`phase20.md`):

- Vim mode (Edit, Vim Mode; off by default, saved as `vim_mode`): the source editor and the rendered view's block editors take Vim's keys, with Normal, Insert, Visual and Visual Line modes, counts, `d c y` with motions and text objects, `p P r J ~ u Ctrl+R .`, a block cursor in Normal mode and the mode in the pane's corner. CRLF documents keep their line breaks.

Released on the user's request ("做完了之后再发版") and the standing permission (2026-10-24).

- Commit `Release 0.1.17` (`9f1c22a`) on top of Phase 20 (`5a92a00`). CI run 43739028124, all three jobs green: ubuntu 4 m 58 s, macos 6 m 39 s, windows 7 m 25 s.
- `cargo publish --locked` from `9f1c22a`: 33 files (`src/vim.rs` added), 1.5 MiB (552.7 KiB compressed), `Published smep v0.1.17`.
- Tag `v0.1.17` pushed; release workflow run 43739829132, all four jobs green: linux 12 m 37 s, windows 20 m 13 s, macos 28 m 53 s, release 15 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.17-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 47,446,121 bytes |
| `smep-v0.1.17-macos-universal.tar.gz` | 47,284,458 bytes |
| `smep-v0.1.17-x86_64-linux.tar.gz` | 31,337,147 bytes |
| `smep-v0.1.17-x86_64-windows.zip` | 26,732,542 bytes |

## Homebrew tap

Run 43742631492 (98 s) bumped the formula and the cask, tested both and pushed `2432a01 smep 0.1.17`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```

## The release script

Every wait in the chain (CI, the release run, the tap run) now polls the run's `status` / `conclusion` instead of trusting `gh run watch`'s exit code, which stopped the 0.1.16 chain early. This chain ran end to end without a hand step.
