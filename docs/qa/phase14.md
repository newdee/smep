# Phase 14 — one design for the screen, the HTML page and the PDF (2026-10-22)

The request: "the style of rendered things is too ugly in read mode, we should write beautiful for every part of markdown, such as table, title, code block, cite, image", Paper and Paper Dark first, by style alone, with a report stylesheet (markdown-it + weasyprint, h1/h2 rules, a filled table header, alternating rows, a washed quote) as the reference, not the copy; and "make the css can be modify, and then we can export to html and pdf".

## Diagnosis

- **Common, structural**: three renderings of one document had three looks. The window drew with gpui-base's defaults plus a few refinements, the HTML export carried a fixed GitHub-like stylesheet, the PDF a fixed Typst preamble; none shared a value. The fix is one design in `Palette` (`src/theme.rs`: the heading scale, weights and rules, the paragraph gap, the code scale, the table's filled header) written three ways: `Palette::text_view_style` for the window, `export::stylesheet` (CSS) for the page, `export::preamble` (Typst `set` / `show` rules) for the PDF.
- **Premise corrected**: "we only need to modify css style" holds for the page and, as a preamble, for the PDF; the window has no stylesheet. gpui-base 0.6 exposed no heading hook at all (size only), so the one thing the request names first, the title, could not be styled without an upgrade: gpui-kit 0.8 (gpui-pre 0.3.9) adds `TextViewStyle::with_heading(level -> StyleRefinement)`. The upgrade cost 8 compile errors, all in `text_view_style`. Still without a hook on screen: the quote's bar colour and wash, the rule's thickness, list markers, images; the page and the PDF do those.
- **Modifiable**: the stylesheet and the preamble are files the user can own. `smep --css [THEME]` / `smep --typst [THEME]` print the built-in ones; `export.css` / `export.typ` in the config directory replace them for every export. The Typst markup `to_typst` emits carries no colour, stroke or size (a test checks), so a replaced preamble is the whole look, not a layer under one.
- **Print**: a PDF takes the light theme of the preview's family (`PreviewTheme::light`: Paper Dark → Paper, Night → GitHub, Solarized Dark → Solarized Light), unless the user's preamble says otherwise.
- **Risk**: every caller of `to_html` / `to_pdf` / `to_typst` takes the look as an argument now (the app, `--css` / `--typst`, the tests); `Settings::dir` is `None` without a path, so tests never read the user's config directory; the block view in editing mode uses the same style, so a heading there carries its rule and the gap under it (on top of the block gap).

## Rounds

Each round = review from one perspective + the full suite (`cargo test`, Windows) and, where it is about the look, the real thing: the debug window on `look.md` (headings 1–6, inline marks, lists, a quote, a Rust block, a 3-column table, a rule, a local image, a task list, a footnote) in Paper and Paper Dark (`SMEP_CONFIG_DIR` + `preview_theme`), the exported page in headless Edge, the exported PDF's pages.

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A | the upgrade: `cargo check --tests` on gpui-kit 0.8 in a throwaway worktree | 8 errors, all `theme.rs` (the heading API) | rewrote `text_view_style` on `with_heading`; everything else compiled unchanged |
| B | mechanism in the window and the exports, first pass | 110 + 4 tests; on screen: h1 2 px rule, h2 1 px rule, framed table with an ink header, framed code with its language label, checkboxes in cinnabar; PDF (2 pages): rules, washed quote, zebra rows, ink header, framed code, page number, footnote; HTML (Edge): the same plus centred image | (1) column lines between table cells on screen: 0.8 draws them on "slack" cells in the cell's border colour, which `border_r_0` does not reach → the cell refinement's border colour is transparent; (2) the local image was missing on screen: the preview never resolved a relative path (and 0.6 had no hook; 0.8 has `image_source`) → `io::locate_image` + `rendered::image_resolver`, the document's directory as the base |
| C | the fixes in the window: Paper, Paper Dark, and the block view (Ctrl+E, rendered mode) | no column lines; the image at its natural size after the rule; block view shows the same headings and image; 111 + 4 (one new `locate_image` test, platform-aware: `/abs` is not absolute on Windows) | the new test's Windows branch, nothing in the code |
| D | the user's own files through the application (gpui test: `export.css` + a preamble with a page header, `Settings.path` in a temp config dir) | the HTML carries the user's CSS alone, the PDF's text shows the header | the test's temp dir `smep-export-look-<pid>` collided with `export.rs`'s test of the same name (one removed the other's directory mid-test, 3 of 3 runs failed together, 0 alone) → renamed |
| E | the command line (`cmd /c`, the GUI binary): `--css`, `--css paper-dark`, `--typst sepia`, `--css nope`, `--help` | 38 / 25 lines, exit 2 with the eight names, help lists both flags; `tests/command_line.rs` covers them | none |
| 1 (clean) | code: clippy `--all-targets` 0 warnings, `cargo fmt --check` 0 diffs; reproducibility: the suite 3× in a row, 112 + 4, 0 failed each time; `the_same_document_exports_to_the_same_bytes` holds with the preamble | clean | none |
| 2 (clean) | the slim build, `cargo test --no-default-features`: 103 + 5, 0 failed; `tests/command_line.rs` on the default build: 5 passed | clean | none |
| 3 (clean) | CI run 39731588052 for `e1db949`, the first build of gpui-pre 0.3.9 on the runners: ubuntu 16 m 30 s (112 + 4), macos 19 m 31 s (110 + 4), windows 31 m 18 s (112 + 5), 0 failed | clean | none |

## Left

- On screen, gpui-base 0.8 has no hook for: the quote's bar colour and wash (border colour, 2 px, muted text), the rule's thickness (1 px), list markers, image alignment and size (natural size, left; `max_w` 100 %). The page and the PDF have them. A PR upstream would close it.
- Footnotes render as plain `[^1]` text on screen (gpui-base), while the page and the PDF typeset them.
- A PDF image is always full width (`#image(width: 100%)`, as before); the page shows it at its natural size. Reading the image's size at export time would fix it.
- `cargo tree` on 0.8 lists `gpui-fast`/`wgpu` as optional dependencies that the default features do not enable (`cargo tree -i gpui-fast`: nothing).
