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

`0.1.8` is the ninth release: on a Mac, `brew install --cask newdee/tap/smep`
installs `smep.app` (Finder, Spotlight, the Dock, "Open With", no Terminal
window) together with the `smep` command. Reading mode scrolls from the
keyboard and renders `.html` files as pages, `smep notes.md` opens a new
document when the file is not there yet, and the Windows "Open Markdown
Files with smep" no longer flashes a console. It opens reading, and Ctrl+E
/ Cmd+E or the Write button in the title bar brings the editor. On Windows
it opens without a terminal window. It wears the Paper look (warm white or
near-black paper, grey inks, cinnabar, light or dark with the system) and
can make itself the default app for Markdown
from the File menu, on top of importing PDF, Word, HTML
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
  preview or the rendered document. The choice is remembered in
  `settings.toml` under the platform config directory.
- Opens reading: the document rendered as one page, nothing to click into
  by accident, and the keyboard scrolls it (Page Up / Page Down, Space,
  Home / End, the arrows). The Write button in the title bar (or Ctrl+E)
  brings the editor in the view mode you last used; Read (or Ctrl+E again)
  puts it away.
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
- Ink on paper: the application wears the Paper look of
  [dfine-blog](https://github.com/newdee/dfine-blog), light or dark with
  the system (warm white or near-black paper, grey inks, cinnabar for the
  cursor, headings and links), and the rendered document is typeset the
  way the blog is: its heading scale, paragraph gap, framed code blocks
  and tables ruled across only, whichever preview theme colours it.
- One static binary, no web view, no Electron.

## Keys

| Action | Windows / Linux | macOS |
|---|---|---|
| Open | Ctrl+O | Cmd+O |
| Import as Markdown | Ctrl+Shift+O | Cmd+Shift+O |
| Save | Ctrl+S | Cmd+S |
| Save as | Ctrl+Shift+S | Cmd+Shift+S |
| Export as PDF | Ctrl+Shift+E | Cmd+Shift+E |
| Write, or back to reading | Ctrl+E (or the Write / Read button) | Cmd+E (or the button) |
| Scroll the rendered document | Page Up / Page Down, Space / Shift+Space, Home / End, ↑ / ↓ | same |
| Source / Split / Rendered view (starts editing) | Ctrl+1 / 2 / 3 | Cmd+1 / 2 / 3 |
| Show or hide the menu bar | Ctrl+Shift+M | Cmd+Shift+M |
| Full screen | F11 | Ctrl+Cmd+F |
| Insert block (on an empty line) | `/` | `/` |
| Edit a block in the rendered view | click it | click it |
| Finish editing a block | Escape (or click elsewhere) | Escape (or click elsewhere) |
| Block menu / clipboard | right-click the editor | right-click the editor |
| Preview theme | right-click the preview or the rendered document | same |
| Find in editor | Ctrl+F | Cmd+F |
| Find and replace | Ctrl+H | Cmd+Shift+F |
| Document start / end | Ctrl+Home / Ctrl+End | Cmd+Up / Cmd+Down |

## Install

```sh
cargo install smep
smep notes.md
```

A file that does not exist yet opens as a new document and is written there
on the first save. On a Mac, Homebrew installs the application:

```sh
brew install --cask newdee/tap/smep
```

The cask puts `smep.app` in `/Applications` (Finder, Spotlight, the Dock,
"Open With", the default app for `.md`) and links its binary as the `smep`
command; both follow new releases by themselves. The app is Developer ID
signed but not yet notarized, so the first start from Finder is refused
once: allow it under System Settings, Privacy & Security, Open Anyway, or
install with `brew install --cask --no-quarantine newdee/tap/smep`.

A bare binary (`cargo install`, the formula `brew install newdee/tap/smep`,
the release tarball) is a command-line tool to macOS: Finder, Spotlight
and "Open With" run it inside a Terminal window, and only an application
can claim `.md` files. Take the cask or the release's `smep.app` for that.
(No Linux formula: that binary needs the system's X11, xkbcommon, Wayland
and Vulkan libraries, which a Homebrew prefix does not provide; use `cargo
install` or the release tarball there.) `smep --version` and `smep --help`
print and exit without opening a window. On Windows smep is a GUI program:
no terminal window comes up with it, and in a terminal it still prints.
PowerShell neither waits for a GUI program nor captures its output, so a
script there asks through `cmd /c smep --version` (or `Start-Process -Wait
-RedirectStandardOutput`).

Building from source needs a Rust toolchain and, on Linux, a few system
libraries; see [docs/dev.md](docs/dev.md). PDF import and PDF export are
the two heavy parts of the binary (about 7 MB and 34 MB); a build without
them, `cargo install smep --no-default-features`, keeps everything else
and reports the missing feature if asked for it.

## Make it the default for `.md`

File, "Open Markdown Files with smep" does what the system allows:

- **macOS**: smep claims Markdown files outright, when it runs as
  `smep.app` (the Homebrew cask or a GitHub release; a `cargo install` or
  formula binary has no bundle, so Finder cannot pick it, and the menu item
  says so). Files opened from Finder land in the running window. The app
  is Developer ID signed but not yet notarized, so a downloaded copy goes
  through Gatekeeper once:
  1. Move `smep.app` into `/Applications` (the cask does this).
  2. Open it; when macOS refuses, allow it under System Settings, Privacy
     & Security, Open Anyway (older systems: right-click, Open). That is
     a one-time pass for this copy of the app.
  3. Pick File, "Open Markdown Files with smep", or in Finder select a
     `.md` file, Get Info, Open With, smep, Change All.
  The association is kept by bundle id and survives updates; each newly
  downloaded version needs step 2 again until the app is notarized
  (`brew install --cask --no-quarantine` skips it).
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
