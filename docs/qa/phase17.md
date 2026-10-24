# Phase 17 — the paper fills the pane, the width is yours, the outline folds (2026-10-24)

The request: "为什么滚动条不在最右边，而是在中间，另外为什么文字没有扑满内容区域，outline 默认显示，可以折叠". Asked back, the user chose: the paper fills the pane and the column width is adjustable (narrow / medium / wide / full, medium by default); "folds" means the whole panel folds away.

## Diagnosis

- **Scrollbar in the middle, structural**: gpui-base draws a scrollable `TextView`'s scrollbar at the right edge of the view. Phase 16 narrowed the whole view to the 800 px page, so the scrollbar stood at the page's edge. The fix narrows the text, not the view: the `TextView` spans the pane and its own horizontal padding (`Smep::reading_margin`) centres the column. The scrollbar overlay is absolutely positioned over the padded box, so it sits at the pane's edge.
- **"Why doesn't the text fill the area"**: Phase 16 did that on purpose, for the earlier complaint of dense text (about 120 letters a line at full width; a reader's page holds 60–75). The two asks pull in opposite directions, so the user chose: the width is a setting, `TextWidth` (44 / 56 / 72 rem, or the pane less 48 px either side), medium by default, under View → Text Width (also in the macOS menu bar). The darker canvas and the page shadow went with the narrow view.
- **Outline shown by default and foldable**: the default is now shown. A fold (‹) in its head, the Outline button or Ctrl+Shift+L puts it away to an 18 px strip (›) that unfolds it.
- **Settings compatibility, structural** (round D): 0.1.12 and 0.1.13 save the whole settings struct, so any saved file says `outline = false` whether or not the user chose that. A default of "shown" under that key would never reach those users. The setting is now a new key, `outline_folded`, and the old key is ignored.
- **Risk**: every margin computation reads the pane as the window less the outline (or its strip). The block view takes its column from the same setting (`RenderedView::set_measure`). The split's preview still fills its pane.

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | mechanism: `the_text_runs_in_a_column_and_the_wheel_beside_it_scrolls`, which checks the medium column's width, that it is centred (left margin = right margin, so the list and its scrollbar reach the pane's edge), the wheel beside it, full width at the least margin, and narrow with the outline folded to its strip; the outline test (shown at start, key folds and unfolds, a click on the strip unfolds) | 125 + 5 | none in the code; two mutations (no centring padding; no click on the strip) each turn their test red |
| B | static: fmt, clippy, names | — | `cargo fmt --check` showed 12 lines (fmt not rerun after the last edits) → formatted |
| C | static again: clippy `--all-targets` 0 (default and slim), no stale `MEASURE` / `PAGE_WIDTH` / `canvas` left in code or docs | 125 + 5, slim 116 + 5 | none, but the next round found one |
| D | invariants: old settings files | — | 0.1.12 / 0.1.13 wrote `outline = false` into every saved file → a new key, `outline_folded`; the old key is ignored (test) |
| 1 (clean) | static: `cargo fmt --check` 0, clippy 0 (default and slim) | 125 + 5, slim 116 + 5 | none |
| 2 (clean) | reproducibility: the suite 3 times in a row | 125 + 5 each time | none |
| 3 | the other platforms: CI | pending | |

There was no screen smoke this time. The user was using the desktop (Phase 16's shot caught their input method), and even a screenshot raises the smep window over what they are doing. The layout is measured in the gpui tests instead: the column's width, its centring and the margins.
