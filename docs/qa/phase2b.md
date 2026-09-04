# QA log — Phase 2b: open / save / save-as, dirty title, close prompt

Scope: the commit "Add open, save, save-as, a dirty marker, and a close prompt"
(`src/io.rs`, `src/keymap.rs`, `src/app.rs`, `src/main.rs`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Baseline (before the rounds)

Compile/test findings fixed before any round started:

- `Context::spawn` takes a two-argument async closure (`E0593`).
- `Document` needed `Debug` for `unwrap_err` in a test; replaced with a `match` and derived `Debug` anyway.
- clippy `needless_lifetimes` on a test helper.
- Three dialog tests called `simulate_new_path_selection` / `simulate_path_prompt_response` before the dialog was pending; the test platform panics with `no pending new path prompt`. Reordered: act, then answer, then await.

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` | 20 passed, 0 failed | 15 headless GPUI tests + 5 unit tests |

## Round 1 — mechanism alive (Windows, real dialogs)

Driven by `SendKeys` against the debug build with a copy of a 7-line file:

| Keys | Title before → after | Effect |
|---|---|---|
| `typed ` | `save-test.md — smep` → `● save-test.md — smep` | dirty marker appears on the first keystroke |
| `typed ^s` | `save-test.md — smep` → `save-test.md — smep` | title clean again; file's first line is `typed ` |
| `more %{F4}` | `save-test.md — smep` → `● save-test.md — smep`, process still running | native Windows dialog "Save changes? save-test.md has unsaved changes." with Save / Don't Save / Cancel; the close was vetoed |

Headless tests covering the same paths without a display:

- `editing_marks_the_document_dirty_and_saving_clears_it` — dirty after an edit, clean after `Save`, file holds `after\r\n` byte for byte.
- `saving_an_untitled_document_asks_for_a_path` / `cancelling_save_as_leaves_the_document_dirty` — the new-path dialog, answered and cancelled.
- `discarding_asks_only_while_dirty` — no prompt when clean; `Cancel` → false, `Don't Save` → true and still dirty.
- `open_replaces_the_buffer_and_resets_dirty` — the open dialog, an `.html` file switching the preview format.
- `ctrl_s_reaches_the_save_action` — `ctrl-s` (`cmd-s` on macOS) through the key binding writes the file.
- `read_keeps_crlf_and_write_round_trips`, `read_rejects_invalid_utf8`, `display_name_falls_back_to_untitled`, `titles_show_the_name_and_a_dirty_marker`.

Round 1 result: 0 findings.

## Round 2 — code correctness (async paths, error paths)

- `on_window_should_close` vetoes, awaits `confirm_discard`, then removes the window through its handle. The `Save` branch awaits the save task, so a cancelled save-as keeps the window open.
- `report` shows a critical prompt and detaches a task that awaits the answer, so the receiver is never dropped early.
- `load` uses `set_value`, which emits no change event; `source` and `saved` are set by hand and the title refreshed.
- Files are read and written synchronously on the UI thread. Fine at the sizes a Markdown editor sees; noted for later (a 100 MB file would stall the UI).
- `io::write` is not atomic (no temp-file-and-rename). A crash mid-write can truncate the file. Carried forward as a known gap.
- `cargo test --locked` × 3: `20 passed` each time.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

Pending: run for the pushed commit.

## Open items carried forward

- Atomic save (write to a temp file, then rename).
- Background I/O for large files.
- No "New document" action yet; no recent-files list.
