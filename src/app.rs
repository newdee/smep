//! The root view: a Markdown editor on the left, its live preview on the right.

use std::path::{Path, PathBuf};
use std::time::Duration;

// The preview uses gpui-base's `TextView` directly: the component wrapper
// folds its own style onto the app theme and offers no colour overrides,
// while the base one takes a full `TextViewStyle`, which the preview themes
// need. Without a style it renders with the defaults the component theme
// installs, so the "System" theme looks exactly as before.
use gpui_kit::base::input::{InputBaseState, MultiLineMode};
use gpui_kit::base::text::TextView;
use gpui_kit::base::{ElementExt as _, POPUP_PRIORITY};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{
    Copy, Cut, Editor, EditorState, InputEvent, Paste, Redo, Replace, RopeExt as _, Search,
    SelectAll, TextareaState, Undo,
};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Theme, TitleBar, h_flex, h_resizable, resizable_panel, v_flex,
    window_border,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::highlight;
use crate::insert;
use crate::io::{self, Document};
use crate::keymap::{
    self, Open, Quit, Save, SaveAs, ToggleFullscreen, ToggleMenuBar, ViewRendered, ViewSource,
    ViewSplit,
};
use crate::rendered::{RenderedEvent, RenderedView};
use crate::settings::{Settings, ViewMode};
use crate::theme::{self, PreviewTheme};

/// How long typing has to pause before the preview re-parses.
pub const PREVIEW_DEBOUNCE: Duration = Duration::from_millis(80);

/// How the preview interprets the source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewFormat {
    Markdown,
    Html,
}

impl PreviewFormat {
    pub fn for_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm") => {
                Self::Html
            }
            _ => Self::Markdown,
        }
    }
}

/// Whichever popup menu is open (insert, editor context, preview theme).
struct InsertMenu {
    menu: Entity<PopupMenu>,
    position: Point<Pixels>,
    _dismiss: Subscription,
}

/// The input the user is typing in: the source editor, or the block being
/// edited in the rendered view. Menus and the "+" work on whichever it is.
#[derive(Clone)]
enum ActiveEditor {
    Source(Entity<EditorState>),
    Block(Entity<TextareaState>),
}

impl ActiveEditor {
    /// Run `f` on the state, whichever kind it is.
    fn read<R>(&self, cx: &App, f: impl FnOnce(&dyn LineInput) -> R) -> R {
        match self {
            Self::Source(editor) => f(editor.read(cx)),
            Self::Block(editor) => f(editor.read(cx)),
        }
    }

    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self {
            Self::Source(editor) => editor.focus_handle(cx),
            Self::Block(editor) => editor.focus_handle(cx),
        }
    }

    /// Window-space bounds of the cursor's line, once the input has laid out.
    fn cursor_line_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.read(cx, |state| state.cursor_line_bounds())
    }

    fn cursor_line_is_blank(&self, cx: &App) -> bool {
        self.read(cx, |state| state.cursor_line_trimmed().is_empty())
    }

    /// Put a block's skeleton at the cursor.
    fn insert(&self, kind: insert::BlockKind, window: &mut Window, cx: &mut App) {
        match self {
            Self::Source(editor) => editor.update(cx, |state, cx| {
                insert::insert_block(state, kind, window, cx);
            }),
            Self::Block(editor) => editor.update(cx, |state, cx| {
                insert::insert_block(state, kind, window, cx);
            }),
        }
    }
}

/// What [`ActiveEditor`] needs from either kind of input state.
trait LineInput {
    fn cursor_line_bounds(&self) -> Option<Bounds<Pixels>>;
    fn cursor_line_trimmed(&self) -> String;
}

impl<M: MultiLineMode> LineInput for InputBaseState<M> {
    fn cursor_line_bounds(&self) -> Option<Bounds<Pixels>> {
        let line = insert::cursor_line_range(self);
        self.range_to_bounds(&(line.start..line.start))
    }

    fn cursor_line_trimmed(&self) -> String {
        insert::cursor_line_trimmed(self)
    }
}

/// The editor pane and the preview pane, kept in sync.
///
/// The editor state owns the text. Every change it emits is copied into
/// `source` at once (that drives the dirty marker) and handed to the preview
/// after a short pause in typing (`previewed` is what the preview has).
/// `saved` is the text as it last was on disk; the document is dirty while
/// `source` differs from it.
pub struct Smep {
    editor: Entity<EditorState>,
    preview: Entity<TextViewState>,
    /// The rendered view, editable block by block; shares `editor`'s text.
    rendered: Entity<RenderedView>,
    path: Option<PathBuf>,
    format: PreviewFormat,
    source: SharedString,
    previewed: SharedString,
    saved: SharedString,
    preview_refresh: Option<Task<()>>,
    /// Start line of each top-level block in `previewed`; one preview item each.
    blocks: Vec<usize>,
    /// The editor's top line last pushed to the preview.
    scroll_sync: Option<usize>,
    insert_menu: Option<InsertMenu>,
    settings: Settings,
    /// Focus for the window when no editor is shown (rendered mode), so the
    /// shortcuts bound on the root keep working.
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

fn preview_state(
    format: PreviewFormat,
    text: &str,
    cx: &mut Context<TextViewState>,
) -> TextViewState {
    match format {
        PreviewFormat::Markdown => TextViewState::markdown(text, cx),
        PreviewFormat::Html => TextViewState::html(text, cx),
    }
}

/// The first buffer line the editor currently renders, once it has laid
/// out. `None` before the first layout.
///
/// The editor lays out only the rows on screen. `range_to_bounds` maps every
/// offset above them to the first rendered line, the rendered lines to
/// increasing tops, and everything below to `None`. So the first rendered
/// line is the last one that still reports the same top as line 0, found by
/// binary search. This holds with soft wrap, where display rows and buffer
/// lines differ.
fn top_visible_line(state: &EditorState) -> Option<usize> {
    let text = state.text();
    let lines = text.lines_len();
    if lines == 0 {
        return Some(0);
    }
    let top_of = |row: usize| {
        let offset = text.line_start_offset(row);
        state.range_to_bounds(&(offset..offset)).map(|b| b.top())
    };
    let first_top = top_of(0)?;
    let (mut last_same, mut beyond) = (0, lines);
    while last_same + 1 < beyond {
        let mid = (last_same + beyond) / 2;
        if top_of(mid) == Some(first_top) {
            last_same = mid;
        } else {
            beyond = mid;
        }
    }
    Some(last_same)
}

impl Smep {
    pub fn new(
        document: Document,
        settings: Settings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Theme::sync_system_appearance(Some(window), cx);

        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value(document.text.clone())
                .language(highlight::LANGUAGE)
                .soft_wrap(true)
                .line_number(false)
                .searchable(true)
                .placeholder("Write here. Type / on an empty line to insert a block.")
        });
        editor.update(cx, |state, cx| {
            state.set_highlighter_factory(highlight::factory(), cx);
            state.focus(window, cx);
        });
        let preview = cx.new(|cx| preview_state(document.format, &document.text, cx));
        let rendered = cx.new(|cx| RenderedView::new(editor.clone(), window, cx));

