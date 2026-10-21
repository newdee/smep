# Release log — 0.1.6 (2026-10-21)

What is new since 0.1.5: smep opens reading; Ctrl+E / Cmd+E, the Edit button in the title bar, "Editing" in the View menu or a view-mode key bring the editor (`phase9.md`).

- Commit `Release 0.1.6` (`307b4e5`) on top of the feature (`0f45706`) and its QA record (`3c215ec`). CI run 39520578219 for `307b4e5`, all three jobs green: ubuntu 4 m 57 s, macos 5 m 29 s, windows 10 m 01 s. Tests: 98 + 3 (ubuntu), 96 + 3 (macos), 98 + 4 (windows), 0 failed.
- `cargo publish --locked` from `307b4e5`: 21 files, 543.6 KiB (138.1 KiB compressed), `Published smep v0.1.6`.
- Tag `v0.1.6` pushed; release workflow run 39521736005, all four jobs green: linux 12 m 05 s, windows 17 m 04 s, macos 34 m 22 s, release 11 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.6-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 44,886,859 bytes |
| `smep-v0.1.6-macos-universal.tar.gz` (the bare universal binary, signed on its own) | 44,884,690 bytes |
| `smep-v0.1.6-x86_64-linux.tar.gz` | 29,742,417 bytes |
| `smep-v0.1.6-x86_64-windows.zip` | 25,389,730 bytes |

## Homebrew tap

`gh workflow run smep.yml --repo newdee/homebrew-tap` right after the release: run 39525347621 (57 s) bumped the formula, audited, installed, ran `brew test`, checked both architectures and pushed `813c51f smep 0.1.6` to the tap.

```sh
brew install newdee/tap/smep   # or: brew upgrade smep
cargo install smep
```

The chain ran unattended from `release-0.1.6.ps1` (session scratchpad, a copy of the 0.1.5 script with the tag changed): wait for CI, publish, tag, wait for the release workflow, trigger and wait for the tap. The working tree was clean for the whole run.
