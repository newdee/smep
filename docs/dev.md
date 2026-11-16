# Developing smep

smep is a single crate on top of [gpui-kit](https://github.com/longbridge/gpui-kit),
which pins Zed's GPUI (`gpui-pre`) from crates.io. There are no git dependencies,
so `cargo install smep` and `cargo build` work from a plain checkout.

## Toolchain

- Rust stable, at least the `rust-version` in `Cargo.toml`.
- A C compiler. smep itself is pure Rust and uses no tree-sitter, but the
  TLS stack GPUI ships (`aws-lc-sys`) builds C code. It uses `cmake` when
  present and falls back to the plain C compiler otherwise (verified on
  Windows with only the MSVC build tools installed).

## Linux (including WSL2 with WSLg)

Build dependencies on Debian/Ubuntu:

```sh
sudo apt install -y clang cmake lld \
  libasound2-dev libfontconfig-dev libwayland-dev libx11-xcb-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libzstd-dev libvulkan1 \
  mesa-vulkan-drivers vulkan-tools fonts-noto-cjk
```

`fonts-noto-cjk` is only needed to render CJK text in the window; without it
those glyphs show as boxes.

Then:

```sh
cargo run -- README.md
```

### WSL notes

- Keep the build directory on the WSL filesystem. If the checkout lives under
  `/mnt/c`, set `CARGO_TARGET_DIR=$HOME/.cache/smep-target`.
- Check which Vulkan driver WSL picked up with `vulkaninfo --summary`.
  `dzn` (Direct3D 12 bridge) is hardware accelerated; `llvmpipe` is software
  rendering, which is slower but fine for development.
- If the window never appears under Wayland, force X11:
  `WAYLAND_DISPLAY='' cargo run`.

## Windows

Install the MSVC build tools (the Rust installer offers them), then
`cargo run`. Nothing else is required. Windows is verified by CI on every
push; day-to-day development happens on Linux.

`smep.exe` is linked as a GUI program (`windows_subsystem = "windows"` in
`main.rs`), so Explorer opens it without a terminal. Started from a
terminal, it attaches to that terminal's console first, so `--version`,
panics and anything on stderr still show in the window you ran `cargo run`
from, and `smep --version > v.txt` still lands in the file (the redirected
handles are kept). PowerShell is the exception: it does not wait for GUI
programs, so `$v = smep --version` stays empty there; `cmd /c` does it.

## macOS

Xcode command line tools, then `cargo run`.

## Releasing

1. Bump `version` in `Cargo.toml`, run `cargo check` so `Cargo.lock` follows,
   commit, push, wait for CI.
2. `cargo publish --locked` (the crates.io token comes from the gitignored
   `.env`: `set -a; . ./.env; set +a` first; in PowerShell,
   `Get-Content .env | ForEach-Object { if ($_ -match '^(\w+)=(.*)$') { Set-Item "Env:$($matches[1])" $matches[2] } }`).
3. `git tag -a vX.Y.Z -m "smep X.Y.Z" && git push origin vX.Y.Z`.

The tag runs `.github/workflows/release.yml`, which publishes a GitHub
release with a Linux tarball, a Windows zip and a universal macOS `smep.app`.
Run it by hand for an existing tag with
`gh workflow run release.yml -f tag=vX.Y.Z`.

The Homebrew cask and formula in [newdee/homebrew-tap](https://github.com/newdee/homebrew-tap)
(`Casks/smep.rb` installs `smep-vX.Y.Z-macos-universal.zip`, the `.app`,
and links its binary as the command; `Formula/smep.rb` installs
`smep-vX.Y.Z-macos-universal.tar.gz`, the bare universal binary the
release ships next to the zip; macOS only) follow the latest published
release on their own: a scheduled workflow there bumps both to the same
version, installs and tests each on a macOS runner, and commits the bump
only if that passes. To take a release at once instead of within six
hours: `gh workflow run smep.yml --repo newdee/homebrew-tap`. Both tests
run `smep --version`, which must print `smep X.Y.Z`. The cask exists
because a bare binary is a command-line tool to macOS: Finder and "Open
With" run it inside a Terminal window, and it cannot claim `.md` files.

The macOS app is signed and notarized only when these repository secrets
exist (they are the same six magpie uses). Without them it is ad-hoc signed
by the linker, which is enough for the formula (a binary run from the
terminal) and `cargo install`, while Finder refuses to open the bundle
until a Developer ID signature is there:

| Secret | Contents |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` password |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notarization: Apple ID, an app-specific password, the team id |

GitHub cannot copy secrets between repositories, so they have to be entered
once more. `scripts/set-apple-secrets.sh path/to/DeveloperID.p12` (or the
`.ps1` twin on Windows) asks for each value with hidden input and stores all
six; nothing is echoed or written to disk.

## Import and export

`src/convert.rs` turns PDF (`pdf-inspector`), Word (`docx-rust`), HTML
(`htmd`) and text into Markdown; `src/export.rs` turns Markdown into Typst
markup and compiles it (`typst-as-lib`, `typst-pdf`) with the fonts
installed on the machine (no fonts are embedded: Typst's own would add ~10 MB,
and every desktop has some; CJK needs a CJK font present). The export
test reads the text back out of the PDF it made. On Linux, CI installs
`fonts-dejavu-core fonts-noto-cjk` for that; a bare box without any font
cannot export.

## The look

A theme is a `Palette` in `src/theme.rs` (the blog's tokens: paper, ink,
soft and faint ink, accent, wash, the code board's surface, frame and
band, hairlines; serif or not) set in one of two designs, `Design::Book`
or `Design::Blog`. Each design has three outputs: the window, the HTML
page, the PDF.

Book (Book, Book Dark, and System, which is one of them with the mode) is
a printed book's page:

- the face is Newsreader, bundled: `assets/fonts/Newsreader-*.ttf` are
  static Regular, Italic, SemiBold, Bold and Bold Italic, cut from the
  variable font (wght, opsz 18) with fontTools' instancer and renamed by
  hand (the instancer cannot name an opsz instance; typographic family
  "Newsreader", OS/2 weight and italic bit per face; a test reads them
  back). `theme::install` adds them to gpui's text system, and `to_pdf`
  hands them to Typst. CJK glyphs come from `theme::BOOK_FALLBACKS`,
  gpui's per-font fallback list (`FontFallbacks`, set on the page by
  `Palette::page`): Noto Serif SC, Source Han Serif, Songti SC, SimSun.
  The test platform's text system resolves any name, so the fonts were
  checked once against DirectWrite from a hidden window (5 distinct
  faces; CJK in Noto Serif SC with the list, SimSun with SimSun alone,
  the system's sans without one);
- 1.125 times the application's text size (18 px at 16), leading 1.78
  (Latin alone would take 1.6; CJK wants more), paragraphs 1.1 rem apart;
- headings on a closer scale (1.75 / 1.32 / 1.1 / 1.0 / 0.92 / 0.92),
  all semibold, leading 1.3, no rule and no marks;
- code at 0.8 on a flat tint with rounded corners, the language as a
  word in the accent; quotes behind gpui-base's hairline;
- the column in ems of the body (`Palette::measure`: 32, 38, 50), so the
  saved `text_width` keeps its meaning across designs; medium is 38 CJK
  characters a line;
- the HTML page uses the same values (`export::book_stylesheet`; the
  reader's machine needs Newsreader, else Georgia), the PDF too
  (`export::book_preamble`, 11.5 pt, no SimSun because it has no bold).

Blog (every other theme) is dfine-blog's article page
(`src/styles/components.css` and `tokens.css` there): the heading scale
(1.62 / 1.46 / 1.2 / 1.02 / 0.92 / 0.92 of the body), bold to h3 and
semibold below, a hairline under h2, a 0.95 rem paragraph gap, code at
0.85 on a board (light paper: a 5 px frame, a faint shadow, a sunken
surface; dark paper: a 1 px frame whose top edge is 2 px of the accent),
tables ruled across with a 2 px faint-ink rule under the header and no
frame. From the palette:

- the window: `Palette::text_view_style` builds gpui-base's
  `TextViewStyle` (`with_heading`, `with_code_block`, `with_table*`); the
  dark board's top edge is an inset box shadow, since gpui draws one
  border colour per box. `rendered::code_block_actions` puts the
  language tab (the accent on its soft wash) and a Copy button in the
  board's corner, where the blog has a band along the top. The renderer
  has no hook for the quote's wash and mark, the section break, list
  markers, images, bold's underlay or the marks before h3 and h4, so
  those keep gpui-base's defaults on screen; nor for serif headings, kept
  out on screen because a CJK serif face with a bold is not on every
  machine. The page being read is paper the width of the pane with the
  text in a centred column (`settings::TextWidth`: 44, 56 or 72 rem in
  Blog, or the pane less `theme::PAGE_MARGIN` either side): the column comes from
  the `TextView`'s own horizontal padding (`Smep::reading_margin`), so
  the list and its scrollbar still span the pane. The text sits at the
  blog's 1.85 leading (set on the pane; gpui-base's paragraphs inherit the
  line height). The wheel over the paper beside the column scrolls the
  list. The block view keeps the same column (`rendered::column` inside
  each full-width block row) and leading; the split's preview fills its
  pane;
- an HTML page: `export::stylesheet` writes all of it as CSS, the blog's
  rules nearly verbatim, and `export::code_boards` wraps each code block
  the Markdown rendering wrote in the blog's `figure.code` with its band;
- a PDF: `export::preamble` writes it as Typst `set` / `show` rules. The
  markup `to_typst` emits carries no colour, stroke or size of its own
  (a test checks), so the preamble is the whole look. A PDF takes the
  light theme of the preview's family (`PreviewTheme::light`): print is
  ink on paper. Typst sets raw text at 0.8 of its surroundings by itself,
  so the preamble gives code an absolute size; and `par(..)` around a raw
  block drops its text from the page (a block in a paragraph), so the
  board sets `par` with a `set` rule instead (the read-back test caught
  it). Every inline call the writer emits (`#emph[..]`, `#link(..)[..]`,
  ...) ends with a `;` of its own (`export::call`): Typst reads on past a
  call into the text right after it, swallowing a `;` and taking `.x`,
  `(` or `[` as more of the call.

## Tabs, the outline, one smep

`app::Smep` is the window; each open document is a `DocumentTab` (its
editor, preview, rendered view, path and dirty state, its block table and
headings). Only the tab on screen while editing has a writable rendered
view (`show_active`), so a block edit never outlives its tab being shown.
Async work names a tab by `TabId`, never by index: tabs close meanwhile.
Opening a file uses up a blank tab (`is_pristine`) or adds one; a file
already open is shown, and Save As onto a file another tab holds is
refused (two tabs would overwrite each other). Closing the window or quitting asks about each
unsaved tab in turn, showing it (`confirm_discard_all`).

The outline comes from the same background parse as the preview's block
table (`highlight::outline`): each heading's line and the top-level block
holding it. A jump scrolls the reading page's list to the block, moves the
editor's cursor to the line (the split's preview follows the editor), or
scrolls the block view.

