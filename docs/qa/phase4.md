# QA log — Phase 4: review after 0.1.1 (bugs, large documents, Homebrew)

Scope: the commit "Keep typing smooth in large documents; fix menus, quitting and the CLI"
(`src/highlight.rs`, `src/rendered.rs`, `src/app.rs`, `src/main.rs`, `.github/workflows/release.yml`, README, `docs/dev.md`)
plus the formula in `newdee/homebrew-tap` (`Formula/smep.rb`, `scripts/bump-smep.sh`, `.github/workflows/smep.yml`).
Rule: a round counts only if it finds nothing; any finding resets the count.

## Findings (all fixed, each held by a test that fails without the fix)

| # | Finding | Evidence | Fix |
|---|---|---|---|
| 1 | Every keystroke re-parsed the whole document on the UI thread, twice per pause (highlighter, then `block_start_lines` for the scroll sync) and once more per frame in the rendered view (`block_ranges`) | release build, `markdown::to_mdast` + spans: 120 KB 65 ms, 240 KB 183 ms, 480 KB 570 ms, 966 KB 7.7 s (superlinear); measured with a temporary probe on the six documents in the table below | highlighter: documents ≤ 32 KB parse in place, larger ones on a background thread 120 ms after typing pauses, and the runs on screen follow each edit meanwhile (`shift`); preview block table computed off the UI thread; rendered view keeps its block list and updates it from the block's own text on each commit |
| 2 | Title-bar and popup menus dispatched their actions through the source editor's focus handle, which is not in the element tree in the rendered view without an active block | `FocusHandle::dispatch_action` is a no-op for a handle outside the tree (gpui-pre `window.rs:629`) | `menu_focus`: the block editor, else the rendered view's own handle, else the source editor |
| 3 | On macOS the process outlived its only window with no way to open another (gpui's default keeps a Mac app alive) | `QuitMode::Default` is `Explicit` on macOS (gpui-pre `app.rs:1912`) | `QuitMode::LastWindowClosed` everywhere |
| 4 | No `--version` / `--help`: any argument was opened as a file, so a package test had nothing to call | `smep --version` → "cannot read --version", exit 1 | both flags print and exit 0 before the window is created |
| 5 | The release stayed a draft, which a Homebrew tap cannot follow (`gh release view` sees published releases only) | v0.1.0 and v0.1.1 both draft | the workflow publishes; the tap's scheduled bump takes it from there |

Parse cost by document (release build, `spans` = the highlighter's parse, `block_ranges` = the same parse for block bounds):

| Document | Bytes | spans | block_ranges |
|---|---|---|---|
| nofn-250 (edit-test.md ×250, no footnotes) | 120,750 | 61–68 ms | 60–62 ms |
| nofn-500 | 241,500 | 183–185 ms | 178–183 ms |
| nofn-1000 | 483,000 | 561–585 ms | 598 ms–1.18 s |
| nofn-2000 | 966,000 | 7.66–7.82 s | 7.22–7.72 s |
| plain-2000 (paragraph + list ×2000) | 219,780 | 179–183 ms | 157–170 ms |
| big (edit-test.md ×2000, 2000 duplicate footnotes) | 1,048,000 | 6.9–8.4 s | 8.0 s |

The debug build is ~12× slower again (1 MB: 41–43 s). The superlinear part is inside `markdown-rs`; documents near 1 MB stay slow to preview, but typing in them no longer waits for the parse.

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` | exit 0 | clean |
| `cargo test --locked` × 3 | 63 passed, 0 failed each time (59 → 63: +1 unit `runs_follow_an_edit_until_the_next_parse`, +3 headless) | |
| `smep --version` / `--help` / missing file | `smep 0.1.2` exit 0 / usage exit 0 / "cannot read …" exit 1 | |

Mutation checks (the fix removed, the test must fail): the block-list shift (`+ 1` on moved ranges) → `block_edits_keep_the_block_list_equal_to_a_fresh_parse` fails; the highlighter deferral is asserted both ways in one test (nothing styled right after the update, styled after `DEBOUNCE`; a small document styled at once).

## Round 1 — static consistency

- README: `--version`/`--help`, the Homebrew line, the unsigned-app note under "Make it the default", the status line → each matches the code or the tap files. Keys table unchanged (no new keys).
- `docs/dev.md` Releasing: publish (not draft), tap follows within six hours or on `gh workflow run smep.yml --repo newdee/homebrew-tap`, `smep --version` contract → matches `release.yml` and the tap workflow.
- Settings keys: all three read and written; no new key.
- Constants: `PREVIEW_DEBOUNCE`, `highlight::DEBOUNCE`, `highlight::SYNC_LIMIT` each read where documented; `refresh_preview` replaced by `show_preview` + `schedule_block_table`, no leftover (clippy would flag dead code).
- The tap formula's test calls exactly what the binary prints (`smep X.Y.Z`); the bump script's two url patterns match the two release file names in `release.yml`.

Round 1 result: 0 findings.

## Round 2 — mechanism and invariants

- Highlighter: the async path is exercised headless (`a_large_document_is_highlighted_after_the_pause_not_on_the_keystroke`); `shift` covers grow-inside, clip-tail, clip-head, delete-whole, shrink, delete-all.
- Rendered view: after every commit the cached block list equals a fresh parse for: grow, split in two, empty, refill, first block, new block at the end with its gap, a source-side edit (full re-parse), from an empty document, and CRLF (`block_edits_keep_the_block_list_equal_to_a_fresh_parse`).
- Menus: `menus_dispatch_from_whatever_is_on_screen` checks the handle in all three states and that a Save dispatched the way a menu does it reaches the file in the rendered view with no block.
- Preview: `the_preview_follows_after_a_pause_in_typing` and the scroll-sync tests still pass with the block table now arriving from the background executor.

Round 2 result: 0 findings.

## Round 3 — boundaries and platforms

- Boundaries: empty document and CRLF in the block-list test; `--version` on a headless run; a missing file still exits 1 with the reason.
- CI: pending for the pushed commit.
- Tap: pending, the formula's own workflow installs and tests the release on a macOS and a Linux runner.

## Not verified here

- Windows window smoke: on this machine today every gpui window fails at start with `DXGI_ERROR_NOT_CURRENTLY_AVAILABLE (0x887A0022)` while an `OrayIddDriver` virtual display is attached (the 0.1.1 binary from yesterday fails the same way, `GPUI_DISABLE_DIRECT_COMPOSITION=1` does not help), so no screenshots this round. The environment, not the code: nothing in smep touches the renderer.
- The felt typing latency in a 500 KB document (would need the window above).
