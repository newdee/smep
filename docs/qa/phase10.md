# QA log — Phase 10: review after 0.1.6 (bugs and small improvements)

Scope: the commit "Render HTML files as HTML when reading; scroll the rendered view from the keyboard;
fix the console flash behind Make Default; open a new file by name"
(`src/rendered.rs`, `src/app.rs`, `src/defaults.rs`, `src/io.rs`, `src/main.rs`, `tests/command_line.rs`,
`Cargo.toml`, README).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Findings of the review (each classified before the fix)

| # | Finding | Kind | Evidence | Fix |
|---|---|---|---|---|
| 1 | An `.html` file opened reading (the default since 0.1.6) showed Markdown's reading of its tags, not the page: the rendered view had no notion of the document's format, only the split preview had | structural (one missing concept), one place; it existed in rendered mode before 0.1.6 but the default view was split then | `RenderedView` always called `TextView::markdown` and `highlight::block_ranges`; a Markdown parse of a 4-tag page gives 3 blocks | `RenderedView` carries the `PreviewFormat`; HTML is one block rendered by `TextView::html`; `Smep::set_preview` (the single place the format changes) tells it, the constructor passes it |
| 2 | File > "Open Markdown Files with smep" on Windows flashed a console window: smep is a GUI program since 0.1.5 and the `cmd /C start` it runs got a console of its own | boundary, one place (the only `Command` on Windows) | probe `console-probe/cmd-flash.ps1`: a GUI process (`pythonw.exe`) running `cmd /C timeout` → console windows 0 → 1; with `CREATE_NO_WINDOW` 0 → 0 | `creation_flags(CREATE_NO_WINDOW)` (feature `Win32_System_Threading`) |
| 3 | Reading a long document, the keyboard did nothing: only the wheel scrolled | gap, one place | — | `RenderedView::on_key_down`: ↑/↓ a line (40 px), Page keys and Space (Shift+Space up) a page less a line, Home/End; ignored while a block editor is typed in or with Ctrl/Alt/Cmd held |
| 4 | While reading, a click on a block stopped the event at the block (it did nothing else), so the view never took the focus from a click; the container's own click handler also called `prevent_default` in reading mode | boundary, two places with one cause | first smoke run of finding 3: PageDown after a click on text did not scroll; `TextView` takes the focus for selection and the first key guard asked for the view's own focus | read-only clicks let the event through and prevent nothing; the key guard asks only that no block editor holds the focus |
| 5 | `smep notes.md` for a file not there yet exited with "cannot read", the README's own install snippet | gap, one place (the dialog and the OS cannot name a missing file) | `tests/command_line.rs` had asserted that very exit | `Document::open`: a missing file in an existing directory is a new document with that path; a missing directory stays an error |
| 6 | The preview theme could be picked by right-click on the split preview only; reading mode had no preview | gap, one place | — | right-click on the rendered document opens the same menu; an open block keeps its editor's own menu (stops the event) |

What was checked against new bugs: the Markdown path of the rendered view is unchanged (`parse_blocks` is `block_ranges` there; the block-list tests still pass), the key handler does not run while a block editor is focused (test types a Space into a block and the view stays), `Document::read` is untouched (the dialog, the OS and drops still report a missing file), and the one Windows `Command` keeps its arguments.

Accepted and noted, not a defect: appending a new block at the end of an HTML document in the rendered view keeps two HTML blocks on screen until the document is reopened (the text is right; the split preview renders the whole page).

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` (default and `--no-default-features`) | clean, clean | |
| `cargo test --locked` × 3 | 102 + 4 passed, 0 failed each time (98 → 102: +3 headless in `app.rs`, +1 unit in `io.rs`) | |
| `cargo test --locked --no-default-features` | 95 + 4 passed | |
| `Cargo.lock` | unchanged (a feature of a dependency already in the tree) | |

Mutation check (all four fixes removed at once, then put back): exactly the four new tests failed, 98 passed —
`an_html_document_is_one_html_block_in_the_rendered_view`, `keys_scroll_the_rendered_view_while_reading`,
`right_click_on_the_rendered_view_opens_the_theme_menu`, `open_starts_a_new_document_for_a_missing_file_in_an_existing_directory`.

## Round 1 — mechanism (Windows, real window)

Debug build, screenshots in the session scratchpad (`shot.ps1`, `key-shot.ps1`: a real click, then `keybd_event`; `SendKeys` and a bare `AppActivate` never reach a gpui window, Ctrl+E included):

| Step | Seen |
|---|---|
| `smep page.html` (reading) | the page: heading, bold, list, table, no tags (`html-reading.png`) |
| `smep brand-new.md` (no such file) | title `brand-new.md`, the empty-document hint, nothing written on disk (`new-file.png`, `Test-Path` false) |
| `smep long.md` (120 sections), click on the text, then PageDown / Space / Shift+Space / End / Home / ↓ | Section 1 → 9 → 17 → 9 → 112–114 → 1 → 1 with the heading 40 px under the bar (`fix2-click-*.png`) |
| the same keys before finding 4 was fixed | nothing moved (`keys-*.png`, `fix-click-*.png`) |
| focus at start | a temporary probe in `render` printed `rendered=true` on every frame from the first: the view holds the focus at start; a click on the title bar before the keys starts the title bar's window move and swallows them (not a smep matter) |

Round 1 result: 0 findings after finding 4 (which reset the count).

## Round 2 — static consistency and invariants

- README: themes "right-click the preview or the rendered document", reading bullet names the scrolling keys, key table has the scroll row, Install says a missing file opens as a new document; `--help` says the same. `docs/dev.md` needs nothing.
- `format` changes in one place (`set_preview`), which the constructor mirrors; `write_to` and `load` go through it.
- `parse_blocks` is the only block parser the rendered view uses (`blocks()` and `commit`), so Markdown and HTML cannot disagree between a full parse and a commit.
- The key handler clamps to `[-max_offset, 0]` and the container clamps again at prepaint; Home/End with Shift, and any key with Ctrl/Alt/Cmd, fall through to the editor's or the root's bindings.
- `Document::open` is used by `main.rs` only; the other three entry points keep `Document::read` on purpose.
- `CREATE_NO_WINDOW` is behind `cfg(windows)` with the only `Command` there; the Linux `Command`s run where a console is the terminal already open.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

(filled in after the push)
