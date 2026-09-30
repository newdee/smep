# QA log — Phase 5: import (PDF, Word, HTML, text) and export (PDF, HTML)

Scope: commits "Import PDF, Word, HTML and text files as Markdown" (`src/convert.rs`, `src/io.rs`, `src/app.rs`, `src/main.rs`, `src/keymap.rs`)
and "Export the document as a PDF or an HTML page" (`src/export.rs`, `src/app.rs`, `src/keymap.rs`, `Cargo.toml`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Choices

- PDF text comes from `pdf-inspector` (pure Rust by default: `lopdf` + `ttf-parser`; the same crate magpie uses), through its per-page Markdown. Its OCR features pull in pdfium and ONNX (C++), so they stay off: a page without a text layer is marked in the output, a PDF with none is an error.
- Word through `docx-rust` (pure Rust): headings from paragraph styles, lists from the numbering definition (bullet vs numbered, level), bold/italic from run properties, links from the relationships part, tables as GFM tables. Images are dropped.
- HTML through `htmd`; text as is (line endings and blank runs normalised).
- PDF export through Typst (`typst-as-lib` + `typst-pdf`, pure Rust): the Markdown syntax tree is rewritten as Typst markup and compiled with the system's fonts plus Typst's embedded Latin fonts. Chosen over `printpdf`/`genpdf` because they leave line breaking, pagination and tables to the caller; Typst does the typesetting. Cost: binary size, below.
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
| + import (pdf-inspector with its CJK CMaps, docx-rust, htmd) | 33,068,544 |
| + export (Typst with embedded fonts) | pending |

## Round 1 — static consistency

Pending.

## Round 2 — mechanism and invariants

Pending.

## Round 3 — boundaries and platforms

Pending.

## Not verified here

- Windows window smoke (drop gesture, the export dialogs): the DirectX failure on this machine (see `phase4.md`) still blocks every gpui window today; the drop is exercised headless through `FileDropEvent::Entered` + `Submit`.
- How the PDF looks (fonts, spacing) was not eyeballed; only its text content was checked.
