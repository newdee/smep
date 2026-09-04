# QA log — Phase 3a: frameless window, title-bar menus, view modes, full screen

Scope: the commit "Draw the window frame: title-bar menus, view modes, full screen"
(`src/app.rs`, `src/keymap.rs`, `src/settings.rs`, `src/main.rs`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Baseline (before the rounds)

Findings fixed before any round started:

- `render_title_bar` / `render_menu_bar` returned `impl IntoElement`, which in edition 2024 captures the `cx` borrow and blocked the later `cx.listener` calls (seven `E0502`). They return `AnyElement`.
- In the rendered view nothing was focused, so no shortcut reached the root (the Windows smoke showed `menu_bar = true` after Ctrl+Shift+M). The root now tracks its own `FocusHandle`, focused whenever the rendered view is entered.
- Two tests built `Settings` by hand or asserted the whole file; updated for the two new keys.

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` × 3 | 48 passed, 0 failed each time | +2 headless (`view_modes_switch_by_keyboard_and_are_remembered`, `the_menu_bar_and_fullscreen_toggle_by_keyboard`), +1 unit (older settings files still load) |

## Round 1 — mechanism (Windows screenshots)

| Gesture | Observation |
|---|---|
| launch | frameless window; title bar holds File / Edit / View on the left, `edge.md` centred, minimise / maximise / close on the right; split view below |
| click "View" | dropdown: Source Ctrl+1, ✓ Split Ctrl+2, Rendered Ctrl+3, Preview Theme ▸, ✓ Menu Bar Ctrl+Shift+M, Full Screen F11 (shortcuts rendered from the bindings) |
| Ctrl+3, then Ctrl+Shift+M | full-width rendered view; title bar shows only the document name and the controls; `settings.toml` has `view_mode = "rendered"` and `menu_bar = false` |

Headless: the view-mode test drives Ctrl+1/3/2 and checks the file after each; the toggle test flips the menu bar twice and full screen twice (`window.is_fullscreen()` on the test platform).

Round 1 result: 0 findings.

## Round 2 — code correctness

- Every title-bar menu sets `action_context` to the editor's focus handle, so Undo / Cut / Find etc. reach the editor even though the menu lives in the title bar.
- `Quit` goes through the same unsaved-changes prompt as closing; only then `cx.quit()`.
- macOS gets the same menus natively through `cx.set_menus`; on Windows and Linux that call is a no-op and the title-bar menus are the only ones.
- Linux requests client-side decorations so the drawn title bar is the only one; the component skips its window controls when the compositor keeps server decorations.
- The rendered view is read-only in this phase; editing there is Phase 3b.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

CI run 33855336384 for `9e71175`, all three jobs green: ubuntu 1 m 17 s, windows 2 m 36 s, macos 2 m 04 s. The 48 tests pass everywhere.

Round 3 result: 0 findings.

## Not verified here

- The macOS native menu bar and traffic-light spacing (no Mac here).
- Linux client-side decorations (WSL still lacks the build packages).
