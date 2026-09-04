# QA log — Phase 3b: editing in the rendered view

Scope: the commit "Edit blocks in place in the rendered view"
(`src/rendered.rs` new; `src/app.rs`, `src/insert.rs`, `src/highlight.rs`, `src/main.rs`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Baseline (findings fixed before the rounds started)

Each of these came out of a Windows smoke run or a review pass and is now held by a test that was seen to fail without the fix.

| Finding | Cause | Fix | Test |
|---|---|---|---|
| A multi-line block showed as ~1.5 rows when clicked | the block editor was a code editor, whose layout never sizes to its rows (only the auto-grow layout does) | block editors are `TextareaState` with `auto_grow(1, MAX)`; the insert helpers became generic over `InputBaseState<M: MultiLineMode>` and the app addresses "the editor being typed in" through an `ActiveEditor` enum | `a_block_editor_grows_with_its_text` (height ≥ 4 × line height for a 4-line list) |
| A click below the blocks made a new block that could not be typed into | the view's own focus-on-click ran after the handler and took focus back from the new editor | `window.prevent_default()` in the gap handler | `a_click_below_the_blocks_starts_a_focused_block_at_the_end` (fails without it: "the new block keeps focus…") |
| Clicking a list with a nested list put the cursor on an empty last line | the parser's list range includes the blank line after a nested list | `block_ranges` trims trailing line breaks | `a_list_with_a_nested_list_does_not_keep_the_blank_line_after_it` |
| Deleting all of a middle block made its editor vanish while still focused | the editor was placed only where a block started inside its range, or at the end | placed by position: before the first block after its range | `an_emptied_block_stays_on_screen_and_takes_typing` (fails with the old placement: typed "n" lost) |
| Edits made elsewhere left the block's offsets stale, so its next commit would land on the wrong range | nothing checked the invariant "the document holds the editor's text at [start, start+len)" | on every source change the view checks it and ends the edit if it fails; `load` ends it explicitly (`set_value` emits no event) | `a_change_from_elsewhere_ends_the_block_edit` |
| No way to finish editing without clicking elsewhere | — | Escape (the text input lets it through) ends the edit and focuses the view | `escape_ends_the_block_edit_and_keeps_the_text` |

Also: the title-bar menus and the popup menus now dispatch their actions to the editor being typed in (a block editor in the rendered view), and `Smep` observes the rendered view so the menus are rebuilt when that changes.

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` × 3 | 59 passed, 0 failed each time (48 → 59: +9 headless, +2 unit) | |

## Round 1 — mechanism (Windows, real window, IME in English mode)

`smep edit-smoke.md` with `view_mode = "rendered"`; keys sent with SendKeys, screenshots after each step (`r3-*.png`).

| Gesture | Observation |
|---|---|
| click below the last block, Backspace, type `123` | a new block at the end with the placeholder gone and `123` in it; title turns `● edit-smoke.md` |
| Escape | `123` renders as a paragraph at the end; no editor on screen |
| click the task list | the whole 3-line list shows as source (`- [x] 已完成 / - [ ] 未完成 /   - 嵌套项`), cursor at the end of the last line |
| End, Backspace | last line reads `  - 嵌套` |
| Escape, Ctrl+S | list renders with `嵌套`; title loses `●`; file diff +3 −1: the `项` gone, `\n\n123` appended after the footnote definition |

An earlier run (before the baseline fixes) typed with the Chinese IME active, which swallowed the letters into a candidate window; that was the test setup, not the app.

Round 1 result: 0 findings.

## Round 2 — logic and invariants (review of `rendered.rs`)

- Commit replaces exactly `[start, start + committed_len)` with the editor's text; a first commit into a block at the end after text inserts the missing blank line (`\n\n` or `\r\n\r\n` by the document's own line ending) and moves `start` past it. Tests cover LF, CRLF, and that the gap is added once.
- The editor's text may parse into several blocks (a paragraph split in two) or into none (emptied); both keep exactly one editor in the tree, at the document position of its range.
- `active_matches_document` is the invariant; it is checked on every source change, so any path that edits the document without going through the block ends the edit rather than corrupting it. `load` is the one path that changes the text with no event and calls `deactivate` itself.
- Focus: activating focuses the block; `deactivate` refocuses the view; `set_view_mode(Rendered)` focuses the active block if there is one, else the view. Shortcuts bound on the root reach the block editor (Ctrl+S from a block editor saves: `saving_from_a_block_editor_writes_the_whole_document`).
- A `/` alone on a line of a block opens the insert menu positioned at that line, and the snippet lands in the block (`a_slash_in_a_block_opens_the_menu_and_inserts_there`).

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

CI run 33857480016 for `78e0b42`, all three jobs green: ubuntu 1 m 34 s, windows 2 m 56 s, macos 1 m 25 s. The 59 tests pass everywhere, the mouse-driven and key-driven ones included.

Round 3 result: 0 findings.

## Not verified here

- Rendering of footnotes and reference links whose definition sits in another block: each block renders on its own, so the reference cannot resolve. Known gap, carried over.
- Arrow keys do not move between blocks; Escape or a click does.