        let subscriptions = vec![
            cx.subscribe_in(
                &rendered,
                window,
                |this, _, event: &RenderedEvent, window, cx| match event {
                    RenderedEvent::SlashTyped => this.open_insert_menu(window, cx),
                },
            ),
            cx.subscribe_in(
                &editor,
                window,
                |this, editor, event: &InputEvent, window, cx| {
                    if let InputEvent::Change = event {
                        let was_dirty = this.is_dirty();
                        this.source = editor.read(cx).value();
                        if this.is_dirty() != was_dirty {
                            this.refresh_title(window);
                        }
                        this.schedule_preview_refresh(window, cx);
                        if Self::slash_typed(editor.read(cx)) {
                            this.open_insert_menu(window, cx);
                        }
                        cx.notify();
                    }
                },
            ),
            // Cursor moves do not emit events; observe so the "+" follows the cursor.
            cx.observe(&editor, |_, _, cx| cx.notify()),
            // A block starting or ending its edit changes where menus send
            // their actions; the menus are rebuilt on render.
            cx.observe(&rendered, |_, _, cx| cx.notify()),
            window.observe_window_appearance(|window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
        ];

        // Closing with unsaved changes asks first. The platform close is
        // vetoed here and redone once the prompt resolves.
        window.on_window_should_close(cx, {
            let this = cx.weak_entity();
            move |window, cx| {
                let Some(smep) = this.upgrade() else {
                    return true;
                };
                if !smep.read(cx).is_dirty() {
                    return true;
                }
                let proceed = smep.update(cx, |this, cx| this.confirm_discard(window, cx));
                let handle = window.window_handle();
                cx.spawn(async move |cx| {
                    if proceed.await {
                        let _ = handle.update(cx, |_, window, _| window.remove_window());
                    }
                })
                .detach();
                false
            }
        });

        let text = document.text;
        let saved: SharedString = text.clone().into();
        let this = Self {
            editor,
            preview,
            rendered,
            path: document.path,
            format: document.format,
            source: saved.clone(),
            previewed: saved.clone(),
            saved,
            preview_refresh: None,
            blocks: highlight::block_start_lines(&text),
            scroll_sync: None,
            insert_menu: None,
            focus_handle: cx.focus_handle(),
            settings,
            _subscriptions: subscriptions,
        };
        this.refresh_title(window);
        if this.settings.view_mode == ViewMode::Rendered {
            this.rendered
                .read(cx)
                .focus_handle()
                .clone()
                .focus(window, cx);
        }
        this
    }

    pub fn preview_theme(&self) -> PreviewTheme {
        self.settings.preview_theme
    }

    /// Switch the preview theme and remember it. A failed write only warns:
    /// the theme still applies for this session.
    pub fn set_preview_theme(&mut self, theme: PreviewTheme, cx: &mut Context<Self>) {
        if self.settings.preview_theme == theme {
            return;
        }
        self.settings.preview_theme = theme;
        self.save_settings();
        cx.notify();
    }

    pub fn view_mode(&self) -> ViewMode {
        self.settings.view_mode
    }

    /// Show source, split, or rendered; remembered across runs.
    pub fn set_view_mode(&mut self, mode: ViewMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.view_mode != mode {
            self.settings.view_mode = mode;
            self.save_settings();
        }
        if mode == ViewMode::Rendered {
            // The rendered view takes focus (its active block, if any, takes
            // it from there), so the root's shortcuts keep working.
            match self.rendered.read(cx).active_editor() {
                Some(block) => block.update(cx, |state, cx| state.focus(window, cx)),
                None => self
                    .rendered
                    .read(cx)
                    .focus_handle()
                    .clone()
                    .focus(window, cx),
            }
        } else {
            self.editor.update(cx, |state, cx| state.focus(window, cx));
        }
        cx.notify();
    }

    pub fn menu_bar_shown(&self) -> bool {
        self.settings.menu_bar
    }

    fn save_settings(&self) {
        if let Err(err) = self.settings.save() {
            eprintln!("smep: could not save settings: {err}");
        }
    }

