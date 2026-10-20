# QA log — Phase 6: the Paper look (keepane's paper / paper-dark)

Scope: the commit "Wear keepane's Paper look: ink on paper, light or dark with the system"
(`assets/themes/paper.json`, `src/theme.rs`, `src/main.rs`, `src/app.rs`, `Cargo.toml`, README).
Rule: a round counts only if it finds nothing; any finding resets the count.

## What changed

- The application (editor, title bar, menus, popups) is drawn in Paper: a gpui-kit theme set with two configs, `Paper` (light) and `Paper Dark`, copied from keepane's `themes/paper.conf` and `paper-dark.conf`: cream `#f0e7d2` / warm near-black `#171716` paper, ink `#33302a` / `#cecabe`, the bar's darker edge for the title bar (`#e3d8bf` / `#121212`), warm grey muted inks, Vitesse hues for the base colours, and one accent, cinnabar (`#a53736` / `#dd625e`), for the caret, headings, focus ring and primary buttons. Links are the Vitesse blue (`#035fa0` / `#4f95cf`).
- `theme::install` loads the set into gpui-kit's `ThemeRegistry` and makes the two configs the application's light and dark themes; `Theme::sync_system_appearance` (already called on start and on every appearance change) then follows the system.
- The preview's "System" theme (the default) therefore shows Paper too; `Paper` and `Paper Dark` were added as fixed preview themes for pinning one regardless of the system.

## Findings (fixed before the rounds)

| Finding | Evidence | Fix |
|---|---|---|
| `target/debug/smep.exe` was 17 days older than the sources and `cargo build` still reported it fresh, so the first screenshots showed gpui-kit's default theme | exe 2026-10-03 vs theme.rs 2026-10-20; `Finished … in 0.82s` with no `Compiling smep` | `cargo clean -p smep` (8,014 files) and rebuild; screenshots retaken |
| Four colour keys in the theme file were not names the schema knows (`link.foreground`, `link.active.foreground`, `link.hover.foreground`, `drag_border`): the deserializer ignores unknown keys, so links fell back to `primary` (cinnabar) | pixel sample of the preview link: `#a53736` ×109 (light), `#dd625e` ×109 (dark); gpui-kit's own `default-theme.json` carries the same misspellings | keys renamed to `link`, `link.active`, `link.hover`, `drag.border`; a test now round-trips every key in the file through the parsed colours and fails on one the schema does not know (checked: the misspelt `link.foreground` makes it fail) |

## Baseline

| Check | Windows (local) | Result |
|---|---|---|
| `cargo fmt --all --check` | exit 0 | clean |
| `cargo clippy --all-targets --locked -- -D warnings` (default and `--no-default-features`) | clean, clean | |
| `cargo test --locked` × 3 | 96 passed, 0 failed each time (94 → 96: +1 unit on the theme file, +1 headless on the installed theme) | |
| `cargo test --locked --no-default-features` | 89 passed | |

## Round 1 — mechanism (Windows, real window)

`smep paper.md` in split view, the debug build after the rebuild above; screenshots `paper-dark-split.png` (Windows in dark mode) and `paper-light-split.png` (Windows switched to light apps for the shot, then restored).

| Pixel sample (41×21 px over the preview's link) | Light | Dark |
|---|---|---|
| paper (most common) | `#f0e7d2` ×536 | `#171716` ×536 |
| link | `#035fa0` ×109 | `#4f95cf` ×109 |

Seen in both: the editor's `#`/`##` heading lines in cinnabar, code in green, the link URL muted italic, the title bar a shade darker than the page, the preview's code block on the bar colour, table borders quiet.

Round 1 result: 0 findings (after the two fixes above, which came out of this round's first pass).

## Round 2 — static consistency and invariants

- The headless test installs the theme and checks, in both modes, the page, ink, title bar and caret colours and the heading colour of the highlight theme (`paper_is_the_application_theme_in_both_modes`).
- Every colour key in the file is one the schema accepts (test above); every value is a 7-character hex colour (checked for `background`; the file uses no named colours).
- README: the Paper bullet and the preview-theme list match `theme.rs` (`ALL` has nine entries, labels as listed).
- `Cargo.toml` `include` carries `assets/themes/*`, so `cargo publish` ships the file `include_str!` needs.
- The right-click theme test moved from GitHub to Paper as the second menu entry, which is what the new order makes it.

Round 2 result: 0 findings.

## Round 3 — platforms (CI)

The run for `4f0f3bf` itself (39423687805) was cancelled by the next push's concurrency rule; the run that covers it is 39427813145 for `9ca565e` (theme + default-app menu + the macOS build fix), all three jobs green: ubuntu 4 m 46 s (warm cache), macos 24 m 35 s, windows 36 m 51 s; 97 tests each.

Round 3 result: 0 findings.

## Not verified here

- macOS and Linux title-bar rendering with the new colours (CI compiles and runs the headless tests only).
- The rendered view's screenshot: the Ctrl+3 keystroke in the smoke script did not reach the window (the shot shows split view); the rendered view reads the same `cx.theme()` colours the tests check.
