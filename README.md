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

`0.1.4` is the fifth release: it wears the Paper look (keepane's paper and
paper-dark, light or dark with the system) and can make itself the default
app for Markdown from the File menu, on top of importing PDF, Word, HTML
and text files as Markdown, exporting the document as a PDF or an HTML
page, the frameless window with its view modes, in-place editing in the
rendered view, opening files from the OS, a Homebrew formula, and typing
that stays smooth in large documents. Expect rough edges; the editor and
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
- Preview themes (Paper and Paper Dark, GitHub, Newsprint, Night, Sepia,
  Solarized Light/Dark, or the application's own look): right-click the
  preview. The choice is remembered in `settings.toml` under the platform
  config directory.
- Markdown highlighting in the editor from the same parser the preview uses.
- The preview follows the editor: whatever block is at the top of the editor
  is at the top of the preview.
- CommonMark + GFM: tables, task lists, strikethrough, footnotes. Raw HTML
  inside Markdown renders too, and `.html` files open straight into the
  HTML preview.
- Open, save, save as; an unsaved-changes prompt before closing.
- Other documents become Markdown: drop a PDF, Word (.docx) or text file on
  the window, pick one under File, Import as Markdown (HTML too), or pass it
  on the command line. The result is a new, unsaved document named after
  the original. PDFs need a text layer; smep does no OCR. Images in Word
  files are not carried over.
- Export as PDF (typeset by [Typst](https://typst.app), built in: headings,
  lists, tables, code, quotes, footnotes, local images; CJK text uses the
  system's fonts) or as a standalone HTML page, under File.
- Ink on paper: the application wears keepane's Paper look, light or dark
  with the system (warm cream or warm near-black, Vitesse hues, cinnabar
  for the cursor and headings).
- One static binary, no web view, no Electron.

## Keys

| Action | Windows / Linux | macOS |
|---|---|---|
| Open | Ctrl+O | Cmd+O |
| Import as Markdown | Ctrl+Shift+O | Cmd+Shift+O |
| Save | Ctrl+S | Cmd+S |
| Save as | Ctrl+Shift+S | Cmd+Shift+S |
| Export as PDF | Ctrl+Shift+E | Cmd+Shift+E |
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

On a Mac, Homebrew works too:

```sh
brew install newdee/tap/smep
```

The formula installs the `smep` command from the release's own universal
build and follows new releases by itself. (No Linux formula: that binary
needs the system's X11, xkbcommon, Wayland and Vulkan libraries, which a
Homebrew prefix does not provide; use `cargo install` or the release
tarball there.) `smep --version` and `smep --help` print and exit without
opening a window. On Windows smep is a GUI program: no terminal window
comes up with it, and in a terminal it still prints. PowerShell neither
waits for a GUI program nor captures its output, so a script there asks
through `cmd /c smep --version` (or `Start-Process -Wait
-RedirectStandardOutput`).

Building from source needs a Rust toolchain and, on Linux, a few system
libraries; see [docs/dev.md](docs/dev.md). PDF import and PDF export are
the two heavy parts of the binary (about 7 MB and 34 MB); a build without
them, `cargo install smep --no-default-features`, keeps everything else
and reports the missing feature if asked for it.

## Make it the default for `.md`

File, "Open Markdown Files with smep" does what the system allows:

- **macOS**: smep claims Markdown files outright, when it runs as the
  `smep.app` from a GitHub release (a `cargo install` or Homebrew binary has
  no bundle, so Finder cannot pick it; the menu item says so). Files opened
  from Finder land in the running window. The app is Developer ID signed
  but not yet notarized, so the first open from Finder needs right-click,
  Open (or System Settings, Privacy & Security, Open Anyway).
- **Windows**: smep registers itself (current user, no admin) and opens the
  Settings page for it, where you choose smep next to `.md` once; Windows
  keeps that choice to itself. `scripts\register-windows.ps1` does the same
  registration from a terminal.
- **Linux**: smep writes `~/.local/share/applications/smep.desktop` and runs
  `xdg-mime default smep.desktop text/markdown`. (`assets/smep.desktop` is
  the same file, for doing it by hand.)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you shall be dual licensed as above, without any
additional terms or conditions.
