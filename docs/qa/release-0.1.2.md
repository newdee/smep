# Release log — 0.1.2 (2026-09-30)

What is new since 0.1.1: typing stays smooth in large documents (the Markdown parse runs off the keystroke), menus work in the rendered view without an active block, the macOS process ends with its window, `smep --version` / `--help`, the release is published (not drafted), and the Homebrew formula `newdee/tap/smep`. QA log: `phase4.md`.

- Version bump in the same commit as the work, `fab9e17`. CI run 36659257504, all three jobs green after a cold cache: ubuntu 15 m 06 s, windows 22 m 00 s, macos 13 m 39 s (63 tests each).
- `cargo publish --locked` from `fab9e17`: 16 files, 382.7 KiB (97.0 KiB compressed), `Published smep v0.1.2`. crates.io reports max_version 0.1.2.
- Tag `v0.1.2` at `fab9e17`, pushed; the release workflow started by the tag.

## Release workflow

Run 36660987704 (started by the tag; cold cache), all four jobs green: linux 8 m 37 s, windows 11 m 02 s, macos 10 m 18 s, release 8 s. Published, not a draft.

Re-run 36662824777 by hand (`gh workflow run release.yml -f tag=v0.1.2`, after `a01e59e` added the bare macOS binary to the packaging): green, linux 9 m 04 s, windows 13 m 34 s, macos 12 m 19 s; the files were re-uploaded over the release (`--clobber`).

| Artifact | Size |
|---|---|
| `smep-v0.1.2-macos-universal.zip` (the `.app`, unsigned) | 13,730,354 bytes |
| `smep-v0.1.2-macos-universal.tar.gz` (the bare universal binary, new) | 13,729,164 bytes |
| `smep-v0.1.2-x86_64-linux.tar.gz` | 14,348,562 bytes |
| `smep-v0.1.2-x86_64-windows.zip` | 8,532,971 bytes |

## Homebrew tap

`newdee/homebrew-tap` commits `1b80cdb` (first formula), `smep: a url only inside on_arm / on_intel…`, and the final `smep: the bare universal binary, macOS only`; the two failed attempts and their causes are in `phase4.md`. The bump script was run locally (Git Bash) against a formula pointed at `v0.0.0`: it moved both urls to v0.1.2 and wrote `d95055f3…37df8`, the same sha256 `Get-FileHash` gives for the file `gh release download` fetches; a second run reports "already at 0.1.2".

Tap run 36663895375 on the final commit (macOS runner, 48 s): `brew audit --formula --strict --online` clean; `brew install --formula newdee/tap/smep`; `brew test` passes (`smep --version` → `smep 0.1.2`); `lipo -archs` shows `arm64` and `x86_64`; uninstall leaves nothing behind.

```sh
brew install newdee/tap/smep
```