What the outline lists is `src/outline.rs`, pure and tested on its own:
every heading at the chosen depth or above (`Settings::outline_depth`, 1 to
6, the chips 1 / 2 / 3 / All in its head), less those inside a folded
section. A section runs to the next heading of its level or above; it can
fold when it holds a heading the depth lists, and its triangle (▾ / ▸)
folds it without going there. Folds are the tab's own (`DocumentTab::folded`,
not saved, cleared when a document is loaded into the tab) and are keyed by
level, title and which of the same-titled headings it is
(`outline::SectionKey`), so they stay with their section while the text
above changes. The section being read, when folded away or past the depth,
is marked by the nearest listed section holding it (`outline::shown_for`).
The tests click the triangles and chips through gpui's `debug_selector`
(recorded in debug test builds only).

`src/instance.rs` keeps one smep per user and config directory: the
running one listens on a local socket (`interprocess`; a named pipe on
Windows, `smep.sock` in the config directory elsewhere), and a later
launch hands its absolute paths over and exits. On Windows the listener
sends its process id first, so the caller can `AllowSetForegroundWindow`
it. A separate `SMEP_CONFIG_DIR` is a separate smep, which is how tests
and smoke runs keep out of the user's.

`smep --css [THEME]` and `smep --typst [THEME]` print the two; saved as
`export.css` / `export.typ` in the config directory (`Settings::dir`,
next to `settings.toml`) they replace the built-in ones for every export,
whatever the theme. Tests never read the user's directory: a `Settings`
without a path has no directory.

