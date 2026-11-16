# Release log — 0.1.19 (2026-11-16)

What is new since 0.1.18 (`phase22.md`):

- Vim mode's `>` and `<`: shift whole lines by two spaces (as far as Tab indents), `>>` / `<<` with a count of lines, with any motion or object, and in Visual mode with a count of shifts; `.` repeats them.
- Search highlighting: every match of the last search is highlighted in the source editor (whole words only after `*` / `#`), kept up to date while typing, until `:noh`.

Released on the standing permission (2026-10-24).

- Commit `Release 0.1.19` (`f69bd6e`) on top of Phase 22 (`555e571`). CI run 43792069467, all three jobs green: ubuntu 4 m 50 s, macos 6 m 50 s, windows 7 m 9 s.
- `cargo publish --locked`: 33 files, 1.5 MiB (564.1 KiB compressed), `Published smep v0.1.19`. Cargo warned that `async-executor 1.14.0` in Cargo.lock is yanked upstream (a warning; the lockfile was left as it is).
- Tag `v0.1.19` pushed; release workflow run 43792879356, all four jobs green: linux 8 m 29 s, windows 13 m 44 s, macos 29 m 42 s, release 15 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.19-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 47,500,334 bytes |
| `smep-v0.1.19-macos-universal.tar.gz` | 47,338,880 bytes |
| `smep-v0.1.19-x86_64-linux.tar.gz` | 31,366,203 bytes |
| `smep-v0.1.19-x86_64-windows.zip` | 26,763,641 bytes |

## Homebrew tap

Run 43795858952 (83 s) bumped the formula and the cask, tested both and pushed `1744f18 smep 0.1.19`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```
