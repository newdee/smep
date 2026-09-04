# QA log — Phase 2a: insert menu on empty lines, HTML preview

Scope: the commit "Add the insert menu on empty lines and HTML file preview"
(`src/insert.rs`, `src/app.rs`, `src/main.rs`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Baseline (before the rounds)

Compile-time findings fixed before any round started:

- `state.focus_handle()` resolved to `Focusable::focus_handle(&self, cx)` instead of the inherent accessor (two `E0061`). Switched to `Entity::focus_handle(cx)`.
- The test helper returning `(Entity<Smep>, &mut VisualTestContext)` needed an explicit lifetime (`E0106`).

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` | 10 passed, 0 failed | 8 headless GPUI tests + 2 unit tests |

## Round 1 — mechanism alive (screenshots + headless tests)

Document `plus.md` whose first line is empty; the editor starts focused on it.

| Step | Observation |
|---|---|
| open | a "+" circle sits 24 px left of the empty first line; no line numbers |
| type `/` | the menu opens directly under the line: 12 entries in 4 groups (Heading 1–3 / Bullet, Numbered, Task list / Quote, Code block, Table, Divider / Image, Link); the `/` stays in the editor and the preview |
| `/` `↓` `⏎` | line 0 becomes `# ` with the cursor after it; menu closed; focus back in the editor |

Headless tests covering the same path without a display:

- `a_slash_on_an_empty_line_opens_the_menu_and_escape_closes_it` — typed via `simulate_input("/")`; escape closes and the `/` survives.
- `a_slash_inside_text_does_not_open_the_menu` — `a` then `/` → `a/`, no menu.
- `choosing_the_first_entry_replaces_the_line` — `down enter` on the open menu → `intro\n# \nend`, cursor at byte 8, preview source updated, menu closed.
- `insert_block_keeps_crlf_line_endings` — `a\r\n\r\nb` + Heading 2 on line 1 → `a\r\n## \r\nb`.
- `multi_line_snippets_put_the_cursor_inside` — Code block → cursor on the empty middle line.
- `every_snippet_cursor_sits_on_a_char_boundary_inside_the_snippet`, `the_menu_has_no_leading_trailing_or_doubled_separators`.

Round 1 result: 0 findings.

## Round 2 — code correctness (focus, dismissal, subscriptions)

- Outside click: `PopupMenu` handles `on_mouse_down_out` itself (gpui-component `popup_menu.rs:1084`) and emits `DismissEvent`; our subscription drops the menu, so the per-render re-focus cannot trap the user. Verified by reading the source, not by a test (no mouse simulation in the harness yet).
- Dropping `_dismiss` from inside its own callback: gpui detaches subscriptions safely; the escape test exercises exactly this path.
- `open_insert_menu` before the first layout returns silently (`range_to_bounds` is `None`); the `/` simply stays. Acceptable.
- `cx.observe(&editor)` re-renders the root on every editor notify (edits, cursor moves). The root render is cheap; the preview re-parses only when the text changes.
- `cargo test --locked` × 3: `10 passed` each time.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

CI run 33833066956 for `1effe14`, all three jobs green with warm caches:

| Job | Wall time |
|---|---|
| ubuntu-latest | 1 m 27 s |
| windows-latest | 2 m 27 s |
| macos-latest | 2 m 02 s |

The 10 tests pass on all three platforms.

Round 3 result: 0 findings.

## Verdict

Three consecutive clean rounds. Phase 2a accepted.

## Open items carried forward

- Hover-only "+" (without moving the cursor) is not implemented; the "+" follows the cursor line.
- No type-to-filter in the menu; keys while it is open go to the menu, Escape returns to the editor.
- Menu labels are English only.
