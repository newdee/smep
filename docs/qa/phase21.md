# Phase 21 — Vim's command line: `:w`, `:q`, `/`, `?`, `n`, `N` (2026-11-16)

The request: "把 :w :q 和 n/N 搜索也加上".

## Design

- The command line is a real single-line `InputState` along the foot of the pane (`Smep::vim_prompt`), not keys collected by the engine: a CJK input method composes before gpui sees a key, so only a real input lets `/中文` be typed (and pasted).
- The engine answers `Op::Prompt` and keeps the command half typed (`d/` waits for its search, a count stays); `Vim::submit` runs the line, `Vim::cancel` drops it. While the line has the focus, the interceptor takes Enter, Escape (Ctrl+[) and Backspace on an empty line, and lets every other key type.
- Search: literal text, case for case (Vim's default case rule; no patterns), forward / backward, wrapping round with Vim's message; `n` / `N`, `*` / `#` (the word under the cursor, whole words), `/` + Enter for the last text; a motion like any other, so `d/foo`, `c?x`, `v/x` and counts work; `.` repeats `d/foo` as `dn`.
- Ex: `:w`, `:q` (E37 while there are unsaved changes), `:q!`, `:wq`, `:x` (writes only when modified), `:12`, `:$`; anything else E492. `:w` and `:wq` go through `save_task` (a Save As dialog for an untitled document), `:q` through `remove_tab`; on the last tab `:q` removes the window, and smep quits (`QuitMode::LastWindowClosed`).
- Messages (`Vim::message`) show in the pane's corner until the next keystroke, whoever takes it.
- The line closes unrun on a click elsewhere in the window (a blur while the window is active) or a tab switch / leaving the editor (`show_active`); another application coming to the front leaves it open. `/` no longer opens the editor's find bar (Ctrl+F still does).

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | engine tests | 29 engine tests: 3 red | all 3 wrong expectations, checked against Vim: `?x` from the first match wraps to the last, then `N` goes on; `v/x` + `d` takes the `x` too (Visual mode is inclusive); `d/b w .` deletes up to the next `b`. Also: `cargo build` called the crate fresh after edits (no recompile, so a removed `Op::Find` still "built"); `cargo clean -p smep` and every later round builds from clean |
| B | logic and error paths, by reading: 25 cases (Ctrl+S / Ctrl+Tab / a click with the line open, Save As cancelled under `:wq`, a failed write, Enter while an input method composes, counts before `/`, `.` after `d/`, `:0`, a huge `:N`) | 170 + 5 | none |
| C | mechanism: eight mutations of the wiring (blur ignored; tab switch keeps the line; Enter not taken; Backspace-on-empty not taken; E37 off; last tab keeps the window; `cancel` not called; focus not returned) | 5 / 8 turn a test red | the blur, the tab-switch close and `cancel` had no test → `vim_mode_command_line_closes_when_left`. Writing it showed focus events reach an active window only (the test activates it), and that a window losing activation blurs the line too, which closed it when the user switched applications: a blur now closes it only while the window is active, and the test checks both (open after deactivation; closed by leaving the editor then). All eight mutations now fail a test |
| D | static: docs against code | 171 + 5 | README said messages last until the next key, but keys Vim lets through (Insert-mode typing, shortcuts) left them → any keystroke clears them (`Vim::clear_message` in the interceptor; the ex test checks a shortcut clears one; the mutation fails it); dev.md still said any blur closes the line → corrected |
| 1 (clean) | static: fmt 0 lines; clippy `-D warnings` 0 (default and `--no-default-features`); README's 8 new claims and dev.md against the code | 171 + 5, slim 161 + 5 | none |
| 2 (clean) | reproducibility: the suite 3 times | 171 + 5 each time, 0 failed | none |
| 3 | the other platforms: CI | pending | |

Engine tests: 29 (3 new: search, wrap messages, Ex commands; the fuzz alphabet and the degenerate sweep now include `/ ? n N * # :`). App tests: 7 `vim_mode_*` (3 new: searching through the command line incl. text set as an input method would; closing it when left; `:w`, `:q`, `:q!`, `:wq` on real files, the last one closing the window).

The test platform types no `!` for Shift+1 (as no `$` for Shift+4), so `:q!` is set into the line as text.

No screen smoke (the user is at the desktop): the command line is checked by `debug_bounds` and focus only.
