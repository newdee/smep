# Phase 13 — an icon (2026-10-22)

Request: "can we add a logo for this app".

## What it is

A cinnabar seal with an M on the Paper look's paper: `assets/icon/logo.svg` is the design, `assets/icon/render.py` draws the same geometry with Pillow at 16, 32, 48, 64, 128, 256, 512 and 1024 px (supersampled 4×, Lanczos) and packs `smep.ico` (16 to 256). No font is involved: the M is one stroke, so the PNGs match the SVG and render the same on every machine.

Where each platform takes it (one design, three carriers, nothing drawn at run time):

- Windows: `build.rs` embeds `smep.ico` as resource 1 through `winresource`; gpui's Windows platform loads icon 1 for the window (`LoadImageW(module, 1, …)`), Explorer and the taskbar read the same resource. Without the SDK's `rc.exe` the build prints a warning and goes on without the icon, so `cargo install` never fails over it.
- macOS: the release workflow builds `smep.icns` with `iconutil` from the PNGs and names it in `Info.plist` (`CFBundleIconFile`).
- Linux: the desktop entry says `Icon=smep`; "Open Markdown Files with smep" writes the 256 px PNG into `~/.local/share/icons/hicolor/256x256/apps/`, the release tarball carries it as `smep.png`, and the window carries `app_id` `smep` so the desktop matches it to the entry.

Not done: gpui's X11-only `WindowOptions::icon` (an `RgbaImage`), which would show the icon on X11 without a desktop entry; gpui does not re-export `image`, and the entry covers the case.

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| 1 | mechanism, the real exe: `ExtractAssociatedIcon` on `target/debug/smep.exe` gives the seal (32 px); the taskbar shows it with the window open; `cargo fmt --check`, clippy `--all-targets`, full suite 103 + 4 (the Linux desktop-entry test runs on CI only) | clean | none |
| 2 | packaging: `cargo package --list` carries `build.rs`, `assets/icon/smep.ico` and `assets/icon/smep-256.png`; `cargo package` builds the crate from the archive: 24 files (21 before), 605.8 KiB, verified build 14 s, `build.rs` ran from the package root and found the `.ico` | clean | none |
| 3 | CI run __CI__ (ubuntu runs the desktop-entry test that checks the written PNG) | | |

The macOS `iconutil` step runs only in the release workflow; the iconset names in it (`icon_NxN.png`, `icon_NxN@2x.png` for N = 16, 32, 128, 256, 512) all have a PNG, checked against `assets/icon` by hand. The first release carrying it is the check.