## Vim mode

`src/vim.rs` is the engine, pure and tested on its own: `Vim::key` takes a
key, the text, the cursor and the selection, and answers `Outcome::Pass`
(the editor or the window handles the key as usual) or
`Outcome::Handled(ops)`: edits, cursor moves, a selection to show, or an
action (undo, redo, page up/down, the command line, `:w` and `:q`). It
never holds the text, so the
same engine drives the source editor and a block editor. Offsets are UTF-8
bytes on character boundaries; a line ends at `\n` or at the `\r` of `\r\n`
(`line_end`, `after_break`, `newline`), so CRLF documents stay CRLF.

The keys reach it first: `Smep::new` registers a keystroke interceptor
(`cx.intercept_keystrokes`), which runs before the key bindings and the
input handler. `Smep::on_vim_key` passes everything while Vim mode is off,
while reading, while a menu is open, or when no editor has the focus; in
Insert mode it passes every key but Escape without reading the text. A
taken key stops propagation, which also stops its text: Windows skips
`TranslateMessage` (no `WM_CHAR`) for a handled key-down, macOS skips
`insertText:`, and the test platform's `dispatch_keystroke` skips the input
handler. A CJK input method in its native mode sees the key before gpui
does, so its text can still reach the document in Normal mode (README).

The command line (`:`, `/`, `?`) is a real single-line `InputState` at the
foot of the pane (`Smep::vim_prompt`), so an input method and paste work
in it: the engine answers `Op::Prompt` and keeps the command half typed
(`d/` waits for its search); the interceptor takes Enter, Escape and
Backspace-on-empty while the line has the focus, and lets the rest type;
`Vim::submit` (or `Vim::cancel`) finishes the command; `.` repeats `d/foo` as
`dn`. Typed in Visual mode, `:` ends Visual mode (keeping `'<` `'>`) and the line
starts with `Vim::prompt_text` (`'<,'>`). Ex commands come back as `Op::Write` / `Op::Quit` / `Op::WriteQuit`,
which `Smep::run_vim_commands` carries out with `save_task` and
`remove_tab` (`:q` on the last tab removes the window, and smep quits).
A click elsewhere in the window (a blur while the window is active) or a
tab switch closes the line without running it; another application coming
to the front leaves it open. Messages (`Vim::message`) go with the next
keystroke, whoever takes it.

