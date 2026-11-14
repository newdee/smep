# Phase 19 — the outline folds by section and lists to a depth (2026-11-14)

The request: "outline 还能按层级折叠吗", then "outline 按层级折叠也做了吧". The first question was answered with two designs (fold a section by its triangle; list down to a level) and a question which to build, which was not answered; the second message took it as done. It was not; both were built, as recommended.

## Design

- `src/outline.rs`, pure: `rows(headings, folded, depth)` lists every heading at `depth` or above, less those inside a folded section (a section runs to the next heading of its level or above); a section is foldable when it holds a heading the depth lists. `shown_for` gives the listed line standing for any heading (itself, or the nearest listed section holding it), which marks the section being read when it is folded away or past the depth.
- Folds are keyed by `SectionKey` (level, title, which of the same-titled headings), so a fold stays with its section while the text above it changes, and a renamed section starts unfolded.
- Folds are the tab's (`DocumentTab::folded`), not saved, cleared when a document is loaded into the tab. The depth is a setting, `outline_depth` (1 to 6, 6 lists all; a new key, so older files get all levels), read through one clamping accessor, `Smep::outline_depth`.
- In the panel: a 12 px triangle column (▾ / ▸, empty where there is nothing to fold, so a level's titles line up) whose click stops there and does not go to the section; chips 1 / 2 / 3 / All in the head beside the fold ‹. Ctrl+Shift+L still puts the whole panel away.
- The UI tests click the triangles and chips where they are drawn, through gpui's `debug_selector` (recorded only in debug test builds; a no-op in release).

Work was done in a second git worktree while the 0.1.15 release chain ran from the main one (see `release-0.1.15.md` for the one slip that caused).

## Rounds

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A (build) | fmt, clippy (default, slim), tests | 134 + 5, slim 124 + 5 | `cargo fmt --check` 28 lines → formatted |
| B | mechanism: six mutations (folds ignored; depth ignored; same-title count always 0; `shown_for` always itself; the triangle's click let through to the jump; folds not cleared on load) | 5 / 6 turn a test red | clearing the folds on load had no test, and the case is real (a tab emptied after folding is reused by the next file opened, whose same-named sections would come up folded) → test added; the mutation now fails it |
| C | boundaries: a hand-edited settings file | — | `outline_depth = 0` listed nothing (only the setter clamped) → one clamping accessor for every read; `a_hand_edited_depth_is_kept_in_range` (0 → 1, 9 → 6, 2 → 2); without the clamp it fails |
| 1 (clean) | static: fmt 0, clippy 0 (default and slim); docs against code | 135 + 5, slim 125 + 5 | none |
| 2 (clean) | reproducibility: the suite 3 times (the clicks go by drawn bounds) | 135 + 5 each time | none |
| 3 (clean) | the other platforms: CI | see below | |

Degenerate outlines are in `outline.rs`'s tests: no headings, a document starting below h1, skipped levels, a fold on a heading that holds nothing, two sections of the same level and title, a heading above every listed one.

No screen smoke (the user is at the desktop).
