# smep — a Simple Markdown Editor & Previewer, written in Rust

<p align="center"><img src="https://raw.githubusercontent.com/newdee/smep/main/assets/icon/smep-256.png" width="128" height="128" alt="smep: a cinnabar seal with an M on paper"></p>

[![crates.io](https://img.shields.io/crates/v/smep.svg)](https://crates.io/crates/smep)
[![CI](https://github.com/newdee/smep/actions/workflows/ci.yml/badge.svg)](https://github.com/newdee/smep/actions/workflows/ci.yml)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**smep** is a markdown editor with live preview, built in pure Rust on the
[GPUI](https://www.gpui.rs) framework (the UI toolkit behind the Zed editor).
A frameless window with a small menu bar in its title bar (hide it with a
shortcut), and three views: source, split, and a rendered view you edit in
place, block by block.

## Status

`0.1.18` is the nineteenth release: Vim mode gets its command line,
`:w`, `:q`, `:q!`, `:wq`, `:x`, and searches with `/`, `?`, `n`, `N`, `*`
and `#`. 0.1.17 brought Vim mode (Edit, Vim Mode), which gives the
editor and the rendered view's block editors Normal, Insert and Visual
modes, with counts, operators, text objects, `.` and a block cursor. 0.1.16
let the outline fold a section by the triangle before it and list headings
down to a level you pick (1, 2, 3 or All in its head). 0.1.15 set the document as a book sets
it, in the new Book theme and its dark twin, the default: Newsreader
(built in) with CJK text in the system's Song face, a size up, a
reader's leading, a book's measure, quiet headings and code on a flat
tint, on screen, in the exported page and in the PDF; and a PDF no
longer loses a `;` (or breaks on a `(`) right after emphasis, a link or
code. 0.1.14 let you choose the column's width (View, Text Width) with
the scrollbar at the window's edge, and folded the outline away to a
strip. 0.1.12
put documents in tabs (a file opened from the file manager, or `smep FILE`, while smep runs lands in a
tab of the open window), an outline of the headings sits beside the
document and a click goes to the section, and the document is set the
way dfine-blog sets an article (code on a board with a cinnabar tab, a
cinnabar top edge on dark paper, tables ruled across) on screen, in the
exported page and in the PDF. Before that, 0.1.11 made the stylesheet
and the Typst preamble behind the exports yours to edit (`smep --css`,
`smep --typst`, then `export.css` / `export.typ` in the config
directory) and showed a document's local images in the preview, on
gpui-kit 0.8; and smep got an icon (a cinnabar
seal with an M on paper) in the Windows executable, the macOS app and
the Linux desktop entry. The Paper look is dfine-blog's (warm white or near-black
paper, grey inks, cinnabar), the rendered document is typeset the blog's
way in every preview theme, reading mode shows the document as one page
(footnotes and selections work across blocks), and the title bar's toggle
is Write / Read, no longer a second "Edit". On a Mac, `brew install --cask newdee/tap/smep` installs `smep.app` (Finder,
Spotlight, the Dock, "Open With", no Terminal window) together with the
`smep` command. Reading mode scrolls from the keyboard and renders `.html`
files as pages, `smep notes.md` opens a new document when the file is not
there yet, and the Windows "Open Markdown Files with smep" no longer
flashes a console. It opens reading, and Ctrl+E / Cmd+E or the Write
button in the title bar brings the editor. On Windows it opens without a
terminal window, and it can make itself the default app for Markdown
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
- Preview themes (Book and Book Dark, Paper and Paper Dark, GitHub,
  Newsprint, Night, Sepia, Solarized Light/Dark, or System, which is Book
  light or dark with the system): right-click the preview or the rendered
  document. The choice is remembered in
  `settings.toml` under the platform config directory (`~/.config/smep`,
  `~/Library/Application Support/smep`, `%APPDATA%\smep`, or
  `$SMEP_CONFIG_DIR`).
- Opens reading: the document rendered as one page, set as a book sets it
  in a column centred on the paper, its width yours
  (View, Text Width: narrow, medium, wide or full; the wheel and the
  scrollbar at the window's edge scroll it from anywhere),
  nothing to click into by accident, and the keyboard scrolls it (Page Up / Page Down, Space,
  Home / End, the arrows). The Write button in the title bar (or Ctrl+E)
  brings the editor in the view mode you last used; Read (or Ctrl+E again)
  puts it away.
- Markdown highlighting in the editor from the same parser the preview uses.
- The preview follows the editor: whatever block is at the top of the editor
  is at the top of the preview.
- CommonMark + GFM: tables, task lists, strikethrough, footnotes. Raw HTML
  inside Markdown renders too, and `.html` files open straight into the
  HTML preview.
- Tabs: every document opens in a tab of its own (the bar shows from the
  second one); Ctrl+T a new one, Ctrl+W closes one, Ctrl+Tab moves on. A
  file opened from the file manager, or `smep FILE` while smep is running,
  lands in a tab of the window that is already open.
- The outline: the document's headings beside it, shown from the start;
  a click goes to that section, and the section being read is marked.
  The triangle before a section folds the headings it holds; 1, 2, 3 and
  All in the outline's head list the headings down to that level. Fold it
  away (its ‹, the Outline button or Ctrl+Shift+L) to a strip along the
  edge, and unfold it from there.
- Vim mode (Edit, Vim Mode; off until you turn it on, then remembered):
  the editor and the block being edited in the rendered view take Vim's
  keys, with Normal, Insert, Visual and Visual Line modes, the mode in the
  corner of the pane and a block cursor in Normal mode. See
  [Vim mode](#vim-mode) for the keys.
- Open, save, save as; an unsaved-changes prompt before a tab or the
  window closes, for each document with changes.
- Other documents become Markdown: drop a PDF, Word (.docx) or text file on
  the window, pick one under File, Import as Markdown (HTML too), or pass it
  on the command line. The result is a new, unsaved document named after
  the original. PDFs need a text layer; smep does no OCR. Images in Word
  files are not carried over.
- Export as PDF (typeset by [Typst](https://typst.app), built in: headings,
  lists, tables, code, quotes, footnotes, local images; Book's serif is
  built in, CJK text uses the system's fonts) or as a standalone HTML page,
  under File. Both wear the
  preview's look (a PDF the light one of its family: print is ink on
  paper). `smep --css` and `smep --typst` print the stylesheet and the
  Typst preamble behind them; saved as `export.css` or `export.typ` in
  the config directory, your edited copy is what every export uses.
- Set like a book: the document reads in Book, the default, on screen, in
  HTML and in PDF. Its serif is
  [Newsreader](https://github.com/productiontype/Newsreader), built into
  smep, with CJK text in the system's Song face (Noto Serif SC, Source Han
  Serif, Songti SC, SimSun, the first one installed); 18 px at the default
  size, a reader's leading, a column of about 38 CJK characters (medium),
  quiet semibold headings, code on a flat tint, quotes behind a hairline.
- Ink on paper: the application wears the Paper look of
  [dfine-blog](https://github.com/newdee/dfine-blog), light or dark with
  the system (warm white or near-black paper, grey inks, cinnabar for the
  cursor, headings and links). Every theme but Book is set the way the blog
  sets an article, on screen, in HTML and in PDF: its
  heading scale with a fading hairline under h2, code on a board (a
  whiteboard in a thick aluminium frame on light paper, a thin frame with a
  cinnabar top edge on dark) with the language in a cinnabar tab and a Copy
  button, tables ruled across under a heavier header rule. The page and
  the PDF add what the window cannot draw yet: cinnabar marks before h3 and
  h4, the quote on a wash under a cinnabar 「, the faded section break,
  framed images.
- One static binary, no web view, no Electron.

## Keys

| Action | Windows / Linux | macOS |
|---|---|---|
| New tab | Ctrl+T | Cmd+T |
| Close tab | Ctrl+W | Cmd+W |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab (or Ctrl+PgDn / PgUp) | Ctrl+Tab / Ctrl+Shift+Tab (or Cmd+Shift+] / [) |
| Outline | Ctrl+Shift+L | Cmd+Shift+L |
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

### Vim mode

Turn it on under Edit, Vim Mode. Writing then starts in Normal mode. The
window's shortcuts (Ctrl+S, Ctrl+Tab, Ctrl+E, …) work in every mode. In
Normal and Visual mode the arrows, Home and End are `h` `j` `k` `l`, `0`
and `$`; Shift+arrows select as usual, and Page Up / Down scroll.

| Keys | What they do |
|---|---|
| `i` `a` `I` `A` `o` `O` | Insert mode (before, after, line start, line end, new line below, above) |
| Escape (or Ctrl+[) | Back to Normal mode; in Normal mode it does what it does without Vim (it finishes a block edit) |
| `h` `j` `k` `l`, `w` `b` `e`, `W` `B` `E`, `0` `^` `$`, `gg` `G`, `{` `}`, `%` | Move (with a count: `3w`, `5j`, `12G`) |
| `f` `F` `t` `T` + a character, `;` `,` | Move to a character on the line, and again |
| `d` `c` `y` + a move or an object | Delete, change, yank (copy): `dw`, `c$`, `y}`, `d3j` |
| `dd` `cc` `yy`, `D` `C` `Y`, `x` `X` `s` `S` | Lines and characters |
| `iw` `aw`, `i"` `a"`, `i(` `a(` (`ib`), `i[`, `i{` (`iB`), `i<` | Objects, after an operator or in Visual mode: `ciw`, `da(` |
| `p` `P` | Put after / before (whole lines below / above) |
| `r` + a character, `J`, `~` | Replace, join lines, switch case |
| `u`, Ctrl+R | Undo, redo |
| `.` | Repeat the last change made from Normal mode (with what was typed in Insert mode); Visual-mode changes are not repeated |
| `v`, `V` | Visual, Visual Line; then a move, `o`, `d` `c` `y` `p` `r` `J` `~` `u` `U` |
| `/` `?` + text + Enter, `n` `N` | Search forward, backward; the next match, the one before (round the end of the document) |
| `*` `#` | Search for the word under the cursor, forward, backward |
| `:w`, `:q`, `:q!`, `:wq`, `:x` | Save; close the tab (`:q` refuses while it has unsaved changes, `:q!` drops them); save and close. On the last tab, closing closes smep |
| `:12`, `:$` | Go to line 12, to the last line |
| Ctrl+D, Ctrl+U | Page down, page up |

`:`, `/` and `?` open a command line along the foot of the pane: type there
(an input method works), Enter runs it, Escape (or Backspace on an empty
line) closes it. A search is for the text as typed, case for case, not a
pattern; `/` + Enter alone searches for the last text again, and a search
works after an operator (`d/foo`) and in Visual mode. Messages (a search
going round the end, an error) show in the pane's corner until the next
key. Ctrl+F still opens the editor's find bar.

Text selected with the mouse (or Ctrl+A) starts Visual mode. Yanks go to
smep's own register, not the system clipboard (Ctrl+C / Ctrl+V still
reach that). Not there: other `:` commands (`:w` with a file name, `:s`,
ranges), patterns in searches, match highlighting, marks, macros, named
registers, `>` / `<`.

Note: Normal mode reads keys, not text. With a CJK input method in its
native mode, the keys reach the input method first and its text lands in
the document, so switch it to English (or Latin input) in Normal mode.

## Install

```sh
cargo install smep
smep notes.md
```

A file that does not exist yet opens as a new document and is written there
on the first save. Several files open in tabs, and while smep is running
`smep FILE` hands FILE to that window instead of opening another. On a Mac, Homebrew installs the application:

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
install` or the release tarball there.) `smep --version`, `smep --help`,
`smep --css [THEME]` and `smep --typst [THEME]` print and exit without
opening a window. On Windows smep is a GUI program:
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
- **Linux**: smep writes `~/.local/share/applications/smep.desktop` (and
  its icon into `~/.local/share/icons`) and runs `xdg-mime default
  smep.desktop text/markdown`. (`assets/smep.desktop` and the `smep.png`
  in the release tarball are the same files, for doing it by hand.)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you shall be dual licensed as above, without any
additional terms or conditions.