`>` and `<` are operators that always take whole lines (`Vim::shift`):
the indent is rebuilt in spaces, 'shiftwidth' a step (`vim::SHIFT_WIDTH`, 2, by
default: gpui's `TabSize` default), the same as the editor's Tab, which
`:set sw` changes too (a test types both and compares).
The shift is one edit, trimmed to the span from the first indent changed to
the last, so it is one undo step.

Search highlighting ('hlsearch'): `Vim::highlight` is the last search while
`highlighting` holds (set by every search motion, cleared by `:noh`);
`Highlight::matches` lists its matches. `Smep::sync_search_marks` shows
them in the source editor as a gpui `RangeDecorationCollection` (fills, one
per match), made per tab on first use. It runs after each Vim key, the
command line closing, a tab shown and Vim mode switched; and on every change
of a tab's text that has marks, finding the matches again (2 MB with 187,000
matches: about 0.1 s in a debug build). Only the tab on screen holds marks.
`set_value` (`Smep::load`) emits no change and clears decorations, so `load`
marks the tab unhighlighted. Block editors are `TextareaState`, which has no
range decorations: they get no highlight.

Patterns (`src/vim/pattern.rs`): `Pattern::new` translates Vim's syntax
(magic, `\v`, `\M` / `\V`, `\c` / `\C`) to the `regex` crate's and compiles
it with `multi_line` and `crlf`, so `^` / `$` are line anchors and `$` stops
before `\r\n`; `.` matches no line break, `\n` is `\r?\n`, `\<` `\>` are
`\b{start}` `\b{end}`. What the crate cannot do (back references, `\zs`,
look-around) is an error, never a silent change of meaning. 'ignorecase' and
'smartcase' pick the case; `*` and `#` escape the word (`pattern::escape`),
skip 'smartcase' and keep the old whole-word check (`is_whole_word`, Vim's
word classes, CJK apart), not `\<\>`. A `:set ic` recompiles the last search,
so `n` and the highlight follow it. `expand` builds a replacement from a
match's captures; `with_previous` puts in `~`.

