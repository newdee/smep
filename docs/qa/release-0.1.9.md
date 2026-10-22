# Release log — 0.1.9 (2026-10-22)

What is new since 0.1.8 (`phase12.md`): the Paper look is dfine-blog's (its paper, inks and cinnabar in the application theme and the Paper / Paper Dark palettes), every preview theme is typeset the blog's way (heading scale, paragraph gap, framed code, tables ruled across), reading mode shows the document as one page (the split view's preview: footnotes and selections work across blocks, one parse, one scroll position), an empty document being read shows a hint, and the title bar's toggle is Write / Read.

- Commit `Release 0.1.9` (`5028d71`) on top of the round (`3bf7334`, `f2134aa`). CI run 39634855081 for `5028d71`, all three jobs green: ubuntu 4 m 16 s, windows 6 m 16 s, macos 8 m 00 s (after a 5 min queue). Tests: 103 + 4 (windows), 103 + 3 (ubuntu), 101 + 3 (macos), 0 failed.
- `cargo publish --locked` from `5028d71`: 21 files, 568.2 KiB (144.0 KiB compressed), `Published smep v0.1.9`.
- Tag `v0.1.9` pushed; release workflow run 39637014518 (found by its branch, the new script step), all four jobs green: linux 12 m 09 s, windows 20 m 02 s, macos 35 m 30 s (signed, notarization skipped), release 15 s. Published, not a draft, at 17:17 UTC.

| Artifact | Size |
|---|---|
| `smep-v0.1.9-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 44,896,522 bytes |
| `smep-v0.1.9-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,894,317 bytes |
| `smep-v0.1.9-x86_64-linux.tar.gz` | 29,747,735 bytes |
| `smep-v0.1.9-x86_64-windows.zip` | 25,393,705 bytes |

## Homebrew tap

The dispatch from the chain was lost to the network blip and the resume was cut short when the session ended; the tap's own schedule took the release: run 39645323240 (17:36 UTC, 19 minutes after the release) bumped the formula and the cask to 0.1.9, tested both, and pushed `0999bea smep 0.1.9`. The next scheduled run (39682822459) found nothing to do.

```sh
brew install --cask newdee/tap/smep   # smep.app + the smep command
brew install newdee/tap/smep          # the command alone
cargo install smep
```

## Script note

The chain reached the release run, then every GitHub API call failed for a minute ("net/http: TLS handshake timeout", a network blip here): the watch, the release view and the tap dispatch. The resume script (`release-0.1.8-resume.ps1 -Id 39637014518 -Tag v0.1.9`) took over once the API answered again. The working tree was clean until the tag was pushed.
