# Phase 22 — Vim's `>` / `<` and search highlighting (2026-11-16)

The request: "把 > < 缩进和搜索高亮也加上".

## Design

- `>` and `<` are operators (`Operator::ShiftRight` / `ShiftLeft`) that always take whole lines: `>>` / `<<` with a count of lines, `>` / `<` with any motion or object (the lines it touches; an exclusive span ending at a line's start leaves that line out), and in Visual mode the lines selected, the count being the number of shifts (`3>`). `.` repeats the Normal-mode ones.
- One shift is `vim::SHIFT_WIDTH` = 2 columns, the editor's own Tab indent (gpui `TabSize` default), so `>>` and Tab agree. The indent is rebuilt in spaces, a tab counting as one shift width (as gpui's `TabSize::indent_count`); an empty line stays empty; `<` takes what indent there is when it is less than a shift. At most `MAX_SHIFTS` (100) widths at once. Over more than two lines the corner says `3 lines >ed 1 time` (Vim's 'report').
- The shift is one edit, trimmed to the span between the first and the last indent changed (`common_prefix` / `common_suffix`, on character boundaries), so it is one undo step and leaves the rest of the text alone.
- Highlighting: the engine holds whether the last search is highlighted (`highlighting`: set by every search motion, so `/`, `?`, `n`, `N`, `*`, `#`; cleared by `:noh` … `:nohlsearch`), and `Vim::highlight()` → `Highlight::matches(text)` lists the matches (whole words only after `*` / `#`, the same `is_whole_word` the search uses).
- The source editor shows them with gpui's `RangeDecorationCollection` (fills, amber at 35 %, painted under the selection), one collection per tab made on first use. `Smep::sync_search_marks` keeps them in step: after each Vim key, the command line closing, a tab shown, Vim mode switched, and on every change of a tab's text that has marks (the matches found again). Only the tab on screen holds marks. Considered and rejected: the editor's own search session (`set_search_query`), which paints a "current match" in the selection colour at the first match on screen, not at Vim's cursor, and cannot be moved there without stepping through every match.
- Block editors (`TextareaState`) have no range decorations in gpui-base 0.8: they search but do not highlight (README says so).

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | engine and app tests as written | 31 engine, 11 `vim_mode_*` | (1) the fuzz's CRLF check counted only typed `<CR>` as LF breaks, but `.` types an insert's text again (Enter included): a pre-existing harness bound, met only now that the alphabet (with `<` `>`) changed the random stream → the bound allows one LF per `<CR>` per `.`; (2) `Smep::load` replaces the text with `set_value`, which emits no change and clears decorations, while `marked` stayed: a file opened into the blank tab showed no highlight → `load` clears `marked`; test `vim_mode_highlights_a_file_opened_into_the_blank_tab` (共性: the only `set_value` in the app; 边界: a local reset, no structural change); (3) dev.md still listed "find" among the engine's actions (gone since Phase 21) → corrected; (4) clippy `single_range_in_vec_init` in a test; (5) the test keys parser read `<<…>` as a named key → `<` names a key only before a known name. Also: typing into a 2 MB document in the test platform costs about 12 s a key in a debug build, highlight or not (400 KB: open 46 s), so the large-document test sets the text and times `sync_search_marks` directly |
| B | mechanism: 15 mutations (highlight not set by a search; `:noh` not clearing; no sync on change / in `load` / on tab shown / on Vim switched / on the line closing / after a key; whole-word filter off; Visual count ignored; edit not trimmed; empty lines shifted; no `MAX_SHIFTS`; no report; tabs counted as one column) | 14 / 15 turn a test red | `MAX_SHIFTS` had no test → `V99999>` on one line gives 201 bytes; the mutation now fails it |
| 1 (clean) | logic and boundaries, by reading the diff: `><`, `>:`, `>/foo` + `.` (as `>n`), empty text, CRLF empty lines, exclusive spans ending at a line start or inside CRLF, `*` then `/` (different `Highlight`, recomputed), a key that both edits and changes the highlight (two syncs, the second on the change) | `cargo clean -p smep`, then 177 + 5, slim 167 + 5; fmt 0 lines | none |
| 2 (clean) | static and reproducibility: README's 7 new claims and dev.md's 6 against the code; 20 references to the new items, none unused; clippy `-D warnings` 0 (default and `--no-default-features`); the suite 3 times | 177 + 5 each time, 0 failed | none |
| 3 (clean) | the other platforms: CI | pending | |

Engine tests: 31 (2 new: shifting; highlighting until `:noh`; the fuzz alphabet and the degenerate sweep now include `<` `>` and `:noh`, the 2 MB timing `>>`, `<<`, `V9j>`). App tests: 11 `vim_mode_*` (4 new: shifting as far as Tab indents, one undo step, Visual counts, `.`; highlighting through typing, `:noh`, `n`, `*`, tabs and Vim switched off; a file opened into the blank tab; 2 MB with 187,245 matches found again in 102 ms, debug).

No screen smoke (the user is at the desktop): the highlight is checked through the decoration ranges and a draw, not by eye.
