# Phase 12 — the blog's Paper, reading as one page, Write / Read (2026-10-21)

Three requests in one round: "paper 和 paper-dark 能不能和 dfine-blog 的 markdown 一样" (option A: the application and the preview both take the blog's palette), "导航栏上面有两个编辑按钮，混淆了", and "只读模式，是不是只展示一个渲染器就行".

## Diagnosis

- **Two Edit buttons**: a naming clash, individual to the title bar: the Edit *menu* (File / Edit / View) and the Edit / Read *toggle* (Phase 9) sat on one bar. The toggle is now Write / Read; the menu keeps its conventional name. The native macOS menu item follows ("Write / Read"). No logic changed.
- **Reading mode showed the block view**: structural. The rendered view exists for editing block by block; read-only it only brought that structure's limits — footnotes and reference links do not resolve across blocks, a selection cannot span blocks, a gap between blocks on top of the paragraph gap, and a second parse of the whole document next to the preview's. Reading now shows the one preview of the whole document (the same `TextViewState` the split view shows), so reading and the split preview are one rendering, one parse, one scroll position. The keys scroll the preview's list (`ListState::scroll_by`, `scroll_to`, `scroll_to_end`) from a focusable reading pane; the rendered view's own key scrolling stays for editing in the rendered mode.
- **Paper = the blog**: the application theme file and the Paper / Paper Dark palettes carry dfine-blog's tokens (`src/styles/tokens.css`): paper `#f8f7f3` / `#121212`, ink `#1a1a18` / `#c1bfba`, soft ink, line, cinnabar `#c3423f` / `#dd625e`. Typography the renderer exposes is the blog's for every preview theme (one `text_view_style`): heading scale 1.62 / 1.46 / 1.2 / 1.02 / 0.92, paragraph gap 0.95 rem, a 1 px framed code block with 8 px corners, tables ruled across only with a 2 px rule under the header. The theme that follows the application now resolves to a palette too (`Palette::from_app`), so "System" and "Paper" style the text the same way instead of the renderer's defaults. Not possible without changes to gpui-base: serif headings or a heading colour, the h2 underline, the h3 square, the quote mark, inline-code borders, the bold wash, emphasis dots (listed under Left).
- **Risk**: `Palette` is no longer optional in the rendered view and the preview; every path that drew with it now draws with a palette, and the tests cover the follow-the-app case. The block view is not rendered while reading, so its block table is computed only when editing in the rendered mode (one test moved to that mode). The reading pane takes the focus where the rendered view did, so the root's shortcuts work; Ctrl+E from reading is covered.

## Rounds

Each round = review from one perspective + the full suite (`cargo test`, Windows) and, where it is about the window, the real one (debug build, `key-shot.ps1`, 2560×1440, dark system).

| Round | Perspective | Result | Findings |
|---|---|---|---|
| A | mechanism in the real window: reading paper.md / long.md, PageDown / End / Home, click + Ctrl+E, an empty document; a light system by flipping `AppsUseLightTheme` | reading is the one page in Paper Dark; PageDown → Section 9, End → 112–120, Home → 1; Write / Read switch; 103 + 4 tests | the table surface came from gpui-base's own theme (`tokens.colors.surface`), which followed the registry flip while gpui-kit's theme did not: white rows on dark paper → the table refinement now sets the paper as its background; the empty-document hint was appended after a full-height TextView and sat on the bottom edge → it replaces the page |
| B | the same window checks after the fixes, plus the fixed Paper theme on the page (`SMEP_CONFIG_DIR` with `preview_theme = "paper"`) and the split | hint at the top left; table rows on the paper in both; Paper page `#f8f7f3` with cinnabar links, framed code, header rule; split editor dark with cinnabar headings; 103 + 4 | none |
| C | static consistency: stale words and types (`Edit button`, `Edit / Read`, `keepane`, `Option<Palette>`, the removed `scroll_handle`), README line 1, clippy `--all-targets` | one stale "Edit button" in the README status line → fixed; clippy clean; 103 + 4 | one |
| 1 (clean) | the editor's selection in the window (click, Ctrl+E, Ctrl+A on paper.md, split): a translucent cinnabar wash, text legible, so the opaque `selection.background` is right; reproducibility: the suite twice more, 103 + 4 both times, 0 failed | clean | none |
| 2 (clean) | code correctness, reading the committed diff: focus and state on each switch (reading pane focus on entering reading, editor or block focus on leaving; `scroll_sync` untouched by reading; `follow_editor` only attached while an editor is on screen), the hint only while reading an empty document, modified keys passed through | clean | none |
| 3 (clean) | CI run __CI__ | | none |

## Left

- The look that needs gpui-base: a serif face or a colour for headings, the h2 fading rule, the h3 square mark, the quote's 「, inline code with a border, the bold wash and emphasis dots under Chinese text. A fork with a few more `TextViewStyle` hooks, then a PR upstream.
- gpui-base's own theme and gpui-kit's read the system appearance separately; on this machine flipping `AppsUseLightTheme` moved only the former. The table was the one place the renderer drew from it; the scrollbar inside the page still does.
