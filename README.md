# smep — a Simple Markdown Editor & Previewer, written in Rust

[![crates.io](https://img.shields.io/crates/v/smep.svg)](https://crates.io/crates/smep)
[![CI](https://github.com/newdee/smep/actions/workflows/ci.yml/badge.svg)](https://github.com/newdee/smep/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**smep** is a markdown editor with live preview, built in pure Rust on the
[GPUI](https://www.gpui.rs) framework (the UI toolkit behind the Zed editor).
A frameless window with a small menu bar in its title bar (hide it with a
shortcut), and three views: source, split, and a rendered view you edit in
place, block by block.

## Status

`0.1.0` is the first usable release. Expect rough edges; the editor and
preview are built on gpui-kit and gpui-pre, which are themselves young.

## What it does

- Source, split (resizable) or rendered view; the preview updates as you type.
- The rendered view edits in place: click a block and it turns into its
  Markdown source, sized to its text; click elsewhere or press Escape and it
  renders again. A click below the last block starts a new one; `/` there
  opens the block menu.
- Frameless window: File / Edit / View menus live in the title bar and can be
  hidden; full screen on a key.
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
| Source / Split / Rendered view | Ctrl+1 / 2 / 3 | Cmd+1 / 2 / 3 |
| Show or hide the menu bar | Ctrl+Shift+M | Cmd+Shift+M |
| Full screen | F11 | Ctrl+Cmd+F |
| Insert block (on an empty line) | `/` | `/` |
| Edit a block in the rendered view | click it | click it |
| Finish editing a block | Escape (or click elsewhere) | Escape (or click elsewhere) |
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

## Make it the default for `.md`

- **macOS**: use the `smep.app` from a GitHub release (a `cargo install`
  binary has no bundle, so Finder cannot pick it). Right-click a `.md` file,
  Get Info, Open with: smep, Change All. Files opened this way land in the
  running window.
- **Windows**: `scripts\register-windows.ps1` adds smep to the "Open with"
  list (current user, no admin); then choose it once in Settings, Apps,
  Default apps, or right-click, Open with, Always.
- **Linux**: copy `assets/smep.desktop` to `~/.local/share/applications/`
  and run `xdg-mime default smep.desktop text/markdown`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you shall be dual licensed as above, without any
additional terms or conditions.
