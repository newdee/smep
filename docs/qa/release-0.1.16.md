# Release log — 0.1.16 (2026-11-14)

What is new since 0.1.15 (`phase19.md`):

- The outline folds a section: the triangle (▾ / ▸) before a heading that holds others folds them away, without going there. Folds belong to the tab and stay with their section while the text above changes.
- The outline lists headings down to a depth: 1, 2, 3 or All in its head, saved as `outline_depth`. A section folded away or past the depth marks the one being read in its place.

Released without asking (standing permission, 2026-10-24).

- Commit `Release 0.1.16` (`fc65c49`) on top of Phase 19 (`5bb4a6f`). CI run 43565740805, all three jobs green: macos 4 m 34 s, ubuntu 5 m 17 s, windows 9 m 21 s.
- `cargo publish --locked` from `fc65c49`: 32 files (`src/outline.rs` added), 1.3 MiB (529.5 KiB compressed), `Published smep v0.1.16`.
- Tag `v0.1.16` pushed; release workflow run 43566603634, all four jobs green: linux 10 m 11 s, windows 15 m 28 s, macos 36 m 24 s (twice its usual time), release 14 s. Published, not a draft.

| Artifact | Size |
|---|---|
| `smep-v0.1.16-macos-universal.zip` (the `.app`, Developer ID signed, not notarized) | 47,363,065 bytes |
| `smep-v0.1.16-macos-universal.tar.gz` | 47,201,487 bytes |
| `smep-v0.1.16-x86_64-linux.tar.gz` | 31,298,676 bytes |
| `smep-v0.1.16-x86_64-windows.zip` | 26,688,259 bytes |

## Homebrew tap

Run 43569550303 (71 s) bumped the formula and the cask, tested both and pushed `68bbb05 smep 0.1.16`.

```sh
brew upgrade --cask smep
brew upgrade smep
cargo install smep
```

## The release script stopped early

`gh run watch` on the release run exited 1 at 06:45, when the Windows job finished and the macOS one was still building (it finished green at 07:06). The script took that as a failed release and stopped before the tap. The run was then followed by polling its status, and the tap bumped by hand. The script now polls the run's `status` / `conclusion` instead of trusting `gh run watch`'s exit code.
