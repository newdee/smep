# Phase 18 — Book: the page set as a book (2026-11-14)

The request: "参照一下这个项目，看看我们有什么可借鉴的，我现在觉得预览并不够好看，https://github.com/bjarneo/quire". After a comparison (three renderings of one Chinese and English sample, headless Edge: smep's screen look, quire's values, quire's type in the blog's colours), the user chose: a new theme, Book, as the default, the blog's Paper kept; CJK text from the system's Song faces rather than a bundled CJK font.

## What quire does that we did not

1. A bundled book serif (Newsreader) where we used the system's UI sans.
2. A size up: 19 px against our 16.
3. A book's measure: 72ch, about 38 CJK characters; our medium was 56 rem, about 60.
4. Restraint: code on a flat tint, no rule under h2, semibold headings, three inks.

Not taken: quire's italic quotes (CJK has no italic; a browser slants the glyphs), its 1.62 leading (tuned for Latin; Book uses 1.78), its purple.

## Design

- `theme::Design` (Book, Blog) on every `Palette`; `PreviewTheme::Book` / `BookDark`; System follows the mode in Book. All the design's numbers live in `theme.rs` and are read by the window (`Palette::page`, `text_view_style`, `measure`), the HTML page (`export::book_stylesheet`) and the PDF (`export::book_preamble`).
- Newsreader: five static faces cut from the variable font with fontTools (opsz 18; the instancer cannot name an opsz instance, so the name and OS/2 tables are set by hand). About 600 KB in the binary. Added to gpui's text system in `theme::install` and to Typst in `to_pdf`.
- CJK: `theme::BOOK_FALLBACKS`, gpui's per-font fallback list (Noto Serif SC, Noto Serif CJK SC, Source Han Serif SC/CN, Songti SC, STSong, SimSun). A PDF leaves SimSun out (no bold).
- The measure in ems of Book's body (32 / 38 / 50), so a saved `text_width = "medium"` means a book's measure in Book and 56 rem in Blog. No settings migration: files that say `preview_theme = "system"` (the default) get Book; a file that names a theme keeps it.

## A bug found on the way (export, older than this phase)

The PDF of the sample lost a `;`: `*disappears*;` came out "disappears the". Typst reads on past a call embedded in markup into the text right after it.

- **Common, not a corner case**: every inline call the writer emits (`#emph`, `#strong`, `#strike`, `#raw`, `#link`, `#footnote`, `#image`, the alt text's `#emph`; nine places) swallowed a following `;`, and a following `.x`, `(` or `[` became a field access or more arguments (an error, or a changed document).
- **Structural**: the writer assumed the text after a call is plain markup. The calls now go through one function, `export::call`, which closes each with a `;` of its own.
- **Risk**: every inline call's markup changes (four expected strings in tests updated); block calls end at a line break and are untouched. `text_right_after_a_mark_stays_text` checks the markup and the text read back out of the PDF.

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | fmt, clippy | — | `cargo fmt --check` 115 lines; clippy 2 (a needless `.into()` in a test) → fixed |
| B | mechanism: five mutations (no closing `;`; fallbacks on Blog instead of Book; Book measured in rems; System following Paper; Book body not scaled) | each turns its test red (5 / 5); review of the diff: the block editor's textarea sets its own size and inherits the serif, as the Georgia themes already do | none (clean 1 of 3) |
| C | static: claims against code | — | `TextWidth`'s doc still said "44 rem, the blog's column"; Book's CSS and Typst had no test of their own → doc fixed, `book_exports_as_a_books_page` |
| D | static again: fmt 0, clippy 0 (default and slim) | 129 + 5, slim 119 + 5 | none (clean 1 of 3) |
| E | reproducibility | — | the same-bytes test checked Paper's PDF only; Book's carries the bundled fonts → the test covers both designs (and CJK text); 3 processes: Paper `9b681d4e3ce5dd69`, Book `fe7ac582daf4b765` |
| 1 (clean) | reproducibility: fmt 0, clippy 0, the suite 3 times | 129 + 5 each time, both hashes the same 3 times | none |
| 2 (clean) | degenerate inputs: the 24 degenerate documents under Book and Book Dark, and 6 "text right after a mark" inputs, typeset (a temporary probe, removed) | 54 / 54 typeset | none |
| 3 (clean) | the other platforms: CI run 43558378183 for `4d8afff`: ubuntu 14 m 30 s (130 + 4), macos 22 m 50 s (128 + 4), windows 32 m 26 s (129 + 5), 0 failed | clean | none |

## Fonts against the real text system

The test platform's text system resolves any font name (every weight came back as the same id, which first looked like a SemiBold/Bold clash), so a test of the installed fonts proves nothing there. Checked once instead from a throwaway example with a hidden window on DirectWrite (Windows 11):

- Newsreader 400, 600, 700, 400 italic, 700 italic resolve to five faces, the `m` advancing 79.5 / 82.9 / 84.6 / 72.3 / 82.4 px at 100 px.
- "Book 中文排版" with the fallback list: the CJK run is a face with a 392 × 286 bounding box (Noto Serif SC); with SimSun alone, 100 × 100 (SimSun); with no list, the system's sans (YaHei, a bolder face at 600).

In the tests: the font files are read back (typographic family "Newsreader", OS/2 weights 400 / 400i / 600 / 700 / 700i, no `fvar`), and the exported PDF carries Newsreader.

No screen smoke: the user is at the desktop. The HTML page (light and dark) and the PDF were looked at as images.
