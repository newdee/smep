# QA log — Phase 2c: Markdown highlighting, preview debounce, scroll sync

Scope: the commit "Highlight Markdown in the editor and keep the preview in step"
(`src/highlight.rs`, `src/app.rs`, `src/main.rs`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Baseline (before the rounds)

Findings fixed before any round started:

- A test expected the CJK/CRLF strong span to end at byte 18; `粗` is three bytes, so it ends at 17. Test corrected, and it now also checks the sliced text.
- The first scroll-sync design measured content height from `range_to_bounds` of the last offset. The editor lays out only the rows on screen, so that offset answers `None`; the fraction was always 0. Replaced by a top-line search that uses the editor's real behaviour: offsets above the rendered rows all map to the first rendered line, rendered lines climb, lines below answer `None`. Binary search finds the boundary in `O(log lines)` calls.
- The sync ran in the root view's render, which precedes the editor's layout for that frame, so it saw the previous frame and stalled. It now runs in the preview pane's prepaint, after the editor laid out, and moves the preview in the same frame.
- `RopeExt` was not in scope for `lines_len` / `line_start_offset`.

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` | 30 passed, 0 failed | 17 headless GPUI tests + 13 unit tests |

## Round 1 — mechanism alive (screenshots + headless tests)

- `edge.md` (CJK, table, task list, quote, code block, footnotes): headings in the theme's `title` colour, bold in bold, italic in italic, inline and fenced code in the `string` colour, links and footnote references in `link_uri`, strikethrough dimmed. Same node classes as the preview renders.
- `long.md` (60 sections): after `{PGDN 12}` the editor's top block is "Section 55" and the preview's top item is "Section 55". Block-exact, not proportional.
- Headless: `the_preview_follows_after_a_pause_in_typing` (two edits, nothing until `advance_clock(80 ms)`, then the last text), `scrolling_the_editor_scrolls_the_preview` (400 paragraphs, `set_scroll_offset(-5000 px)` → editor top line equals the editor's own visible display row, preview item is exactly that line / 2), plus 8 pure highlighter tests (nesting, link text vs URL, byte offsets with CRLF and CJK, sorted non-overlapping runs, range coverage with defaults in gaps, block start lines).

Round 1 result: 0 findings.

## Round 2 — code correctness

- The highlighter re-parses the whole buffer on every edit (`update` gets the full rope). Fine up to a few hundred KB; a 1 MB document will feel it. Carried forward: incremental or debounced highlighting.
- Three parses per pause in typing: highlighter (per keystroke), `block_start_lines`, and the preview's own. Sharing one mdast is a later optimisation.
- `refresh_preview` resets `scroll_sync`, so a re-parse re-syncs even when the top line did not move (block boundaries may have shifted).
- The preview never drives the editor, so a reader scrolling the preview is not fought until the editor next scrolls or re-parses.
- `cargo test --locked` × 3: `30 passed` each time.

Round 2 result: 0 findings.

## Round 3 — platforms (CI), plus one finding from round 1 revisited

- `Ctrl+End` did nothing in the Windows smoke. The editor binds only `home` / `end` (line) and the macOS `cmd-up` / `cmd-down`; there is no document start/end on Windows or Linux. **Finding.** smep now binds `ctrl-home` / `ctrl-end` / `ctrl-shift-home` / `ctrl-shift-end` to the editor's `MoveToStart` / `MoveToEnd` / `SelectToStart` / `SelectToEnd` (non-macOS), with a headless test `ctrl_end_and_ctrl_home_jump_across_the_document`. Smoke rerun: `^{END}` lands the editor on "Section 60" and the preview follows to "Section 60".
- Count resets. Windows checks on the fixed tree: fmt 0, clippy 0, `31 passed`.

## Round 4 — platforms (CI)

Pending: run for the pushed commit.

## Open items carried forward

- Incremental highlighting; one shared parse.
