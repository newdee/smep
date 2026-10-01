# QA log — Phase 5: import (PDF, Word, HTML, text) and export (PDF, HTML)

Scope: commits "Import PDF, Word, HTML and text files as Markdown" (`src/convert.rs`, `src/io.rs`, `src/app.rs`, `src/main.rs`, `src/keymap.rs`)
and "Export the document as a PDF or an HTML page" (`src/export.rs`, `src/app.rs`, `src/keymap.rs`, `Cargo.toml`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Choices

- PDF text comes from `pdf-inspector` (pure Rust by default: `lopdf` + `ttf-parser`; the same crate magpie uses), through its per-page Markdown. Its OCR features pull in pdfium and ONNX (C++), so they stay off: a page without a text layer is marked in the output, a PDF with none is an error.
- Word through `docx-rust` (pure Rust): headings from paragraph styles, lists from the numbering definition (bullet vs numbered, level), bold/italic from run properties, links from the relationships part, tables as GFM tables. Images are dropped.
- HTML through `htmd`; text as is (line endings and blank runs normalised).
- PDF export through Typst (`typst-as-lib` + `typst-pdf`, pure Rust): the Markdown syntax tree is rewritten as Typst markup and compiled with the fonts installed on the machine (Typst's own fonts are not embedded; see Binary size). Chosen over `printpdf`/`genpdf` because they leave line breaking, pagination and tables to the caller; Typst does the typesetting. Cost: binary size, below.
- HTML export: GFM rendering from the `markdown` crate wrapped in a page with a small stylesheet.

## Findings (fixed before the rounds)

| Finding | Evidence | Fix |
|---|---|---|
| Two tests shared the temp directory `smep-export-<pid>`: one removed it under the other, so the export test failed only in the full parallel run | `exporting_writes_a_pdf_and_an_html_page_next_to_the_document` failed in the full run ("the PDF was written"), passed alone | distinct directory names |
| `typst::layout::PagedDocument` is not re-exported by the `typst` crate in 0.15 | `E0432` | depend on `typst-layout` directly |

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` × 3 | 86 passed, 0 failed each time (63 → 86: +15 unit in `convert`/`export`, +5 headless, +3 elsewhere) | |

The unit tests feed real documents: a PDF written by hand in the test (one page, Helvetica, a text layer), `.docx` files written by `docx-rust` itself and read back, HTML strings; the export test compiles a document with Typst and reads the text back out of the resulting PDF with the same reader the import uses (`pdf-inspector`), so "the words are in the PDF, in embedded fonts" is asserted, not assumed. On Windows and macOS the CJK line is checked too (system CJK fonts exist there; a bare Linux box may have none).

Binary size (local Windows release build, `strip = true`):

| Build | Bytes |
|---|---|
| 0.1.2 | 21,877,248 |
| + import (pdf-inspector, docx-rust, htmd), no LTO | 33,068,544 |
| + export, Typst with its embedded fonts, no LTO | 78,765,568 |
| + export, system fonts only, fat LTO (13 m 39 s to build) | 62,533,632 |

Rows above mix profiles (the import row predates LTO), so they do not give a feature's cost. Measured in one profile, the shipped one (thin LTO, `strip = true`, system fonts only), on 2026-10-02:

| Features | Bytes | Step |
|---|---|---|
| `--no-default-features` | 24,538,624 | |
| `pdf-import` | 31,217,152 | +6,678,528 |
| `pdf-import` + `pdf-export` (default) | 64,813,056 | +33,595,904 |

CI run 36693209479 (`76f74a5`, default features): Windows 64,817,152, macOS 51,028,416, Linux 74,421,560.

Typst's own fonts (the `typst-assets` `fonts` feature: Libertinus Serif, New Computer Modern and its math face, DejaVu Sans Mono; 17 files, 9,683,068 bytes in `typst-assets` 0.15.1, all `include_bytes!`) only cover Latin text and every desktop has fonts, so they are not enabled: `cargo tree -e features` shows `typst-kit` with `scan-fonts` only and `typst-assets` with `default` only. The two heavy parts are cargo features (`pdf-import`, `pdf-export`, both default) for anyone who wants a slim build: `cargo test --no-default-features` passes 81 tests, `cargo clippy` is clean in both configurations.

## Round 1 — static consistency

- README: import bullet (drop / File > Import / command line; HTML only through Import; PDFs need a text layer; Word images dropped), export bullet (what the PDF keeps; CJK via system fonts), the two new key rows (Ctrl+Shift+O, Ctrl+Shift+E), the feature note under Install → each matches `convert::Format::for_import` / `for_explicit_import`, `export::Writer`, `keymap.rs`, `Cargo.toml`.
- `smep --help` names PDF, Word and text as convertible → matches `main.rs`.
- Title-bar File menu and the native macOS menu carry the same four new items (Import, Export as PDF, Export as HTML, plus the separators).
- `docs/dev.md` "Import and export" section names the crates, the font policy and the CI font packages → matches `Cargo.toml` and `ci.yml`.
- Dead code: `Format::label` and `Document::suggested_path` were written and never used → removed (clippy would have flagged them; it did).

Round 1 result: 0 findings after the two removals above (made before the round started counting).

## Round 2 — mechanism and invariants

- Import: the app test drives `load_path` on a `.txt` and checks the document, its dirty state, the title (`● notes.md`), the Save As directory (the original's) and the path after saving; the drop test sends `FileDropEvent::Entered` + `Submit` and expects the unsaved-changes prompt first; an unreadable "PDF" leaves the open document untouched and shows the error dialog.
- Export: the app test presses Ctrl+Shift+E, answers the dialog, and reads the words back out of the written PDF; then exports HTML and checks the title and body; neither touches the dirty flag.
- Mutation checks (fix removed, test must fail): the Word list marker's `None => bullet` fallback → `word_lists_use_the_numbering_definition` fails (every item numbered); `$` dropped from the Typst escape set → `syntax_characters_in_text_are_escaped` fails. Both restored; the tree matches the pushed commit (`git status` clean).
- Invariant: a converted document is dirty until saved (`saved = ""`), and `named_path` (title, Save As, export dialog) is its own path or the original's with `.md`; `write_to` makes the chosen path the document's own.

Round 2 result: 0 findings.

## Round 3 — boundaries and platforms

- Boundaries in unit tests: empty text, empty PDF text layer (error), garbage bytes as PDF/DOCX (errors, no panic), empty `.docx` (empty Markdown), CRLF text, images outside the document's folder (named, not embedded), a missing local image (compile error reported, not a panic), empty Markdown export (preamble only).
- CI run 36693209479 on `76f74a5`: fmt, clippy, tests and release build green on all three. Tests: Linux 86, Windows 86, macOS 85 (`ctrl_end_and_ctrl_home_jump_across_the_document` is `cfg(not(target_os = "macos"))`, as before this phase).
- Re-reading every size and font claim against the numbers above found four stale ones:

| # | Finding | Evidence | Fix |
|---|---|---|---|
| 1 | `src/export.rs` module doc said the PDF uses "the fonts embedded in the binary for Latin text" | `cargo tree -e features`: no `typst-kit/embedded-fonts`, no `typst-assets/fonts` | doc says no font is embedded |
| 2 | This log's Choices said the same | as 1 | reworded |
| 3 | Typst's fonts were "~20 MB" (Cargo.toml, `docs/dev.md`) and "~16 MB" (this log); the 16 came from subtracting a fat-LTO build from a no-LTO one | the 17 font files total 9,683,068 bytes | ~10 MB everywhere |
| 4 | Feature costs "~11 MB" and "~30 MB" (Cargo.toml, README), and "most of it the CJK character maps", came from builds in different profiles | one-profile builds above: +6.68 MB, +33.60 MB; pdf-inspector's largest CJK table is 349 KB of source | ~7 MB and ~34 MB, CJK remark dropped |

Round 3 result: 4 findings (documentation only; no code path changed). The count restarts.

## Round 4 — code correctness (conversions read line by line)

Tests on the round's start: fmt and clippy clean in both feature configurations, `cargo test` 86 passed. Reading `convert.rs` and `export.rs` against CommonMark and Typst's syntax found four defects; each got a test first, and all five new or extended assertions failed before the fix (outputs quoted):

| # | Finding | Before | Fix |
|---|---|---|---|
| 1 | Word splits text into runs (spell checking, revisions); two italic runs side by side came out as `*foo**bar*`, which CommonMark renders with literal asterisks (rule of 3; asserted on the `markdown` crate) | `"A *foo**bar* end.\n"` | consecutive runs with the same emphasis are joined and marked once (`Styled`) |
| 2 | Word's `numId 0` (numbering switched off) became a bullet | `"- not a list\n"` | `numId 0` is a plain paragraph; a missing `numId` stays a bullet |
| 3 | Line starts that open syntax in the target were not escaped: Typst `=` (heading), `-`/`+` (lists), `2.` (numbered); Markdown `1.`/`1)` (ordered list), `===` (setext), `~~~` (fence) | `"Total\n= 5"` passed through; `escape("1. Intro") == "1. Intro"` | one line-start tracker (`convert::LineStart`) shared by both escapers, which differ only in their marker sets |
| 4 | In the PDF, a list item's soft or hard line break dropped the continuation out of the item: Typst ends an item at the first line indented no deeper than its marker | `"- one\ntwo\n"` | continuation lines indented (`indented`) for paragraphs, list items, raw-HTML text |

Scope (one occurrence or a pattern?): 3 is a pattern, both escapers had the same gap, so both were fixed through one shared tracker. 1, 2 and 4 are single places; `grep` finds no other run-by-run emphasis or unindented multi-line write.

Regression risk: escaping more is harmless where it was not needed (`\=`, `\-`, `\.` print the character in both languages; the Typst lexer accepts a backslash before any non-space character, `typst-syntax` 0.15.1 `lexer.rs` `backslash`); joining runs changes only runs that already shared emphasis; indentation only touches markup that contains a newline and sits deeper than the top level. The 86 earlier tests pass unchanged.

The PDF round-trip test now also typesets `- one\n  wrapped`, `Total\n= 5` and `2\. counted` and reads `wrapped`, `= 5` and `2. counted` back. Mutation: with `=` dropped from the Typst line-start set, it and the escaping test fail (the reader gets `Total **5**`: a heading); restored, byte-identical.

After the fixes: fmt clean; clippy clean (default and `--no-default-features`); `cargo test` 90 passed; `cargo test --no-default-features` 85 passed.

Round 4 result: 4 findings. The count restarts.

## Round 5 — reproducibility (clean 1/3)

- New test `the_same_document_exports_to_the_same_bytes`: two exports in one process are byte-identical (PDF and HTML). Across processes, the FNV-1a of the PDF printed with `--nocapture` on three separate runs: `0fd3959b5bb64844`, 23,554 bytes, all three (the system font list is re-read each run, so this is the check that matters).
- Review: `Styled`, `LineStart` and `indented` are pure (no time, randomness or hash-map order); `typst_pdf::PdfOptions::default()` writes no timestamp.
- fmt clean, clippy clean, `cargo test` × 3: 91 passed each time.

Round 5 result: 0 findings.

## Round 6 — boundaries and degenerate input (clean 2/3)

- New test `degenerate_inputs_typeset_without_error`: 18 inputs compiled by Typst for real, every one `Ok` — empty, a lone `=`, escaped `\-` `\+` `1\.`, a ten-digit `9999999999\.`, indented `  = x`, CRLF, empty list items, an orphan continuation, `=`/`-` inside a quote, a list with hard breaks and escaped markers, markers in table cells, a footnote whose text starts with `=` and `-`, every escaped symbol, inline HTML starting with `=`.
- New tests `escaping_is_linear_in_a_long_line` (both escapers): a 1 MiB line of digits then `. =` comes out ending `\. =` (the `=` is mid-line, so left alone) within the 2 s bound, debug build.
- Review: a tab before a marker is not treated as indentation by `LineStart`; unreachable — Word tabs become spaces before escaping, and CommonMark strips leading whitespace from paragraph continuation lines and after hard breaks, so a Text node never holds a newline followed by a tab.
- fmt clean; clippy clean in both configurations; `cargo test` 94 passed; `cargo test --no-default-features` 87 passed.

Round 6 result: 0 findings.

## Round 7 — static consistency (clean 3/3)

- No leftovers: `push_run` and any test-only hook are gone; every `LineStart` and `indented` use is at the intended site (`grep`).
- Size claims agree: Cargo.toml ~7 / ~34 MB, README 7 / 34 MB, the table above +6.68 / +33.60 MB; Typst fonts ~10 MB in Cargo.toml, `docs/dev.md` and this log.
- README and `docs/dev.md` describe nothing that Rounds 3–4 changed in behaviour (they name formats and crates, not run handling or escaping); this log's Choices still hold.
- fmt clean; `cargo test` 94 passed; release build 64,816,128 bytes (3,072 more than before Round 4's code); `smep --version` → `smep 0.1.2`.

Round 7 result: 0 findings. Three clean rounds in a row: accepted.

## Not verified here

- Windows window smoke (drop gesture, the export dialogs): the DirectX failure on this machine (see `phase4.md`) still blocks every gpui window today; the drop is exercised headless through `FileDropEvent::Entered` + `Submit`.
- How the PDF looks (fonts, spacing) was not eyeballed; only its text content was checked.
