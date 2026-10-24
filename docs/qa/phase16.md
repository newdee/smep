# Phase 16 — the page as a PDF reader shows it (2026-10-24)

The request: "感觉默认预览还是不好看，我希望是 pdf reader 那种打开后的效果，现在字的密度太高了".

## Diagnosis

- **Structural, one cause**: the text was dense because of two settings the blog had and the window did not. The text ran the whole width of the pane, up to about 1100 px, roughly 120 Latin letters a line; a reader's page or the blog column holds 60–75. The lines also sat at gpui's default line height (about 1.6) instead of the blog's 1.85. Reading mode is one `TextView` over the whole document, so the fix lives in one place: the page that holds it (`app.rs`, `theme::PAGE_WIDTH` / `PAGE_MARGIN` / `MEASURE` / `READING_LEADING`, `Palette::canvas`, `Palette::page_shadow`).
- **The look**: a page 800 px of paper wide (a 44 rem column between 48 px margins) with a soft shadow, standing on the paper a shade darker (`canvas`), text at 1.85. gpui-base's paragraphs inherit the line height from the page; headings, code and tables set their own. The page is a continuous column (its top and bottom are the pane's), as a reader in continuous-scroll mode.
- **Side effect handled**: the text's list now covers only the column, and gpui lists take the wheel only over themselves. A reader scrolls from anywhere in its window, so the wheel over the canvas or the margins is passed to the list.
- **Block view** (editing, rendered): same leading, same column. Each block's row stays full width, so a click beside the text still edits that block (see round 1).
- **Split preview**: same leading, no page; it fills its half of the window.

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | the page on screen, Paper and Paper Dark (`page.md`: CJK and English paragraphs, list, quote, Rust block, table) | the text in a centred column on a darker ground, loose leading, the board and table inside it | none |
| B (build) | mechanism: the wheel beside the column (`the_page_is_a_column_and_the_wheel_beside_it_scrolls`), the canvas darker than the paper for all 8 palettes | 125 + 5; with the forwarding removed the wheel test fails | none |
| C | behaviour kept: clicks in the block view | — | each block was first wrapped in a column-wide div, so a click in the margin beside a block reached the view and started a new block at the end. The column now sits inside each block (`rendered::column`), whose row is full width. New test `a_click_beside_a_blocks_text_edits_that_block` fails with the old layout and passes with the new one |
| 1 (clean) | static: every new constant and method has a reader (3–6 references), README / dev.md say what the code does; `cargo fmt --check` 0, clippy `--all-targets` 0 (default and slim) | 126 + 5, slim 117 + 5 | none |
| 2 (clean) | reproducibility: the suite 3 times in a row | 126 + 5 each time | none |
| 3 (clean) | the other platforms: CI run 40055735232 for `7486941`: ubuntu 6 m 22 s (127 + 4), windows 7 m 13 s (126 + 5), macos 18 m 30 s (125 + 4), 0 failed | clean | none |

The screen smoke of the block view was cut short. The screenshot showed an input method window: the user was typing on the desktop. Synthetic keys could have reached their input, so the smoke stopped there. The block view's layout is covered by the click test.

## Left

- The page has no top or bottom paper edge: it is a continuous column, as a reader in continuous mode, not paged.
- gpui breaks CJK lines before closing punctuation (a line can start with "，"); that is the text layout's, not the page's.
