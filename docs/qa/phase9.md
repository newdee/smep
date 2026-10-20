# QA log — Phase 9: opens reading; Edit / Read switches

Scope: the commit "Open in reading mode; Edit (Ctrl+E or the title-bar button) brings the editor"
(`src/app.rs`, `src/rendered.rs`, `src/keymap.rs`, README).
Rule: a round counts only if it finds nothing; any finding resets the count.

## What changed

- Every launch starts reading: the rendered document, read-only (a click on a block does not
  open it, no "+" button, no caret). The rendered view holds the focus so the shortcuts work.
- Editing is session state (`Smep.editing`), never written to `settings.toml`; the view mode
  stays the remembered setting. `layout()` is what the window shows: the view mode while editing,
  rendered while reading.
- Ways in and out: Ctrl+E / Cmd+E (`ToggleEditing`), the Edit / Read button at the right of the
  title bar, "Editing" (checked) in the View menu and "Edit / Read" in the native macOS menu; a
  view-mode key or menu item (Source / Split / Rendered) while reading starts editing in that
  mode. Going back to reading ends an edit in progress and closes the insert menu.
- `RenderedView` gained `read_only`: `activate` is a no-op, blocks lose the text cursor, the
  empty-document hint says to use Edit.

## Finding before the rounds

The title-bar button hovered but never toggled (two synthetic-click runs, a View-menu click with
the same click sequence opened the menu). Cause: gpui-kit's `TitleBar` sets `should_move` on
mouse down and calls `window.start_window_move()` on the next mouse move; Windows then takes the
mouse and the mouse up that completes a click never reaches the button. The menu buttons do not
suffer because the dropdown opens from the press. Fix: the button's wrapper stops propagation of
the left mouse down, so the title bar never arms the move. Rounds restarted after it.

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` (default and `--no-default-features`) | clean, clean | |
| `cargo test --locked` × 3 | 98 + 4 passed, 0 failed each time (97 → 98: +1 reading-mode test) | |
| `cargo test --locked --no-default-features` | 91 + 4 passed | |

## Round 1 — mechanism (Windows, real window, synthetic clicks)

`smep paper.md`, the debug build, screenshots in the session scratchpad:

| Step | Seen |
|---|---|
| launch | rendered document, button reads "Edit", View menu shows "Editing" unchecked with Ctrl+E (`reading-mode.png`, `click2-1.png`) |
| click the paragraph while reading | nothing changes (`block-1.png`) |
| click Edit | button reads "Read" (`click3-1.png`) |
| click the paragraph while editing | the paragraph becomes its source `这是一段**粗体**、*斜体*、…` with the caret at its end (`block-3.png`) |
| click Read | button reads "Edit", document rendered again (`click3-2.png`) |

The unit test `opens_reading_and_a_key_or_the_view_menu_starts_editing`: starts reading with the
rendered view focused; `activate` does nothing; Ctrl+E gives the remembered split view with the
source editor focused; Ctrl+E from a block edit ends it and reads again; Ctrl+1 while reading
starts editing in source view; `settings.toml` carries `view_mode = "source"` and no `editing`.

Ctrl+E sent with `SendKeys` did not reach the window in the smoke script (the same as Ctrl+3
earlier; a SendKeys/IME quirk); the unit test covers the key.

Round 1 result: 0 findings.

## Round 2 — static consistency and invariants

- README: the feature bullet and the two key-table rows name the button, Ctrl+E / Cmd+E and that
  a view-mode key starts editing; `docs/dev.md` needs nothing.
- `is_editing` / `is_read_only` are `#[cfg(test)]`: no dead code in the binary (clippy `-D warnings`).
- Every path that changes `editing` also sets the rendered view's `read_only` and the focus:
  `set_editing(false)`, `set_view_mode` (the only way to `editing = true`), the constructor.
- `active_editor`, `menu_focus` and `render` all go through `layout()`, so while reading the "+"
  button, the slash menu and the menus' action context behave as in rendered mode without a block.
- Existing tests exercise editing through `open_with`, which turns editing on after construction;
  the reading-mode test uses the raw constructor (`open_reading`).

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

(filled in from the CI run)
