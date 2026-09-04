# smep — a Simple Markdown Editor & Previewer, written in Rust

[![crates.io](https://img.shields.io/crates/v/smep.svg)](https://crates.io/crates/smep)
[![CI](https://github.com/newdee/smep/actions/workflows/ci.yml/badge.svg)](https://github.com/newdee/smep/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**smep** is a markdown editor with live preview, built in pure Rust on the
[GPUI](https://www.gpui.rs) framework (the UI toolkit behind the Zed editor).
Two panes, no toolbar: write on the left, read on the right.

## Status

`0.1.0` is on its way; `main` already does everything below. The crates.io
release `0.0.1` is a placeholder that only prints its version.

## What it does

- Editor and rendered preview side by side, resizable, updated as you type.
- A menu of blocks (headings, lists, task list, quote, code block, table,
  divider, image, link) wherever you ask for it: type `/` on an empty line,
  click the `+` beside one, click below the end of the text, or right-click.
- Seven preview themes (System, GitHub, Newsprint, Night, Sepia, Solarized
  Light/Dark): right-click the preview. The choice is remembered in
  `settings.toml` under the platform config directory.
- Markdown highlighting in the editor from the same parser the preview uses.
- The preview follows the editor: whatever block is at the top of the editor
  is at the top of the preview.
- CommonMark + GFM: tables, task lists, strikethrough, footnotes. Raw HTML
  inside Markdown renders too, and `.html` files open straight into the
  HTML preview.
- Open, save, save as; an unsaved-changes prompt before closing.
- Light and dark, following the system.
- One static binary, no web view, no Electron.

## Keys

| Action | Windows / Linux | macOS |
|---|---|---|
| Open | Ctrl+O | Cmd+O |
| Save | Ctrl+S | Cmd+S |
| Save as | Ctrl+Shift+S | Cmd+Shift+S |
| Insert block (on an empty line) | `/` | `/` |
| Block menu / clipboard | right-click the editor | right-click the editor |
| Preview theme | right-click the preview | right-click the preview |
| Find in editor | Ctrl+F | Cmd+F |
| Find and replace | Ctrl+H | Cmd+Shift+F |
| Document start / end | Ctrl+Home / Ctrl+End | Cmd+Up / Cmd+Down |

## Install

```sh
cargo install smep
smep notes.md
```

Building from source needs a Rust toolchain and, on Linux, a few system
libraries; see [docs/dev.md](docs/dev.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you shall be dual licensed as above, without any
additional terms or conditions.