    fn view_source(&mut self, _: &ViewSource, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Source, window, cx);
    }

    fn view_split(&mut self, _: &ViewSplit, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Split, window, cx);
    }

    fn view_rendered(&mut self, _: &ViewRendered, window: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Rendered, window, cx);
    }

    fn toggle_menu_bar(&mut self, _: &ToggleMenuBar, _: &mut Window, cx: &mut Context<Self>) {
        self.settings.menu_bar = !self.settings.menu_bar;
        self.save_settings();
        cx.notify();
    }

    fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
        cx.notify();
    }

    /// Quit after the unsaved-changes prompt, if any.
    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        let proceed = self.confirm_discard(window, cx);
        cx.spawn(async move |_, cx| {
            if proceed.await {
                cx.update(|cx| cx.quit());
            }
        })
        .detach();
    }

    /// The File / Edit / View menus for the title bar. Every menu returns
    /// focus to the editor being typed in, so the editing actions land there.
    fn render_menu_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let editor_focus = self.active_editor(cx).focus_handle(cx);
        let mode = self.view_mode();
        let menu_bar = self.menu_bar_shown();
        let current_theme = self.preview_theme();
        let this = cx.weak_entity();

        let button = |id: &'static str, label: &'static str| {
            Button::new(id)
                .label(label)
                .ghost()
                .xsmall()
                .tab_stop(false)
        };

        h_flex()
            .id("menu-bar")
            .h_full()
            .items_center()
            .gap_0p5()
            .child(button("menu-file", "File").dropdown_menu({
                let focus = editor_focus.clone();
                move |menu, _, _| {
                    menu.menu("Open…", Box::new(Open))
                        .menu("Save", Box::new(Save))
                        .menu("Save As…", Box::new(SaveAs))
                        .separator()
                        .menu("Quit", Box::new(Quit))
                        .action_context(focus.clone())
                }
            }))
            .child(button("menu-edit", "Edit").dropdown_menu({
                let focus = editor_focus.clone();
                move |menu, _, _| {
                    menu.menu("Undo", Box::new(Undo))
                        .menu("Redo", Box::new(Redo))
                        .separator()
                        .menu("Cut", Box::new(Cut))
                        .menu("Copy", Box::new(Copy))
                        .menu("Paste", Box::new(Paste))
                        .menu("Select All", Box::new(SelectAll))
                        .separator()
                        .menu("Find", Box::new(Search))
                        .menu("Replace", Box::new(Replace))
                        .action_context(focus.clone())
                }
            }))
            .child(button("menu-view", "View").dropdown_menu({
                let focus = editor_focus;
                move |mut menu, window, cx| {
                    for candidate in [ViewMode::Source, ViewMode::Split, ViewMode::Rendered] {
                        let action: Box<dyn Action> = match candidate {
                            ViewMode::Source => Box::new(ViewSource),
                            ViewMode::Split => Box::new(ViewSplit),
                            ViewMode::Rendered => Box::new(ViewRendered),
                        };
                        menu = menu.item(
                            PopupMenuItem::new(candidate.label())
                                .checked(candidate == mode)
                                .action(action),
                        );
                    }
                    let themes = PopupMenu::build(window, cx, |mut submenu, _, _| {
                        for candidate in theme::ALL {
                            let this = this.clone();
                            submenu = submenu.item(
                                PopupMenuItem::new(candidate.label())
                                    .checked(candidate == current_theme)
                                    .on_click(move |_, _, cx| {
                                        let _ = this.update(cx, |this, cx| {
                                            this.set_preview_theme(candidate, cx)
                                        });
                                    }),
                            );
                        }
                        submenu
                    });
                    menu.separator()
                        .item(PopupMenuItem::submenu("Preview Theme", themes))
                        .separator()
                        .item(
                            PopupMenuItem::new("Menu Bar")
                                .checked(menu_bar)
                                .action(Box::new(ToggleMenuBar)),
                        )
                        .menu("Full Screen", Box::new(ToggleFullscreen))
                        .action_context(focus.clone())
                }
            }))
            .into_any_element()
    }

    /// The title bar: menus on the left (when shown), the document in the
    /// middle, the window controls on the right (added by `TitleBar`).
    fn render_title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = format!(
            "{}{}",
            if self.is_dirty() { "● " } else { "" },
            Document::display_name(self.path.as_deref())
        );
        let muted = cx.theme().muted_foreground;
        let menu_bar = self.menu_bar_shown().then(|| self.render_menu_bar(cx));
        TitleBar::new()
            .child(
                h_flex()
                    .id("chrome")
                    .flex_1()
                    .h_full()
                    .items_center()
                    .children(menu_bar)
                    .child(div().flex_1())
                    .child(div().text_sm().text_color(muted).child(title))
                    .child(div().flex_1()),
            )
            .into_any_element()
    }

    /// Re-parse the preview once typing pauses. Each call restarts the wait.
    fn schedule_preview_refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_refresh = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(PREVIEW_DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| this.refresh_preview(cx));
        }));
    }

    /// Hand the current source to the preview, if it does not have it yet.
    fn refresh_preview(&mut self, cx: &mut Context<Self>) {
        if self.previewed == self.source {
            return;
        }
        self.previewed = self.source.clone();
        let text = self.previewed.clone();
        self.blocks = highlight::block_start_lines(&text);
        self.preview
            .update(cx, |state, cx| state.set_text(&text, cx));
        self.scroll_sync = None;
        cx.notify();
    }

    /// Point the preview at `text` in `format`, right away.
    fn set_preview(&mut self, format: PreviewFormat, cx: &mut Context<Self>) {
        self.preview_refresh = None;
        self.previewed = self.source.clone();
        let text = self.previewed.clone();
        self.blocks = highlight::block_start_lines(&text);
        if format != self.format {
            self.format = format;
            self.preview = cx.new(|cx| preview_state(format, &text, cx));
        } else {
            self.preview
                .update(cx, |state, cx| state.set_text(&text, cx));
        }
        self.scroll_sync = None;
        cx.notify();
    }

    /// Scroll the preview to the block that holds the editor's top line.
    /// Editor scrolling drives the preview; the preview never drives the editor.
    fn sync_preview_scroll(&mut self, cx: &mut Context<Self>) {
        let Some(top) = top_visible_line(self.editor.read(cx)) else {
            return;
        };
        if self.scroll_sync == Some(top) {
            return;
        }
        self.scroll_sync = Some(top);

        let list = self.preview.read(cx).list_state().clone();
        let count = list.item_count();
        if count == 0 {
            return;
        }
        let item_ix = if count == self.blocks.len() {
            // One preview item per block: the last block starting at or above `top`.
            self.blocks
                .partition_point(|&start| start <= top)
                .saturating_sub(1)
        } else {
            // The preview is mid-parse or split blocks differently; go proportional.
            let lines = self.editor.read(cx).text().lines_len().max(1);
            (count * top / lines).min(count - 1)
        };
        list.scroll_to(ListOffset {
            item_ix,
            offset_in_item: px(0.),
        });
    }

    /// The window title for a document: `● name — smep` while dirty.
    pub fn title_for(path: Option<&Path>, dirty: bool) -> String {
        let marker = if dirty { "● " } else { "" };
        format!("{marker}{} — smep", Document::display_name(path))
    }

    pub fn is_dirty(&self) -> bool {
        self.source != self.saved
    }

    fn refresh_title(&self, window: &mut Window) {
        window.set_window_title(&Self::title_for(self.path.as_deref(), self.is_dirty()));
    }

    /// Replace the buffer with `document`, as after Open.
    fn load(&mut self, document: Document, window: &mut Window, cx: &mut Context<Self>) {
        self.path = document.path;
        self.saved = document.text.clone().into();
        self.source = self.saved.clone();
        // `set_value` emits no change event, so `source` is set by hand above
        // and the rendered view is told that its block is gone.
        self.rendered
            .update(cx, |rendered, cx| rendered.deactivate(window, cx));
        self.editor
            .update(cx, |state, cx| state.set_value(document.text, window, cx));
        self.set_preview(document.format, cx);
        self.refresh_title(window);
    }

    /// Write the buffer to `path` and adopt it as the document's path.
    fn write_to(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = self.editor.read(cx).value();
        match io::write(&path, &text) {
            Ok(()) => {
                self.saved = text.clone();
                self.source = text;
                let format = PreviewFormat::for_path(&path);
                self.path = Some(path);
                if format != self.format {
                    self.set_preview(format, cx);
                }
                self.refresh_title(window);
                cx.notify();
                true
            }
            Err(err) => {
                self.report(
                    &format!("Could not save {}", path.display()),
                    &err.to_string(),
                    window,
                    cx,
                );
                false
            }
        }
    }

    fn report(&self, what: &str, detail: &str, window: &mut Window, cx: &mut Context<Self>) {
        let answered = window.prompt(PromptLevel::Critical, what, Some(detail), &["OK"], cx);
        cx.spawn(async move |_, _| {
            let _ = answered.await;
        })
        .detach();
    }

    /// Save to the current path, or ask for one. Resolves to whether it saved.
    fn save_task(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        match self.path.clone() {
            Some(path) => Task::ready(self.write_to(path, window, cx)),
            None => self.save_as_task(window, cx),
        }
    }

    /// Ask for a path, then save there. Resolves to whether it saved.
    fn save_as_task(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        let directory = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let suggested = self
            .path
            .as_deref()
            .map(|path| Document::display_name(Some(path)))
            .unwrap_or_else(|| "untitled.md".to_string());
        let chosen = cx.prompt_for_new_path(&directory, Some(&suggested));

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return false;
            };
            this.update_in(cx, |this, window, cx| this.write_to(path, window, cx))
                .unwrap_or(false)
        })
    }

    /// Ask before discarding unsaved changes. Resolves to whether to proceed.
    fn confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        if !self.is_dirty() {
            return Task::ready(true);
        }
        let detail = format!(
            "{} has unsaved changes.",
            Document::display_name(self.path.as_deref())
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "Save changes?",
            Some(&detail),
            &["Save", "Don't Save", "Cancel"],
            cx,
        );

        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(0) => match this.update_in(cx, |this, window, cx| this.save_task(window, cx)) {
                Ok(saved) => saved.await,
                Err(_) => false,
            },
            Ok(1) => true,
            _ => false,
        })
    }

    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        let proceed = self.confirm_discard(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            if !proceed.await {
                return;
            }
            let Ok(chosen) = this.update(cx, |_, cx| {
                cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                })
            }) else {
                return;
            };
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| this.load_path(path, window, cx));
        })
        .detach();
    }

    /// Open the file at `path` in this window, asking about unsaved changes
    /// first. This is what a double-click in the file manager ends up in.
    pub fn open_document_at(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let proceed = self.confirm_discard(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            if proceed.await {
                let _ = this.update_in(cx, |this, window, cx| this.load_path(path, window, cx));
            }
        })
        .detach();
    }

    /// Read `path` into the buffer, or report why not.
    fn load_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        match Document::read(path.clone()) {
            Ok(document) => self.load(document, window, cx),
            Err(err) => self.report(
                &format!("Could not open {}", path.display()),
                &err.to_string(),
                window,
                cx,
            ),
        }
    }

    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        self.save_task(window, cx).detach();
    }

    fn save_as(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        self.save_as_task(window, cx).detach();
    }

    /// A lone `/` on the cursor's line, with the cursor after it.
    fn slash_typed(state: &EditorState) -> bool {
        insert::cursor_line_trimmed(state) == "/"
            && state.cursor() == insert::cursor_line_range(state).end
    }

    /// The editor the user is typing in: the active block in the rendered
    /// view, otherwise the source editor.
    fn active_editor(&self, cx: &App) -> ActiveEditor {
        if self.view_mode() == ViewMode::Rendered
            && let Some(editor) = self.rendered.read(cx).active_editor()
        {
            return ActiveEditor::Block(editor);
        }
        ActiveEditor::Source(self.editor.clone())
    }

    /// Window-space bounds of the cursor's line, once the editor has laid out.
    fn cursor_line_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.active_editor(cx).cursor_line_bounds(cx)
    }

    /// Open a popup menu at `position`, replacing any menu already open.
    /// Actions in it dispatch from the editor, and focus returns there.
    fn open_menu(
        &mut self,
        position: Point<Pixels>,
        build: impl FnOnce(PopupMenu) -> PopupMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor_focus = self.active_editor(cx).focus_handle(cx);
        let menu = PopupMenu::build(window, cx, |menu, _, _| {
            build(menu).action_context(editor_focus)
        });
        let dismiss = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            this.insert_menu = None;
            cx.notify();
        });
        self.insert_menu = Some(InsertMenu {
            menu,
            position,
            _dismiss: dismiss,
        });
        cx.notify();
    }

    /// The block entries, one per snippet, inserting at the editor's cursor.
    fn insert_items(editor: &ActiveEditor, mut menu: PopupMenu) -> PopupMenu {
        for entry in insert::MENU {
            menu = match entry {
                Some(kind) => {
                    let kind = *kind;
                    let editor = editor.clone();
                    menu.item(
                        PopupMenuItem::new(kind.label())
                            .on_click(move |_, window, cx| editor.insert(kind, window, cx)),
                    )
                }
                None => menu.separator(),
            };
        }
        menu
    }

    /// Open the block menu under the cursor's line.
    pub fn open_insert_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.insert_menu.is_some() {
            return;
        }
        let Some(bounds) = self.cursor_line_bounds(cx) else {
            return;
        };
        let editor = self.active_editor(cx);
        self.open_menu(
            bounds.bottom_left(),
            move |menu| Self::insert_items(&editor, menu),
            window,
            cx,
        );
    }

    /// The editor's right-click menu: clipboard, then the blocks. The editor
    /// has already moved the cursor to the click, so blocks land there.
    pub fn open_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = self.active_editor(cx);
        self.open_menu(
            position,
            move |menu| {
                let menu = menu
                    .menu("Cut", Box::new(Cut))
                    .menu("Copy", Box::new(Copy))
                    .menu("Paste", Box::new(Paste))
                    .separator();
                Self::insert_items(&editor, menu)
            },
            window,
            cx,
        );
    }

    /// The preview's right-click menu: pick a theme.
    pub fn open_theme_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.preview_theme();
        let this = cx.weak_entity();
        self.open_menu(
            position,
            move |mut menu| {
                for theme in theme::ALL {
                    let this = this.clone();
                    menu = menu.item(
                        PopupMenuItem::new(theme.label())
                            .checked(theme == current)
                            .on_click(move |_, _, cx| {
                                let _ =
                                    this.update(cx, |this, cx| this.set_preview_theme(theme, cx));
                            }),
                    );
                }
                menu
            },
            window,
            cx,
        );
    }

    /// A click in the editor below its last line: the Feishu gesture for
    /// "add something at the end". The editor handles the click first (it
    /// moves the cursor); the menu opens on the deferred pass, at the end.
    fn on_editor_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || !self.is_below_text(event.position, cx) {
            return;
        }
        let this = cx.entity();
        window.defer(cx, move |window, cx| {
            this.update(cx, |this, cx| {
                this.editor.update(cx, |state, cx| {
                    let end = state.text().len();
                    state.set_selected_range(end..end, cx);
                });
                this.open_insert_menu(window, cx);
            });
        });
    }

    /// Whether `position` is inside the editor but under its last line.
    /// The last line answers `None` while it is scrolled out of view, and
    /// then nothing is "below the text".
    fn is_below_text(&self, position: Point<Pixels>, cx: &App) -> bool {
        let state = self.editor.read(cx);
        if !state.input_bounds().contains(&position) {
            return false;
        }
        let len = state.text().len();
        match state.range_to_bounds(&(len..len)) {
            Some(last) => position.y > last.bottom(),
            None => false,
        }
    }

    /// The "+" beside an empty line while the editor has focus.
    fn render_plus(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.insert_menu.is_some() {
            return None;
        }
        let editor = self.active_editor(cx);
        if !editor.focus_handle(cx).is_focused(window) || !editor.cursor_line_is_blank(cx) {
            return None;
        }
        let bounds = editor.cursor_line_bounds(cx)?;

        let size = px(18.);
        let origin = point(
            bounds.left() - px(24.),
            bounds.top() + (bounds.size.height - size) / 2.,
        );
        let border = cx.theme().border;
        let color = cx.theme().muted_foreground;
        let hover = cx.theme().muted;

        let plus = div()
            .id("insert-plus")
            .size(size)
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(border)
            .text_color(color)
            .text_sm()
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child("+")
            .on_click(cx.listener(|this, _, window, cx| this.open_insert_menu(window, cx)));

        Some(
            deferred(anchored().position(origin).child(plus))
                .with_priority(1)
                .into_any_element(),
        )
    }

    /// The open insert menu, focused so the keyboard drives it.
    fn render_insert_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.insert_menu.as_ref()?;
        let focus = open.menu.focus_handle(cx);
        if !focus.contains_focused(window, cx) {
            focus.focus(window, cx);
        }
        Some(
            deferred(
                anchored()
                    .position(open.position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(open.menu.clone()),
            )
            .with_priority(POPUP_PRIORITY)
            .into_any_element(),
        )
    }
}

