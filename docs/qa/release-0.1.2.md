# Release log — 0.1.2 (2026-09-30)

What is new since 0.1.1: typing stays smooth in large documents (the Markdown parse runs off the keystroke), menus work in the rendered view without an active block, the macOS process ends with its window, `smep --version` / `--help`, the release is published (not drafted), and the Homebrew formula `newdee/tap/smep`. QA log: `phase4.md`.

- Version bump in the same commit as the work, `fab9e17`. CI run 36659257504, all three jobs green after a cold cache: ubuntu 15 m 06 s, windows 22 m 00 s, macos 13 m 39 s (63 tests each).
- `cargo publish --locked` from `fab9e17`: 16 files, 382.7 KiB (97.0 KiB compressed), `Published smep v0.1.2`. crates.io reports max_version 0.1.2.
- Tag `v0.1.2` at `fab9e17`, pushed; the release workflow started by the tag.

## Release workflow

Pending.

## Homebrew tap

Pending.
