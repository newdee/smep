# Release log — 0.1.14 (2026-10-24)

What is new since 0.1.13 (`phase17.md`):

- The paper fills the pane. The text runs in a centred column, and its width is a setting: View, Text Width, narrow / medium / wide / full, medium by default.
- The scrollbar is at the window's edge.
- The outline shows from the start and folds away to a strip. The setting's new key is `outline_folded`; the old `outline` key that 0.1.12 / 0.1.13 wrote is ignored.

Released without asking (standing permission, 2026-10-24).

- Commit `Release 0.1.14` (`b1bdfc3`) on top of Phase 17 (`42ee394`). CI run 40064761496, all three jobs green: windows 6 m 25 s, macos 6 m 33 s, ubuntu 6 m 57 s. Tests: 125 + 5 (windows), 126 + 4 (ubuntu), 124 + 4 (macos), 0 failed.
- `cargo publish --locked` from `b1bdfc3`: 25 files, 726.3 KiB (198.4 KiB compressed), `Published smep v0.1.14`.
- Tag `v0.1.14` pushed; release workflow run 40065288677, all four jobs green: linux 12 m 16 s, windows 13 m 43 s, macos 19 m 30 s, release 13 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.14-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 46,664,989 bytes |
| `smep-v0.1.14-macos-universal.tar.gz` | 46,503,475 bytes |
| `smep-v0.1.14-x86_64-linux.tar.gz` | 30,950,690 bytes |
| `smep-v0.1.14-x86_64-windows.zip` | 26,312,395 bytes |

## Homebrew tap

Run 40066621450 (64 s) bumped the formula and the cask, tested both and pushed `0c00103 smep 0.1.14`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```