impl Render for Smep {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The editor lays out before the preview (it comes first in the
        // split), so by the preview's prepaint the editor's rows for this
        // frame are known and the preview can follow them in the same frame.
        let follow_editor = {
            let this = cx.weak_entity();
            move |_, _: &mut Window, cx: &mut App| {
                let _ = this.update(cx, |this, cx| this.sync_preview_scroll(cx));
            }
        };
        let palette = self.preview_theme().palette();
        let background = palette.map_or(cx.theme().background, |p| p.background);
        let foreground = palette.map_or(cx.theme().foreground, |p| p.foreground);
        let preview = div()
            .id("preview")
            .size_full()
            .overflow_hidden()
            .bg(background)
            .text_color(foreground)
            .when_some(palette.and_then(|p| p.font_family()), |this, family| {
                this.font_family(family)
            })
            .px_6()
            .py_4()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.open_theme_menu(event.position, window, cx);
                }),
            )
            .on_prepaint(follow_editor)
            .child(
                TextView::new(&self.preview)
                    .scrollable(true)
                    .when_some(palette, |view, p| view.style(p.text_view_style())),
            );

        // Right-click: the editor moves the cursor to the click, then hands
        // over here while it is still borrowed, so the menu opens one step
        // later. An empty native menu is returned, so none shows.
        let context_menu = {
            let this = cx.weak_entity();
            move |menu, window: &mut Window, cx: &mut App| {
                let position = window.mouse_position();
                let this = this.clone();
                window.defer(cx, move |window, cx| {
                    let _ =
                        this.update(cx, |this, cx| this.open_context_menu(position, window, cx));
                });
                menu
            }
        };
        let editor = div()
            .id("editor")
            .size_full()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_editor_mouse_down))
            .child(
                Editor::new(&self.editor)
                    .bordered(false)
                    .size_full()
                    .pl(px(28.))
                    .context_menu(context_menu),
            );

        let plus = self.render_plus(window, cx);
        let menu = self.render_insert_menu(window, cx);
        let title_bar = self.render_title_bar(cx);

        let content = match self.view_mode() {
            ViewMode::Source => editor.into_any_element(),
            ViewMode::Rendered => {
                self.rendered
                    .update(cx, |rendered, cx| rendered.set_palette(palette, cx));
                self.rendered.clone().into_any_element()
            }
            ViewMode::Split => h_resizable("smep-split")
                .child(resizable_panel().child(editor.into_any_element()))
                .child(resizable_panel().child(preview.into_any_element()))
                .into_any_element(),
        };

        window_border().child(
            v_flex()
                .size_full()
                .key_context(keymap::CONTEXT)
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::open))
                .on_action(cx.listener(Self::save))
                .on_action(cx.listener(Self::save_as))
                .on_action(cx.listener(Self::quit))
                .on_action(cx.listener(Self::view_source))
                .on_action(cx.listener(Self::view_split))
                .on_action(cx.listener(Self::view_rendered))
                .on_action(cx.listener(Self::toggle_menu_bar))
                .on_action(cx.listener(Self::toggle_fullscreen))
                .child(title_bar)
                .child(div().flex_1().min_h_0().w_full().child(content))
                .children(plus)
                .children(menu),
        )
    }
}