Ex lines (`src/vim/ex.rs`): `parse_range` reads `%`, `.`, `$`, `N`, `'<`,
`'>`, `+N` / `-N`, `,` / `;` (a backwards range is swapped); `parse_substitute`
the delimiter, pattern, replacement and flags; `Options::set` every `:set`
form, stopping at the first error. `Vim::substitute` walks the matches from
the range's first line start (only those starting in the range; one per line
without `g`; an empty match right after a match skipped, as Vim does; never a
match that starts or ends inside a CRLF, an empty one between its `\r` and
`\n` included), builds the
new text of the lines and answers one edit trimmed to the changed span, so
`u` takes it back at once. `:set` answers `Op::Options` when 'number',
'wrap' or 'shiftwidth' changed; `Smep::apply_editor_options` (`show_options`)
sets them on every tab's editor (`set_line_number`, `set_soft_wrap`,
`set_tab_size`), on new tabs, and back to smep's defaults while Vim mode is
off. A block editor's Tab types a tab character whatever its `TabSize`, so
it is left alone.
`ActiveEditor::apply_vim` makes the edits with `set_selected_range` +
`replace` (each one an undo step) and runs the actions through the editor's
own focus handle (`FocusHandle::dispatch_action`, synchronous); after an
undo it collapses any restored selection, or the next key would read it as
a mouse selection and start Visual mode. Normal mode's block cursor is a
deferred overlay at `range_to_bounds` of the character under the cursor;
the mode and the keys of a pending command sit in the pane's corner. The
mode is the window's, not the editor's: switching editor (`retarget`) ends
Visual mode and a half-typed command, and starting to write resets to
Normal mode. `Settings::vim_mode` keeps the switch (Edit, Vim Mode).

## Icon

`assets/icon/logo.svg` is the design (a cinnabar seal with an M on the
Paper look's paper); `assets/icon/render.py` (Pillow) draws the same
geometry at 16 to 1024 px and packs `smep.ico`. Run it after changing the
design and commit the PNGs and the `.ico`. Where each platform takes it:

- Windows: `build.rs` embeds `smep.ico` as resource 1 with `winresource`
  (gpui loads icon 1 for the window; Explorer shows it). It needs the
  Windows SDK's `rc.exe`; without it the build warns and goes on without
  the icon.
- macOS: the release workflow makes `smep.icns` from the PNGs with
  `iconutil` and names it in `Info.plist` (`CFBundleIconFile`).
- Linux: `smep.desktop` names `Icon=smep`; "Open Markdown Files with smep"
  writes `smep-256.png` into `~/.local/share/icons/hicolor/256x256/apps/`,
  and the release tarball carries it as `smep.png`. The window sets
  `app_id` `smep` so the desktop matches it to the entry.

## Checks before pushing

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs the same three on Ubuntu, Windows and macOS, plus a release build.
