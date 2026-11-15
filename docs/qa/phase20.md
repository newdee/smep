# Phase 20 — Vim mode (2026-11-16)

The request: "给smep加上vim模式把，做完了之后再发版".

## Design

- `src/vim.rs`, pure: `Vim::key(key, text, cursor, selection)` answers `Pass` or `Handled(ops)` (edits, cursor moves, a selection, undo / redo / page / find). It never holds the text, so one engine drives the source editor and a rendered-view block editor alike.
- Modes: Normal, Insert, Visual, Visual Line. Counts, operators `d c y` with motions and objects, `dd cc yy D C Y x X s S p P r J ~ u Ctrl+R .`, motions `h j k l w b e W B E 0 ^ $ gg G { } % f F t T ; ,`, Enter / Backspace / Space, objects `iw aw i" a" i' a' i( a( i[ i{ i<`, the arrows / Home / End as `h j k l 0 $` outside Insert mode, `/` (find bar), Ctrl+D / Ctrl+U.
- The keys reach Vim first: a keystroke interceptor (`cx.intercept_keystrokes`) in `Smep::new`. A taken key stops propagation, which also keeps its character out of the text (Windows: no `TranslateMessage`, so no `WM_CHAR`; macOS: no `insertText:`; the test platform: no input handler). Checked in gpui-pre 0.3.9's sources before building on it.
- Edits go through `set_selected_range` + `replace` (an undo step each); undo / redo / page / find through the editor's own focus handle (`FocusHandle::dispatch_action`, synchronous). After an undo the restored selection is collapsed, or the next key would read it as a mouse selection.
- Normal mode's block cursor is an overlay at `range_to_bounds` of the character under the cursor (half a square at a line's end); the mode and the keys of a pending command sit in the pane's corner. `Settings::vim_mode` (off by default), Edit, Vim Mode (window menu, checked; macOS menu).
- Line breaks: a line ends at `\n` or at the `\r` of `\r\n`, so CRLF documents stay CRLF (`line_end`, `after_break`, `break_before`, `newline`).
- Kept out, and said so in the README: `:` commands, `/` with `n` / `N`, marks, macros, named registers, `>` / `<`, the system clipboard for yanks, `.` for Visual-mode changes. A CJK input method in native mode composes before gpui sees the key, so in Normal mode it should be switched to English.

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | engine tests first, then the app | 22 engine tests: 7 red | all 7 were wrong expectations (checked against Vim: `d2w` at a line's end leaves the cursor on the last blank, `vlo` then `h` selects `bcd`, `J` onto an empty line, `F,,`, `da(` at `f(…)`, `~.`) or a platform quirk (the test platform does not shift `4` to `$`) → expectations fixed |
| B | boundaries: CRLF documents | — | the engine took a line break to be `\n` in 9 places (`line_end`, `down_lines`, `o` / `O`, linewise delete / yank / put, `J`, `}`, `V`'s selection, `w` / `b` at an empty line): `A` typed between `\r` and `\n`, `dd` left a lone `\r`. Common and structural → shared helpers (`line_end` stops at `\r`, `after_break`, `break_before`, `newline`, `char_end`); `crlf_line_breaks_stay_whole` (19 cases) and a line-break invariant over the degenerate sweep (11 texts incl. 4 CRLF × 37 commands × every start). The invariant then caught a tenth place: `dw` across lines used `target_line - 1` → `break_before`. LF behaviour unchanged: the 22 earlier tests pass as they were |
| C | static: docs against code | 162 + 5 | the README said `/` opens the find bar (block editors have none) → "the source editor's"; the arrows passed to the editor, which collapsed a Visual selection (ending Visual mode silently) and let Normal mode's cursor past a line's end → plain arrows / Home / End are motions outside Insert mode, Shift / Alt arrows still the editor's (`the_arrows_are_motions_outside_insert_mode`); `simulate` was `pub` for no reader → private |
| D | mechanism: six mutations of the wiring (no `stop_propagation`; no collapse after undo; no `retarget`; no reset on Write; no reset on toggle; `vim_editor` never updated) | 1 / 6 turn a test red | four mechanisms had no test. A probe showed the editor's undo does select what it restores (4..8 after undoing `dw`), so the collapse is needed → `vim_mode_resets_where_it_should_and_undo_leaves_a_cursor` (undo then `x`; Insert → Read → Write; `d` in the source then `x` in a block), and the toggle test now leaves from Insert mode. All six mutations now fail a test |
| E | logic and invariants: every entry (source editor, block editor, menus open, reading, find bar focused) and every op batch ending on a character | — | code: none. Docs: the README said `.` repeats "the last change", but Visual-mode changes are not recorded → said so; `xvld.` pins it |
| F | correctness and size: 2000 random 16-key sequences over 5 texts (prose, CJK, CRLF with brackets and quotes, empty, one character; fixed seed); the slowest key on a 2 MB document | worst key 2.6 ms (debug), 0.89 ms (release) | the fuzz caught the test editor's Backspace splitting a CRLF (an Insert-mode key the test editor stands in for); a probe showed the real editor deletes `\r\n` as one (`"a\r\nb"` → `"ab"`) → the test editor does too. Not a product bug, but a finding: the round does not count |
| G | static again: `cargo fmt --check` | — | 28 lines unformatted (the last test edits) → formatted |
| 1 (clean) | static: fmt 0 lines; clippy `-D warnings` 0 (default and `--no-default-features`); README's 22 claims against the code; every `Op` and `Key` variant built somewhere (no dead-code warning) | 165 + 5, slim 155 + 5 | none |
| 2 (clean) | reproducibility: the suite 3 times (the fuzz has a fixed seed, so the same 2000 sequences each time) | 165 + 5 each time, 0 failed | none |
| 3 | the other platforms: CI | pending | |

Engine tests (`vim.rs`): 26, among them the degenerate sweep (11 texts × 37 commands × every start position), the fuzz, the 2 MB timing. App tests: 4 (`vim_mode_*`), through `simulate_keystrokes`, so through the interceptor and the editor.

No screen smoke (the user is at the desktop): the block cursor and the corner label are checked by `debug_bounds` only.