#[cfg(test)]
mod tests {
    // Not `use gpui_kit::*`: with `test-support` on, the glob would also pull
    // in GPUI's `test` attribute and shadow the built-in `#[test]`.
    use std::path::{Path, PathBuf};

    use gpui_kit::base::input::{InputBaseState, InputModeKind};
    use gpui_kit::component::input::{EditorState, Position, TextareaState};
    use gpui_kit::{
        Entity, Focusable as _, MouseButton, MouseDownEvent, TestAppContext, VisualTestContext,
        point, px,
    };

    use super::{PREVIEW_DEBOUNCE, PreviewFormat, Smep};
    use crate::insert::{self, BlockKind};
    use crate::io::Document;
    use crate::keymap::{Open, Save};
    use crate::settings::{Settings, ViewMode};
    use crate::theme::PreviewTheme;

    /// `cmd-…` on macOS, `ctrl-…` elsewhere, as in the key bindings.
    fn primary(keys: &str) -> String {
        format!(
            "{}-{keys}",
            if cfg!(target_os = "macos") {
                "cmd"
            } else {
                "ctrl"
            }
        )
    }

    fn open<'a>(
        cx: &'a mut TestAppContext,
        text: &str,
    ) -> (Entity<Smep>, &'a mut VisualTestContext) {
        open_document(
            cx,
            Document {
                path: None,
                text: text.to_string(),
                format: PreviewFormat::Markdown,
            },
        )
    }

    fn open_document(
        cx: &mut TestAppContext,
        document: Document,
    ) -> (Entity<Smep>, &mut VisualTestContext) {
        open_with(cx, document, Settings::default())
    }

    fn open_with(
        cx: &mut TestAppContext,
        document: Document,
        settings: Settings,
    ) -> (Entity<Smep>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::keymap::init(cx);
        });
        cx.add_window_view(|window, cx| Smep::new(document, settings, window, cx))
    }

    fn mouse_down(cx: &mut VisualTestContext, button: MouseButton, x: f32, y: f32) {
        cx.simulate_event(MouseDownEvent {
            button,
            position: point(px(x), px(y)),
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        });
        cx.run_until_parked();
    }

    fn editor(smep: &Entity<Smep>, cx: &VisualTestContext) -> Entity<EditorState> {
        cx.read(|cx| smep.read(cx).editor.clone())
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    }

    fn value<M: InputModeKind>(
        editor: &Entity<InputBaseState<M>>,
        cx: &VisualTestContext,
    ) -> String {
        cx.read(|cx| editor.read(cx).value().to_string())
    }

    fn menu_open(smep: &Entity<Smep>, cx: &VisualTestContext) -> bool {
        cx.read(|cx| smep.read(cx).insert_menu.is_some())
    }

    fn dirty(smep: &Entity<Smep>, cx: &VisualTestContext) -> bool {
        cx.read(|cx| smep.read(cx).is_dirty())
    }

    fn type_text<M: InputModeKind>(
        editor: &Entity<InputBaseState<M>>,
        text: &str,
        cx: &mut VisualTestContext,
    ) {
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| state.replace_all(text, window, cx));
        });
    }

    /// A fresh directory under the system temp dir, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("smep-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[gpui_kit::test]
    fn an_edit_in_the_editor_reaches_the_preview(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        type_text(&editor, "# Hello", cx);

        assert_eq!(cx.read(|cx| smep.read(cx).source.to_string()), "# Hello");
    }

    #[gpui_kit::test]
    fn the_preview_follows_after_a_pause_in_typing(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        type_text(&editor, "# One", cx);
        type_text(&editor, "# One two", cx);
        assert_eq!(cx.read(|cx| smep.read(cx).previewed.to_string()), "");

        cx.executor().advance_clock(PREVIEW_DEBOUNCE);
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| smep.read(cx).previewed.to_string()),
            "# One two"
        );
    }

    #[gpui_kit::test]
    fn scrolling_the_editor_scrolls_the_preview(cx: &mut TestAppContext) {
        let text: String = (1..=400).map(|n| format!("Paragraph {n}\n\n")).collect();
        let (smep, cx) = open(cx, &text);
        let editor = editor(&smep, cx);
        draw(cx);
        let list = cx.read(|cx| smep.read(cx).preview.read(cx).list_state().clone());
        assert_eq!(list.logical_scroll_top().item_ix, 0);
        assert!(list.item_count() > 100, "one item per paragraph");

        cx.update(|_, cx| {
            editor.update(cx, |state, cx| {
                state.set_scroll_offset(gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(-5000.)), cx)
            });
        });
        draw(cx);
        draw(cx);

        // 5000 px at the editor's row height lands well past line 100; each
        // paragraph is two buffer lines, so the preview item is about half that.
        // No soft wrap kicks in for these short lines, so the display row the
        // editor reports is also the buffer line.
        let display_row = cx.read(|cx| editor.read(cx).visible_row_range().unwrap().start);
        let top_line = cx.read(|cx| smep.read(cx).scroll_sync);
        let top_line = top_line.expect("the editor has laid out");
        assert!(top_line > 100, "editor top line {top_line}");
        assert_eq!(top_line, display_row);
        let item = list.logical_scroll_top().item_ix;
        assert_eq!(item, top_line / 2, "preview item for line {top_line}");
    }

    #[gpui_kit::test]
    fn the_initial_text_is_the_preview_source(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "- one\n- two");
        let editor = editor(&smep, cx);

        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "- one\n- two"
        );
        assert_eq!(value(&editor, cx), "- one\n- two");
        assert!(!dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn editing_marks_the_document_dirty_and_saving_clears_it(cx: &mut TestAppContext) {
        let dir = TempDir::new("save");
        let path = dir.path("notes.md");
        std::fs::write(&path, "before").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        let editor = editor(&smep, cx);
        assert!(!dirty(&smep, cx));

        type_text(&editor, "after\r\n", cx);
        assert!(dirty(&smep, cx));
        assert_eq!(
            cx.read(|cx| Smep::title_for(smep.read(cx).path.as_deref(), true)),
            "● notes.md — smep"
        );

        cx.update(|window, cx| smep.update(cx, |this, cx| this.save(&Save, window, cx)));
        cx.run_until_parked();

        assert!(!dirty(&smep, cx));
        assert_eq!(std::fs::read(&path).unwrap(), b"after\r\n");
        // Undoing back to the saved text is clean again; one more edit is not.
        type_text(&editor, "before", cx);
        assert!(dirty(&smep, cx));
    }

    #[gpui_kit::test]
    async fn saving_an_untitled_document_asks_for_a_path(cx: &mut TestAppContext) {
        let dir = TempDir::new("save-as");
        let target = dir.path("chosen.md");
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        type_text(&editor, "fresh", cx);

        // The dialog is pending only once the task has asked for it.
        let saved = cx.update(|window, cx| smep.update(cx, |this, cx| this.save_task(window, cx)));
        cx.simulate_new_path_selection({
            let target = target.clone();
            move |_| Some(target)
        });

        assert!(saved.await);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "fresh");
        assert!(!dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), Some(target));
    }

    #[gpui_kit::test]
    async fn cancelling_save_as_leaves_the_document_dirty(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        type_text(&editor, "fresh", cx);

        let saved = cx.update(|window, cx| smep.update(cx, |this, cx| this.save_task(window, cx)));
        cx.simulate_new_path_selection(|_| None);

        assert!(!saved.await);
        assert!(dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), None);
    }

    #[gpui_kit::test]
    async fn discarding_asks_only_while_dirty(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        let clean =
            cx.update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard(window, cx)));
        assert!(clean.await, "a clean document needs no prompt");
        assert!(!cx.has_pending_prompt());

        type_text(&editor, "unsaved", cx);
        let proceed =
            cx.update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard(window, cx)));
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        assert!(!proceed.await, "cancel keeps the document");

        let proceed =
            cx.update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard(window, cx)));
        cx.simulate_prompt_answer("Don't Save");
        assert!(proceed.await, "don't save proceeds without writing");
        assert!(dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn open_replaces_the_buffer_and_resets_dirty(cx: &mut TestAppContext) {
        let dir = TempDir::new("open");
        let path = dir.path("other.html");
        std::fs::write(&path, "<b>bold</b>").unwrap();
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        cx.update(|window, cx| smep.update(cx, |this, cx| this.open(&Open, window, cx)));
        // The task asks for the file only after the (clean) discard check resolves.
        cx.run_until_parked();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response({
            let path = path.clone();
            move |_| Some(vec![path])
        });
        cx.run_until_parked();

        assert_eq!(value(&editor, cx), "<b>bold</b>");
        assert_eq!(
            cx.read(|cx| smep.read(cx).previewed.to_string()),
            "<b>bold</b>",
            "open refreshes the preview at once, no debounce"
        );
        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "<b>bold</b>"
        );
        assert_eq!(cx.read(|cx| smep.read(cx).format), PreviewFormat::Html);
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), Some(path));
        assert!(!dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn view_modes_switch_by_keyboard_and_are_remembered(cx: &mut TestAppContext) {
        let dir = TempDir::new("view-mode");
        let settings = Settings {
            path: Some(dir.path("settings.toml")),
            ..Settings::default()
        };
        let document = Document {
            path: None,
            text: "# T".into(),
            format: PreviewFormat::Markdown,
        };
        let (smep, cx) = open_with(cx, document, settings);
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);
        assert_eq!(cx.read(|cx| smep.read(cx).view_mode()), ViewMode::Split);

        cx.simulate_keystrokes(&primary("1"));
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| smep.read(cx).view_mode()), ViewMode::Source);
        assert!(
            std::fs::read_to_string(dir.path("settings.toml"))
                .unwrap()
                .contains("view_mode = \"source\"")
        );

        cx.simulate_keystrokes(&primary("3"));
        cx.run_until_parked();
        draw(cx);
        assert_eq!(cx.read(|cx| smep.read(cx).view_mode()), ViewMode::Rendered);

        // Back to split: keystrokes still reach the root even though the
        // editor was not rendered in the rendered mode.
        cx.simulate_keystrokes(&primary("2"));
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| smep.read(cx).view_mode()), ViewMode::Split);
    }

    fn rendered_mode(smep: &Entity<Smep>, cx: &mut VisualTestContext) {
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.set_view_mode(ViewMode::Rendered, window, cx)
            })
        });
        draw(cx);
    }

    fn activate_block(
        smep: &Entity<Smep>,
        range: std::ops::Range<usize>,
        cx: &mut VisualTestContext,
    ) {
        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());
        cx.update(|window, cx| {
            rendered.update(cx, |rendered, cx| rendered.activate(range, window, cx))
        });
        draw(cx);
    }

    fn active_block(smep: &Entity<Smep>, cx: &VisualTestContext) -> Entity<TextareaState> {
        cx.read(|cx| smep.read(cx).rendered.read(cx).active_editor())
            .expect("a block is being edited")
    }

    #[gpui_kit::test]
    fn editing_a_block_in_the_rendered_view_writes_through(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title\n\npara\n\n- x");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);

        activate_block(&smep, 9..13, cx);
        let block = active_block(&smep, cx);
        assert_eq!(value(&block, cx), "para");
        assert_eq!(
            cx.read(|cx| block.read(cx).cursor()),
            4,
            "cursor at the block's end"
        );

        type_text(&block, "para two", cx);
        assert_eq!(value(&document, cx), "# Title\n\npara two\n\n- x");
        assert!(dirty(&smep, cx));

        // Grow the block into two paragraphs; the document follows exactly.
        type_text(&block, "para two\n\nthree", cx);
        assert_eq!(value(&document, cx), "# Title\n\npara two\n\nthree\n\n- x");

        // Moving to another block keeps everything and edits that one.
        activate_block(&smep, 0..7, cx);
        let block = active_block(&smep, cx);
        assert_eq!(value(&block, cx), "# Title");
        type_text(&block, "# Big Title", cx);
        assert_eq!(
            value(&document, cx),
            "# Big Title\n\npara two\n\nthree\n\n- x"
        );
    }

    #[gpui_kit::test]
    fn a_new_block_at_the_end_gets_its_blank_line(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());

        cx.update(|window, cx| {
            rendered.update(cx, |rendered, cx| rendered.activate_at_end(window, cx))
        });
        draw(cx);
        let block = active_block(&smep, cx);
        type_text(&block, "new", cx);
        assert_eq!(value(&document, cx), "# Title\n\nnew");
        type_text(&block, "new text", cx);
        assert_eq!(
            value(&document, cx),
            "# Title\n\nnew text",
            "the gap is added once"
        );

        // CRLF documents get a CRLF gap.
        let (smep, cx) = open(cx, "a\r\nb");
        let document = cx.read(|cx| smep.read(cx).editor.clone());
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());
        cx.update(|window, cx| {
            rendered.update(cx, |rendered, cx| rendered.activate_at_end(window, cx))
        });
        draw(cx);
        let block = active_block(&smep, cx);
        type_text(&block, "c", cx);
        assert_eq!(value(&document, cx), "a\r\nb\r\n\r\nc");
    }

    #[gpui_kit::test]
    fn a_slash_in_a_block_opens_the_menu_and_inserts_there(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());
        cx.update(|window, cx| {
            rendered.update(cx, |rendered, cx| rendered.activate_at_end(window, cx))
        });
        draw(cx);

        cx.simulate_input("/");
        assert!(menu_open(&smep, cx), "the slash in a block opens the menu");
        draw(cx);
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();

        let block = active_block(&smep, cx);
        assert_eq!(value(&block, cx), "## ");
        assert_eq!(value(&document, cx), "# Title\n\n## ");
        assert!(!menu_open(&smep, cx));
    }

    #[gpui_kit::test]
    fn saving_from_a_block_editor_writes_the_whole_document(cx: &mut TestAppContext) {
        let dir = TempDir::new("block-save");
        let path = dir.path("blocks.md");
        std::fs::write(&path, "# Title\n\npara\n").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        rendered_mode(&smep, cx);

        // Activating a block focuses its editor; the shortcut must still
        // reach the root from there.
        activate_block(&smep, 9..13, cx);
        let block = active_block(&smep, cx);
        assert!(cx.update(|window, cx| block.focus_handle(cx).is_focused(window)));
        type_text(&block, "para (edited)", cx);
        assert!(dirty(&smep, cx));

        cx.simulate_keystrokes(&primary("s"));
        cx.run_until_parked();

        assert!(!dirty(&smep, cx));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# Title\n\npara (edited)\n"
        );
    }

    #[gpui_kit::test]
    fn a_click_below_the_blocks_starts_a_focused_block_at_the_end(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title");
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());

        // Well under the one heading, inside the view.
        mouse_down(cx, MouseButton::Left, 300., 500.);
        draw(cx);

        let block = active_block(&smep, cx);
        assert!(
            cx.update(|window, cx| block.focus_handle(cx).is_focused(window)),
            "the new block keeps focus after the view's own click handling"
        );
        assert!(!cx.update(|window, cx| rendered.read(cx).focus_handle().is_focused(window)));
        type_text(&block, "tail", cx);
        assert_eq!(value(&editor(&smep, cx), cx), "# Title\n\ntail");
    }

    #[gpui_kit::test]
    fn an_emptied_block_stays_on_screen_and_takes_typing(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title\n\npara\n\n- x");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);
        activate_block(&smep, 9..13, cx);
        let block = active_block(&smep, cx);

        type_text(&block, "", cx);
        assert_eq!(value(&document, cx), "# Title\n\n\n\n- x");
        draw(cx);

        // Keys reach the editor only while it is painted.
        cx.simulate_input("n");
        cx.run_until_parked();
        assert_eq!(value(&block, cx), "n");
        assert_eq!(value(&document, cx), "# Title\n\nn\n\n- x");
    }

    #[gpui_kit::test]
    fn a_change_from_elsewhere_ends_the_block_edit(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title\n\npara");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());
        let active = |cx: &VisualTestContext| cx.read(|cx| rendered.read(cx).active_editor());

        // Typing in the source view moves the block.
        activate_block(&smep, 9..13, cx);
        type_text(&document, "# Longer Title\n\npara", cx);
        assert!(active(cx).is_none(), "the block's offsets went stale");

        // Opening another document.
        activate_block(&smep, 9..13, cx);
        assert!(active(cx).is_some());
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.load(
                    Document {
                        path: None,
                        text: "new".into(),
                        format: PreviewFormat::Markdown,
                    },
                    window,
                    cx,
                )
            })
        });
        assert!(active(cx).is_none());

        // The block's own commits do not end the edit.
        activate_block(&smep, 0..3, cx);
        let block = active_block(&smep, cx);
        type_text(&block, "newer", cx);
        type_text(&block, "newest", cx);
        assert!(active(cx).is_some());
        assert_eq!(value(&document, cx), "newest");
    }

    #[gpui_kit::test]
    fn escape_ends_the_block_edit_and_keeps_the_text(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title\n\npara");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);
        activate_block(&smep, 9..13, cx);
        let block = active_block(&smep, cx);
        type_text(&block, "para!", cx);

        cx.simulate_keystrokes("escape");
        cx.run_until_parked();

        let rendered = cx.read(|cx| smep.read(cx).rendered.clone());
        assert!(cx.read(|cx| rendered.read(cx).active_editor().is_none()));
        assert_eq!(value(&document, cx), "# Title\n\npara!");
        assert!(
            cx.update(|window, cx| rendered.read(cx).focus_handle().is_focused(window)),
            "focus returns to the view so shortcuts keep working"
        );
    }

    #[gpui_kit::test]
    fn a_block_editor_grows_with_its_text(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title\n\n- a\n- b\n- c\n- d");
        rendered_mode(&smep, cx);
        activate_block(&smep, 9..24, cx);
        let block = active_block(&smep, cx);
        draw(cx);

        let line_height = cx
            .read(|cx| block.read(cx).line_height())
            .expect("laid out");
        let height = cx.read(|cx| block.read(cx).input_bounds().size.height);
        assert!(
            height >= line_height * 4.,
            "four lines need four rows: {height:?} < 4 × {line_height:?}"
        );
    }

    #[gpui_kit::test]
    fn the_menu_bar_and_fullscreen_toggle_by_keyboard(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);
        assert!(cx.read(|cx| smep.read(cx).menu_bar_shown()));

        cx.simulate_keystrokes(&primary("shift-m"));
        cx.run_until_parked();
        assert!(!cx.read(|cx| smep.read(cx).menu_bar_shown()));
        cx.simulate_keystrokes(&primary("shift-m"));
        cx.run_until_parked();
        assert!(cx.read(|cx| smep.read(cx).menu_bar_shown()));

        let fullscreen = if cfg!(target_os = "macos") {
            "ctrl-cmd-f"
        } else {
            "f11"
        };
        assert!(!cx.update(|window, _| window.is_fullscreen()));
        cx.simulate_keystrokes(fullscreen);
        cx.run_until_parked();
        assert!(cx.update(|window, _| window.is_fullscreen()));
        cx.simulate_keystrokes(fullscreen);
        cx.run_until_parked();
        assert!(!cx.update(|window, _| window.is_fullscreen()));
    }

    #[gpui_kit::test]
    fn opening_a_path_from_the_os_asks_about_unsaved_changes_first(cx: &mut TestAppContext) {
        let dir = TempDir::new("os-open");
        let path = dir.path("incoming.md");
        std::fs::write(&path, "# From Finder").unwrap();
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        // Clean buffer: opens right away.
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.open_document_at(path.clone(), window, cx)
            })
        });
        cx.run_until_parked();
        assert_eq!(value(&editor, cx), "# From Finder");
        assert_eq!(cx.read(|cx| smep.read(cx).path.clone()), Some(path.clone()));

        // Dirty buffer: the prompt comes first; "Cancel" keeps the edits.
        type_text(&editor, "edited", cx);
        let other = dir.path("other.md");
        std::fs::write(&other, "other").unwrap();
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.open_document_at(other.clone(), window, cx)
            })
        });
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert_eq!(value(&editor, cx), "edited");

        // "Don't Save" lets the new file in.
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.open_document_at(other.clone(), window, cx)
            })
        });
        cx.simulate_prompt_answer("Don't Save");
        cx.run_until_parked();
        assert_eq!(value(&editor, cx), "other");
        assert!(!dirty(&smep, cx));

        // A missing file reports instead of failing silently.
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.open_document_at(dir.path("missing.md"), window, cx)
            })
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt(), "an error dialog is shown");
        cx.simulate_prompt_answer("OK");
        assert_eq!(value(&editor, cx), "other");
    }

    #[gpui_kit::test]
    fn ctrl_s_reaches_the_save_action(cx: &mut TestAppContext) {
        let dir = TempDir::new("keys");
        let path = dir.path("keys.md");
        std::fs::write(&path, "").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);
        type_text(&editor, "typed", cx);
        assert!(dirty(&smep, cx));

        let save = if cfg!(target_os = "macos") {
            "cmd-s"
        } else {
            "ctrl-s"
        };
        cx.simulate_keystrokes(save);
        cx.run_until_parked();

        assert!(!dirty(&smep, cx));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "typed");
    }

    #[cfg(not(target_os = "macos"))]
    #[gpui_kit::test]
    fn ctrl_end_and_ctrl_home_jump_across_the_document(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "first\nsecond\nthird");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);

        cx.simulate_keystrokes("ctrl-end");
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| editor.read(cx).cursor()),
            "first\nsecond\nthird".len()
        );

        cx.simulate_keystrokes("ctrl-home");
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), 0);

        cx.simulate_keystrokes("ctrl-shift-end");
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| editor.read(cx).selected_range()),
            0.."first\nsecond\nthird".len()
        );
    }

    #[gpui_kit::test]
    fn a_slash_on_an_empty_line_opens_the_menu_and_escape_closes_it(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);

        cx.simulate_input("/");
        assert!(
            menu_open(&smep, cx),
            "typing / on an empty line opens the menu"
        );

        draw(cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!menu_open(&smep, cx), "escape closes the menu");
        assert_eq!(value(&editor, cx), "/", "escape keeps what was typed");
    }

    #[gpui_kit::test]
    fn a_slash_inside_text_does_not_open_the_menu(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "a");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_cursor_position(Position::new(0, 1), window, cx);
            });
        });
        draw(cx);

        cx.simulate_input("/");
        assert_eq!(value(&editor, cx), "a/");
        assert!(!menu_open(&smep, cx));
    }

    #[gpui_kit::test]
    fn choosing_the_first_entry_replaces_the_line(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "intro\n\nend");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_cursor_position(Position::new(1, 0), window, cx);
            });
        });
        draw(cx);

        cx.update(|window, cx| smep.update(cx, |this, cx| this.open_insert_menu(window, cx)));
        assert!(menu_open(&smep, cx));
        draw(cx);

        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();

        assert_eq!(value(&editor, cx), "intro\n# \nend");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "intro\n# ".len());
        assert!(!menu_open(&smep, cx), "the menu closes after a choice");
        assert_eq!(
            cx.read(|cx| smep.read(cx).source.to_string()),
            "intro\n# \nend",
            "the preview sees the inserted block"
        );
    }

    #[gpui_kit::test]
    fn a_click_below_the_text_opens_the_menu_at_the_end(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "one\ntwo");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.focus(window, cx);
                state.set_cursor_position(Position::new(0, 0), window, cx);
            });
        });
        draw(cx);
        let (input, last) = cx.read(|cx| {
            let state = editor.read(cx);
            let len = state.text().len();
            (
                state.input_bounds(),
                state
                    .range_to_bounds(&(len..len))
                    .expect("the last line is on screen"),
            )
        });
        assert!(
            input.bottom() > last.bottom() + px(40.),
            "room below the text"
        );

        // On the text: the editor's own click, no menu.
        mouse_down(
            cx,
            MouseButton::Left,
            f32::from(last.left()) + 4.,
            f32::from(last.top()) + 4.,
        );
        assert!(!menu_open(&smep, cx));

        // Well below it: cursor to the end, menu open.
        mouse_down(
            cx,
            MouseButton::Left,
            f32::from(last.left()) + 4.,
            f32::from(input.bottom()) - 10.,
        );
        assert!(
            menu_open(&smep, cx),
            "a click below the text opens the menu"
        );
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "one\ntwo".len());
    }

    #[gpui_kit::test]
    fn right_click_in_the_editor_opens_the_context_menu(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "one\ntwo");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);
        let first = cx.read(|cx| editor.read(cx).range_to_bounds(&(0..0)).unwrap());

        // Down then up: the editor opens its context menu on release.
        let (x, y) = (f32::from(first.left()) + 4., f32::from(first.top()) + 4.);
        mouse_down(cx, MouseButton::Right, x, y);
        cx.simulate_event(gpui_kit::MouseUpEvent {
            button: MouseButton::Right,
            position: point(px(x), px(y)),
            modifiers: Default::default(),
            click_count: 1,
        });
        cx.run_until_parked();
        draw(cx);
        cx.run_until_parked();

        assert!(menu_open(&smep, cx), "right-click opens smep's menu");
        assert!(!cx.read(|cx| {
            smep.read(cx)
                .insert_menu
                .as_ref()
                .unwrap()
                .menu
                .read(cx)
                .is_empty()
        }));
    }

    #[gpui_kit::test]
    fn right_click_on_the_preview_picks_a_theme_and_saves_it(cx: &mut TestAppContext) {
        let dir = TempDir::new("theme");
        let settings = Settings {
            preview_theme: PreviewTheme::System,
            path: Some(dir.path("settings.toml")),
            ..Settings::default()
        };
        let document = Document {
            path: None,
            text: "# T".into(),
            format: PreviewFormat::Markdown,
        };
        let (smep, cx) = open_with(cx, document, settings);
        draw(cx);
        let editor = editor(&smep, cx);
        let preview_x = f32::from(cx.read(|cx| editor.read(cx).input_bounds().right())) + 60.;

        mouse_down(cx, MouseButton::Right, preview_x, 200.);
        assert!(
            menu_open(&smep, cx),
            "right-click on the preview opens the theme menu"
        );
        draw(cx);

        // First `down` selects System (the current one), the second GitHub.
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();

        assert_eq!(
            cx.read(|cx| smep.read(cx).preview_theme()),
            PreviewTheme::Github
        );
        assert!(!menu_open(&smep, cx));
        assert!(
            std::fs::read_to_string(dir.path("settings.toml"))
                .unwrap()
                .contains("preview_theme = \"github\"")
        );
    }

    #[gpui_kit::test]
    fn inserting_on_a_line_with_text_adds_a_line_below(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "text\nnext");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_cursor_position(Position::new(0, 2), window, cx);
                insert::insert_block(state, BlockKind::Heading1, window, cx);
            });
        });
        assert_eq!(value(&editor, cx), "text\n# \nnext");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "text\n# ".len());

        let (smep, cx) = open(cx, "a\r\nb");
        let crlf_editor = cx.read(|cx| smep.read(cx).editor.clone());
        cx.update(|window, cx| {
            crlf_editor.update(cx, |state, cx| {
                state.set_cursor_position(Position::new(0, 1), window, cx);
                insert::insert_block(state, BlockKind::Quote, window, cx);
            });
        });
        assert_eq!(value(&crlf_editor, cx), "a\r\n> \r\nb");
        assert_eq!(cx.read(|cx| crlf_editor.read(cx).cursor()), "a\r\n> ".len());
    }

    #[gpui_kit::test]
    fn insert_block_keeps_crlf_line_endings(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "a\r\n\r\nb");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_cursor_position(Position::new(1, 0), window, cx);
                insert::insert_block(state, BlockKind::Heading2, window, cx);
            });
        });

        assert_eq!(value(&editor, cx), "a\r\n## \r\nb");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "a\r\n## ".len());
    }

    #[gpui_kit::test]
    fn multi_line_snippets_put_the_cursor_inside(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "/");
        let editor = editor(&smep, cx);
        cx.update(|window, cx| {
            editor.update(cx, |state, cx| {
                state.set_cursor_position(Position::new(0, 1), window, cx);
                insert::insert_block(state, BlockKind::CodeBlock, window, cx);
            });
        });

        assert_eq!(value(&editor, cx), "```\n\n```");
        assert_eq!(cx.read(|cx| editor.read(cx).cursor()), "```\n".len());
    }

    #[test]
    fn html_files_get_the_html_preview() {
        assert_eq!(
            PreviewFormat::for_path(Path::new("a.html")),
            PreviewFormat::Html
        );
        assert_eq!(
            PreviewFormat::for_path(Path::new("a.HTM")),
            PreviewFormat::Html
        );
        assert_eq!(
            PreviewFormat::for_path(Path::new("a.md")),
            PreviewFormat::Markdown
        );
        assert_eq!(
            PreviewFormat::for_path(Path::new("README")),
            PreviewFormat::Markdown
        );
    }

    #[test]
    fn titles_show_the_name_and_a_dirty_marker() {
        assert_eq!(Smep::title_for(None, false), "Untitled — smep");
        assert_eq!(
            Smep::title_for(Some(Path::new("/x/a.md")), true),
            "● a.md — smep"
        );
    }
}
