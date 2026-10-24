//! The root view: the open documents in tabs, each a Markdown editor and
//! its live preview, with the outline of the one on screen beside it.

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
    Copy, Cut, Editor, EditorState, InputEvent, Paste, Position, Redo, Replace, RopeExt as _,
    Search, SelectAll, TextareaState, Undo,
};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{
    ActiveTheme as _, Selectable as _, Sizable as _, Theme, TitleBar, h_flex, h_resizable,
    resizable_panel, v_flex, window_border,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::convert;
use crate::defaults;
use crate::export;
use crate::highlight::{self, Heading};
use crate::insert;
use crate::io::{self, Document};
use crate::keymap::{
    self, CloseTab, ExportHtml, ExportPdf, Import, MakeDefault, NewTab, NextTab, Open, PreviousTab,
    Quit, Save, SaveAs, TextFull, TextMedium, TextNarrow, TextWide, ToggleEditing,
    ToggleFullscreen, ToggleMenuBar, ToggleOutline, ViewRendered, ViewSource, ViewSplit,
};
use crate::rendered::{self, RenderedEvent, RenderedView};
use crate::settings::{Settings, TextWidth, ViewMode};
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

/// One open document: its editor and preview, kept in sync, and what is
/// known about it on disk.
///
/// The editor state owns the text. Every change it emits is copied into
/// `source` at once (that drives the dirty marker) and handed to the preview
/// after a short pause in typing (`previewed` is what the preview has).
/// `saved` is the text as it last was on disk; the document is dirty while
/// `source` differs from it.
struct DocumentTab {
    /// Names the tab across awaits, where its index may have changed.
    id: TabId,
    editor: Entity<EditorState>,
    preview: Entity<TextViewState>,
    /// The rendered view, editable block by block; shares `editor`'s text.
    rendered: Entity<RenderedView>,
    path: Option<PathBuf>,
    /// The file a converted document came from; names it and places its
    /// first save while it has no path of its own.
    origin: Option<PathBuf>,
    format: PreviewFormat,
    source: SharedString,
    previewed: SharedString,
    saved: SharedString,
    preview_refresh: Option<Task<()>>,
    /// Start line of each top-level block in `previewed`; one preview item each.
    blocks: Vec<usize>,
    /// The headings of `previewed`, for the outline.
    headings: Vec<Heading>,
    /// The editor's top line last pushed to the preview.
    scroll_sync: Option<usize>,
    _subscriptions: Vec<Subscription>,
}

type TabId = u64;

impl DocumentTab {
    fn is_dirty(&self) -> bool {
        self.source != self.saved
    }

    /// The document's path, or the one it would suggest: the original's
    /// with `.md` for a converted document. `None` for a blank one.
    fn named_path(&self) -> Option<PathBuf> {
        self.path
            .clone()
            .or_else(|| self.origin.as_deref().map(convert::markdown_path_for))
    }

    /// The document's directory, where its relative images are looked up;
    /// `None` for a document that has no place yet.
    fn document_dir(&self) -> Option<PathBuf> {
        self.named_path()
            .as_deref()
            .and_then(Path::parent)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
    }

    /// A blank document nobody has typed in: opening a file replaces it
    /// rather than opening a tab beside it.
    fn is_pristine(&self) -> bool {
        self.path.is_none()
            && self.origin.is_none()
            && self.source.is_empty()
            && self.saved.is_empty()
    }

    /// The tab's label: the name, after a dot while unsaved.
    fn label(&self) -> String {
        format!(
            "{}{}",
            if self.is_dirty() { "● " } else { "" },
            Document::display_name(self.named_path().as_deref())
        )
    }

    /// Whether this tab holds the file at `path`.
    fn holds(&self, path: &Path) -> bool {
        self.path
            .as_deref()
            .is_some_and(|own| io::same_file(own, path))
    }
}

/// The window: a tab per open document (the bar shows once there are two),
/// the outline of the one on screen beside it when asked for, and the
/// title bar with the menus.
pub struct Smep {
    tabs: Vec<DocumentTab>,
    /// Index of the tab on screen.
    active: usize,
    next_tab: TabId,
    insert_menu: Option<InsertMenu>,
    settings: Settings,
    /// Editing, or reading: the document rendered and untouchable. Every
    /// launch starts reading; this is not remembered.
    editing: bool,
    /// Focus for the window when no editor is shown (rendered mode), so the
    /// shortcuts bound on the root keep working.
    focus_handle: FocusHandle,
    /// Focus of the reading pane: the keys scroll the document from here.
    reading_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

/// How far an arrow key scrolls the document being read; a page is the
/// viewport less this.
const LINE_STEP: Pixels = px(40.);

/// The outline's width.
const OUTLINE_WIDTH: Pixels = px(240.);

/// What is left of the outline when it is folded away: a strip to unfold it.
const OUTLINE_STRIP: Pixels = px(18.);

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

        let subscriptions = vec![window.observe_window_appearance(|window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        })];

        // Closing with unsaved changes asks first, tab by tab. The platform
        // close is vetoed here and redone once the prompts resolve.
        window.on_window_should_close(cx, {
            let this = cx.weak_entity();
            move |window, cx| {
                let Some(smep) = this.upgrade() else {
                    return true;
                };
                if !smep.read(cx).is_dirty() {
                    return true;
                }
                let proceed = smep.update(cx, |this, cx| this.confirm_discard_all(window, cx));
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

        let mut this = Self {
            tabs: Vec::new(),
            active: 0,
            next_tab: 0,
            insert_menu: None,
            focus_handle: cx.focus_handle(),
            reading_focus: cx.focus_handle(),
            settings,
            editing: false,
            _subscriptions: subscriptions,
        };
        this.push_tab(document, window, cx);
        this.refresh_title(window);
        // Reading mode: the reading pane holds the focus so the root's
        // shortcuts and the scrolling keys work.
        this.reading_focus.focus(window, cx);
        this
    }

    /// Add a tab for `document` after the others, without showing it.
    /// Returns its index.
    fn push_tab(
        &mut self,
        document: Document,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> usize {
        let id = self.next_tab;
        self.next_tab += 1;
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
        });
        let preview = cx.new(|cx| preview_state(document.format, &document.text, cx));
        let rendered = cx.new(|cx| RenderedView::new(editor.clone(), document.format, window, cx));
        // Only the tab on screen while editing has a writable rendered view
        // (see `show_active`).
        rendered.update(cx, |rendered, cx| rendered.set_read_only(true, window, cx));

        let subscriptions = vec![
            cx.subscribe_in(
                &rendered,
                window,
                move |this, _, event: &RenderedEvent, window, cx| match event {
                    RenderedEvent::SlashTyped => {
                        if this.tab().id == id {
                            this.open_insert_menu(window, cx);
                        }
                    }
                },
            ),
            cx.subscribe_in(
                &editor,
                window,
                move |this, editor, event: &InputEvent, window, cx| {
                    if let InputEvent::Change = event {
                        this.on_change(id, editor, window, cx);
                    }
                },
            ),
            // Cursor moves do not emit events; observe so the "+" follows the cursor.
            cx.observe(&editor, |_, _, cx| cx.notify()),
            // A block starting or ending its edit changes where menus send
            // their actions; the menus are rebuilt on render.
            cx.observe(&rendered, |_, _, cx| cx.notify()),
        ];

        let source: SharedString = document.text.into();
        // A converted document is new and unsaved; a read one is as on disk.
        let saved = if document.origin.is_some() {
            SharedString::default()
        } else {
            source.clone()
        };
        self.tabs.push(DocumentTab {
            id,
            editor,
            preview,
            rendered,
            path: document.path,
            origin: document.origin,
            format: document.format,
            source: source.clone(),
            previewed: source,
            saved,
            preview_refresh: None,
            blocks: Vec::new(),
            headings: Vec::new(),
            scroll_sync: None,
            _subscriptions: subscriptions,
        });
        self.schedule_outline(id, window, cx);
        self.tabs.len() - 1
    }

    /// Open `documents` in tabs after the others, leaving the one on screen
    /// there (the rest of `smep a.md b.md`). A file already open is not
    /// opened twice.
    pub fn add_documents(
        &mut self,
        documents: Vec<Document>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for document in documents {
            let open = document
                .path
                .as_deref()
                .is_some_and(|path| self.tabs.iter().any(|tab| tab.holds(path)));
            if !open {
                self.push_tab(document, window, cx);
            }
        }
        cx.notify();
    }

    /// The tab on screen.
    fn tab(&self) -> &DocumentTab {
        &self.tabs[self.active]
    }

    fn tab_index(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == id)
    }

    fn tab_by_id(&self, id: TabId) -> Option<&DocumentTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    fn tab_by_id_mut(&mut self, id: TabId) -> Option<&mut DocumentTab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    #[cfg(test)]
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    #[cfg(test)]
    pub fn active_tab(&self) -> usize {
        self.active
    }

    /// A change in the editor of tab `id`.
    fn on_change(
        &mut self,
        id: TabId,
        editor: &Entity<EditorState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(ix) = self.tab_index(id) else {
            return;
        };
        let tab = &mut self.tabs[ix];
        let was_dirty = tab.is_dirty();
        tab.source = editor.read(cx).value();
        let shown = ix == self.active;
        if shown && tab.is_dirty() != was_dirty {
            self.refresh_title(window);
        }
        self.schedule_preview_refresh(id, window, cx);
        if shown && Self::slash_typed(editor.read(cx)) {
            self.open_insert_menu(window, cx);
        }
        cx.notify();
    }

    #[cfg(test)]
    pub fn reading_focus(&self) -> &FocusHandle {
        &self.reading_focus
    }

    /// The preview's list, which the keys scroll while reading.
    #[cfg(test)]
    pub fn reading_list(&self, cx: &App) -> ListState {
        self.tab().preview.read(cx).list_state().clone()
    }

    #[cfg(test)]
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// What the window shows: the chosen view mode while editing, the
    /// rendered document while reading (as one page, the split view's
    /// preview; the block view is for editing).
    pub fn layout(&self) -> ViewMode {
        if self.editing {
            self.settings.view_mode
        } else {
            ViewMode::Rendered
        }
    }

    /// Switch between editing (the remembered view mode) and reading.
    pub fn set_editing(&mut self, editing: bool, window: &mut Window, cx: &mut Context<Self>) {
        if editing {
            self.set_view_mode(self.settings.view_mode, window, cx);
            return;
        }
        if !self.editing {
            return;
        }
        self.editing = false;
        self.show_active(window, cx);
    }

    /// Put the active tab on screen as the mode wants it: while reading its
    /// rendered view is read-only (an edit in progress in a block ends) and
    /// the reading pane has the focus; while editing the view mode takes
    /// over (`set_view_mode`).
    fn show_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_menu = None;
        if self.editing {
            self.set_view_mode(self.settings.view_mode, window, cx);
        } else {
            self.tab()
                .rendered
                .clone()
                .update(cx, |rendered, cx| rendered.set_read_only(true, window, cx));
            self.reading_focus.focus(window, cx);
        }
        self.refresh_title(window);
        cx.notify();
    }

    /// Show tab `ix`. The tab left keeps its text, scroll and undo; its
    /// block edit, if any, ends.
    pub fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        if ix != self.active {
            let left = self.tab().rendered.clone();
            left.update(cx, |rendered, cx| rendered.set_read_only(true, window, cx));
            self.active = ix;
        }
        self.show_active(window, cx);
    }

    /// A new, empty document in a tab of its own.
    pub fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ix = self.push_tab(Document::empty(), window, cx);
        self.activate(ix, window, cx);
    }

    /// Close tab `ix`, after the unsaved-changes prompt. The last tab
    /// closed leaves an empty one: the window stays until it is closed.
    pub fn close_tab(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix >= self.tabs.len() {
            return;
        }
        self.activate(ix, window, cx);
        let id = self.tab().id;
        let proceed = self.confirm_discard(window, cx);
        cx.spawn_in(window, async move |this, cx| {
            if proceed.await {
                let _ = this.update_in(cx, |this, window, cx| this.remove_tab(id, window, cx));
            }
        })
        .detach();
    }

    fn remove_tab(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.tab_index(id) else {
            return;
        };
        self.tabs.remove(ix);
        if self.tabs.is_empty() {
            self.push_tab(Document::empty(), window, cx);
        }
        if ix < self.active {
            self.active -= 1;
        }
        self.active = self.active.min(self.tabs.len() - 1);
        self.show_active(window, cx);
    }

    fn new_tab_action(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        self.new_tab(window, cx);
    }

    fn close_tab_action(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.close_tab(self.active, window, cx);
    }

    fn next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.activate((self.active + 1) % self.tabs.len(), window, cx);
    }

    fn previous_tab(&mut self, _: &PreviousTab, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.tabs.len();
        self.activate((self.active + count - 1) % count, window, cx);
    }

    /// Keys scroll the document being read: a line, a page, the ends.
    /// Modified keys are shortcuts and pass.
    fn on_reading_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if modifiers.control || modifiers.alt || modifiers.platform {
            return;
        }
        let list = self.tab().preview.read(cx).list_state().clone();
        let page = (list.viewport_bounds().size.height - LINE_STEP).max(LINE_STEP);
        match (keystroke.key.as_str(), modifiers.shift) {
            ("down", false) => list.scroll_by(LINE_STEP),
            ("up", false) => list.scroll_by(-LINE_STEP),
            ("pagedown", false) | ("space", false) => list.scroll_by(page),
            ("pageup", false) | ("space", true) => list.scroll_by(-page),
            ("home", false) => list.scroll_to(ListOffset {
                item_ix: 0,
                offset_in_item: px(0.),
            }),
            ("end", false) => list.scroll_to_end(),
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
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

    /// Show source, split, or rendered; remembered across runs. Choosing a
    /// view mode means editing, so reading mode ends.
    pub fn set_view_mode(&mut self, mode: ViewMode, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.view_mode != mode {
            self.settings.view_mode = mode;
            self.save_settings();
        }
        self.editing = true;
        let rendered = self.tab().rendered.clone();
        rendered.update(cx, |rendered, cx| rendered.set_read_only(false, window, cx));
        if mode == ViewMode::Rendered {
            // The rendered view takes focus (its active block, if any, takes
            // it from there), so the root's shortcuts keep working.
            match rendered.read(cx).active_editor() {
                Some(block) => block.update(cx, |state, cx| state.focus(window, cx)),
                None => rendered.read(cx).focus_handle().clone().focus(window, cx),
            }
        } else {
            self.tab()
                .editor
                .clone()
                .update(cx, |state, cx| state.focus(window, cx));
        }
        cx.notify();
    }

    pub fn menu_bar_shown(&self) -> bool {
        self.settings.menu_bar
    }

    pub fn outline_shown(&self) -> bool {
        !self.settings.outline_folded
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

    fn toggle_editing(&mut self, _: &ToggleEditing, window: &mut Window, cx: &mut Context<Self>) {
        self.set_editing(!self.editing, window, cx);
    }

    fn toggle_menu_bar(&mut self, _: &ToggleMenuBar, _: &mut Window, cx: &mut Context<Self>) {
        self.settings.menu_bar = !self.settings.menu_bar;
        self.save_settings();
        cx.notify();
    }

    fn toggle_outline(&mut self, _: &ToggleOutline, _: &mut Window, cx: &mut Context<Self>) {
        self.settings.outline_folded = !self.settings.outline_folded;
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

    /// Quit after the unsaved-changes prompts, if any.
    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        let proceed = self.confirm_discard_all(window, cx);
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
        let editor_focus = self.menu_focus(cx);
        let mode = self.view_mode();
        let editing = self.editing;
        let menu_bar = self.menu_bar_shown();
        let outline = self.outline_shown();
        let text_width = self.text_width();
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
                    menu.menu("New Tab", Box::new(NewTab))
                        .menu("Open…", Box::new(Open))
                        .menu("Import as Markdown…", Box::new(Import))
                        .menu("Save", Box::new(Save))
                        .menu("Save As…", Box::new(SaveAs))
                        .separator()
                        .menu("Export as PDF…", Box::new(ExportPdf))
                        .menu("Export as HTML…", Box::new(ExportHtml))
                        .separator()
                        .menu("Open Markdown Files with smep", Box::new(MakeDefault))
                        .separator()
                        .menu("Close Tab", Box::new(CloseTab))
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
                    menu = menu
                        .item(
                            PopupMenuItem::new("Editing")
                                .checked(editing)
                                .action(Box::new(ToggleEditing)),
                        )
                        .separator();
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
                    let widths = PopupMenu::build(window, cx, |mut submenu, _, _| {
                        for candidate in TextWidth::ALL {
                            let action: Box<dyn Action> = match candidate {
                                TextWidth::Narrow => Box::new(TextNarrow),
                                TextWidth::Medium => Box::new(TextMedium),
                                TextWidth::Wide => Box::new(TextWide),
                                TextWidth::Full => Box::new(TextFull),
                            };
                            submenu = submenu.item(
                                PopupMenuItem::new(candidate.label())
                                    .checked(candidate == text_width)
                                    .action(action),
                            );
                        }
                        submenu.action_context(focus.clone())
                    });
                    menu.separator()
                        .item(
                            PopupMenuItem::new("Outline")
                                .checked(outline)
                                .action(Box::new(ToggleOutline)),
                        )
                        .item(PopupMenuItem::submenu("Preview Theme", themes))
                        .item(PopupMenuItem::submenu("Text Width", widths))
                        .separator()
                        .menu("Next Tab", Box::new(NextTab))
                        .menu("Previous Tab", Box::new(PreviousTab))
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
    /// middle, the outline and Write / Read switches and the window
    /// controls on the right (the last added by `TitleBar`).
    fn render_title_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = self.tab().label();
        let muted = cx.theme().muted_foreground;
        let menu_bar = self.menu_bar_shown().then(|| self.render_menu_bar(cx));
        let outline_button = Button::new("outline-toggle")
            .label("Outline")
            .ghost()
            .xsmall()
            .tab_stop(false)
            .selected(self.outline_shown())
            .on_click(cx.listener(|this, _, window, cx| {
                this.toggle_outline(&ToggleOutline, window, cx);
            }));
        // The one switch between reading and editing that needs no key.
        let mode_button = Button::new("edit-toggle")
            .label(if self.editing { "Read" } else { "Write" })
            .ghost()
            .xsmall()
            .tab_stop(false)
            .on_click(cx.listener(|this, _, window, cx| {
                this.set_editing(!this.editing, window, cx);
            }));
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
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .pr_2()
                            .gap_1()
                            // The title bar starts a window move on mouse
                            // down plus any movement, and the move swallows
                            // the mouse up a click needs; the press stays here.
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(outline_button)
                            .child(mode_button),
                    ),
            )
            .into_any_element()
    }

    /// The tabs, once there is more than one: a click shows one, its ×
    /// closes it.
    fn render_tab_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.tabs.len() < 2 {
            return None;
        }
        let this = cx.weak_entity();
        let muted = cx.theme().muted_foreground;
        let tabs = self.tabs.iter().enumerate().map(|(ix, tab)| {
            let this = this.clone();
            let close = div()
                .id(("close-tab", tab.id as usize))
                .ml_1()
                .px_1()
                .rounded_sm()
                .text_color(muted)
                .hover(|style| style.bg(gpui_kit::black().opacity(0.08)))
                .child("×")
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, window, cx| {
                    let _ = this.update(cx, |this, cx| this.close_tab(ix, window, cx));
                });
            Tab::new().label(tab.label()).suffix(close)
        });
        Some(
            TabBar::new("tabs")
                .underline()
                // The first tab clear of the window's edge.
                .prefix(div().w_2())
                .small()
                .menu(true)
                .max_width(px(220.))
                .selected_index(self.active)
                .on_click({
                    let this = this.clone();
                    move |ix: &usize, window, cx| {
                        let _ = this.update(cx, |this, cx| this.activate(*ix, window, cx));
                    }
                })
                .children(tabs)
                .into_any_element(),
        )
    }

    /// Re-parse tab `id`'s preview once typing pauses. Each call restarts
    /// the wait.
    fn schedule_preview_refresh(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let task = cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(PREVIEW_DEBOUNCE).await;
            let Ok(Some(text)) = this.update(cx, |this, _| {
                let tab = this.tab_by_id(id)?;
                (tab.previewed != tab.source).then(|| tab.source.clone())
            }) else {
                return;
            };
            // The block table is a whole-document parse, as costly as the
            // preview's own (which the preview runs off the UI thread too).
            let (blocks, headings) = cx
                .background_spawn({
                    let text = text.clone();
                    async move { highlight::outline(&text) }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.show_preview(id, text, blocks, headings, cx)
            });
        });
        if let Some(tab) = self.tab_by_id_mut(id) {
            tab.preview_refresh = Some(task);
        }
    }

    /// Hand `text` to tab `id`'s preview, with `blocks` its block table and
    /// `headings` its outline.
    fn show_preview(
        &mut self,
        id: TabId,
        text: SharedString,
        blocks: Vec<usize>,
        headings: Vec<Heading>,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tab_by_id_mut(id) else {
            return;
        };
        tab.previewed = text.clone();
        tab.blocks = blocks;
        tab.headings = headings;
        tab.preview
            .update(cx, |state, cx| state.set_text(&text, cx));
        tab.scroll_sync = None;
        cx.notify();
    }

    /// Point tab `id`'s preview and rendered view at its current source in
    /// `format`, right away. The block table follows a moment later; until
    /// then the scroll sync goes by proportion.
    fn set_preview(
        &mut self,
        id: TabId,
        format: PreviewFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tab_by_id_mut(id) else {
            return;
        };
        tab.preview_refresh = None;
        let text = tab.source.clone();
        if format != tab.format {
            tab.format = format;
            tab.preview = cx.new(|cx| preview_state(format, &text, cx));
            tab.rendered
                .clone()
                .update(cx, |rendered, cx| rendered.set_format(format, window, cx));
        }
        let headings = Vec::new();
        self.show_preview(id, text, Vec::new(), headings, cx);
        self.schedule_outline(id, window, cx);
    }

    /// Compute the block table and the outline for what tab `id`'s preview
    /// shows, off the UI thread, and adopt them if the preview still shows
    /// that text.
    fn schedule_outline(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.tab_by_id(id).map(|tab| tab.previewed.clone()) else {
            return;
        };
        let task = cx.spawn_in(window, async move |this, cx| {
            let (blocks, headings) = cx
                .background_spawn({
                    let text = text.clone();
                    async move { highlight::outline(&text) }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if let Some(tab) = this.tab_by_id_mut(id)
                    && tab.previewed == text
                {
                    tab.blocks = blocks;
                    tab.headings = headings;
                    tab.scroll_sync = None;
                    cx.notify();
                }
            });
        });
        if let Some(tab) = self.tab_by_id_mut(id) {
            tab.preview_refresh = Some(task);
        }
    }

    /// The preview item holding top-level block `block`: the block itself
    /// while the preview has one item per block, else by proportion of
    /// `line` in `lines` (mid-parse, or split differently).
    fn preview_item(
        tab: &DocumentTab,
        list: &ListState,
        block: usize,
        line: usize,
        lines: usize,
    ) -> Option<usize> {
        let count = list.item_count();
        if count == 0 {
            return None;
        }
        Some(if count == tab.blocks.len() {
            block.min(count - 1)
        } else {
            (count * line / lines.max(1)).min(count - 1)
        })
    }

    /// Scroll the preview to the block that holds the editor's top line.
    /// Editor scrolling drives the preview; the preview never drives the editor.
    fn sync_preview_scroll(&mut self, cx: &mut Context<Self>) {
        let ix = self.active;
        let Some(top) = top_visible_line(self.tabs[ix].editor.read(cx)) else {
            return;
        };
        let tab = &mut self.tabs[ix];
        if tab.scroll_sync == Some(top) {
            return;
        }
        tab.scroll_sync = Some(top);
        let tab = &self.tabs[ix];
        let list = tab.preview.read(cx).list_state().clone();
        // The last block starting at or above `top`.
        let block = tab
            .blocks
            .partition_point(|&start| start <= top)
            .saturating_sub(1);
        let lines = tab.editor.read(cx).text().lines_len();
        if let Some(item_ix) = Self::preview_item(tab, &list, block, top, lines) {
            list.scroll_to(ListOffset {
                item_ix,
                offset_in_item: px(0.),
            });
        }
    }

    /// Bring heading `ix` of the outline to the top of whatever shows the
    /// document: the page being read, the editor (the split's preview
    /// follows it), or the rendered blocks.
    pub fn jump_to_heading(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let tab = self.tab();
        let Some(heading) = tab.headings.get(ix).cloned() else {
            return;
        };
        match (self.editing, self.layout()) {
            (false, _) => {
                let list = tab.preview.read(cx).list_state().clone();
                let lines = tab.previewed.lines().count();
                if let Some(item_ix) =
                    Self::preview_item(tab, &list, heading.block, heading.line, lines)
                {
                    list.scroll_to(ListOffset {
                        item_ix,
                        offset_in_item: px(0.),
                    });
                }
                self.reading_focus.focus(window, cx);
            }
            (true, ViewMode::Rendered) => {
                tab.rendered.clone().update(cx, |rendered, cx| {
                    rendered.scroll_to_block(heading.block, window, cx)
                });
            }
            (true, _) => {
                let editor = tab.editor.clone();
                let line = heading
                    .line
                    .min(editor.read(cx).text().lines_len().saturating_sub(1));
                editor.update(cx, |state, cx| {
                    state.set_cursor_position(Position::new(line as u32, 0), window, cx);
                });
                // The cursor move scrolls the line into view, at whatever
                // edge it came from. Two frames on, the line is laid out
                // and the editor scrolls it up to the top.
                window.on_next_frame(move |window, _| {
                    window.on_next_frame(move |_, cx| {
                        editor.update(cx, |state, cx| {
                            let offset = state.text().line_start_offset(line);
                            let Some(bounds) = state.range_to_bounds(&(offset..offset)) else {
                                return;
                            };
                            let below = bounds.top() - state.input_bounds().top();
                            let scroll = state.scroll_offset();
                            state.set_scroll_offset(point(scroll.x, scroll.y - below), cx);
                        });
                    });
                });
            }
        }
        cx.notify();
    }

    /// The column of text while reading, or `None` for the whole pane.
    fn reading_column(&self, window: &Window) -> Option<Pixels> {
        let rems = self.settings.text_width.rems()?;
        Some(window.rem_size() * rems)
    }

    /// The paper either side of the text while reading: what centres the
    /// column in the pane (the window less the outline), never less than
    /// `theme::PAGE_MARGIN`.
    fn reading_margin(&self, window: &Window) -> Pixels {
        let Some(column) = self.reading_column(window) else {
            return theme::PAGE_MARGIN;
        };
        let outline = if self.outline_shown() {
            OUTLINE_WIDTH
        } else {
            OUTLINE_STRIP
        };
        let pane = window.viewport_size().width - outline;
        ((pane - column) / 2.).max(theme::PAGE_MARGIN)
    }

    pub fn text_width(&self) -> TextWidth {
        self.settings.text_width
    }

    fn text_narrow(&mut self, _: &TextNarrow, _: &mut Window, cx: &mut Context<Self>) {
        self.set_text_width(TextWidth::Narrow, cx);
    }

    fn text_medium(&mut self, _: &TextMedium, _: &mut Window, cx: &mut Context<Self>) {
        self.set_text_width(TextWidth::Medium, cx);
    }

    fn text_wide(&mut self, _: &TextWide, _: &mut Window, cx: &mut Context<Self>) {
        self.set_text_width(TextWidth::Wide, cx);
    }

    fn text_full(&mut self, _: &TextFull, _: &mut Window, cx: &mut Context<Self>) {
        self.set_text_width(TextWidth::Full, cx);
    }

    /// Run the text this wide while reading; remembered.
    pub fn set_text_width(&mut self, width: TextWidth, cx: &mut Context<Self>) {
        if self.settings.text_width != width {
            self.settings.text_width = width;
            self.save_settings();
            cx.notify();
        }
    }
    /// The heading the document is at: the last one at or above the top of
    /// what shows it.
    fn current_heading(&self, cx: &App) -> Option<usize> {
        let tab = self.tab();
        if tab.headings.is_empty() {
            return None;
        }
        let at_or_above = |above: &dyn Fn(&Heading) -> bool| tab.headings.iter().rposition(above);
        match self.layout() {
            ViewMode::Source => {
                let top = top_visible_line(tab.editor.read(cx))?;
                at_or_above(&|heading| heading.line <= top)
            }
            ViewMode::Rendered if self.editing => {
                let top = tab.rendered.read(cx).top_block();
                at_or_above(&|heading| heading.block <= top)
            }
            _ => {
                let list = tab.preview.read(cx).list_state();
                if list.item_count() != tab.blocks.len() {
                    return None;
                }
                let top = list.logical_scroll_top().item_ix;
                at_or_above(&|heading| heading.block <= top)
            }
        }
    }

    /// The outline: the document's headings, indented by level, the one the
    /// document is at marked. A click goes there.
    fn render_outline(&self, cx: &mut Context<Self>) -> AnyElement {
        let current = self.current_heading(cx);
        let theme = cx.theme();
        let (foreground, muted, accent, hover, border, background) = (
            theme.foreground,
            theme.muted_foreground,
            theme.link,
            theme.muted,
            theme.border,
            theme.title_bar,
        );
        let headings = &self.tab().headings;
        let entries = headings.iter().enumerate().map(|(ix, heading)| {
            let marked = current == Some(ix);
            div()
                .id(("heading", ix))
                .w_full()
                .pl(px(16. + 12. * f32::from(heading.level.saturating_sub(1))))
                .pr_3()
                .py(px(3.))
                .text_sm()
                .truncate()
                .cursor_pointer()
                .text_color(if marked {
                    accent
                } else if heading.level <= 2 {
                    foreground
                } else {
                    muted
                })
                .hover(move |style| style.bg(hover))
                .child(SharedString::from(heading.title.clone()))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.jump_to_heading(ix, window, cx);
                }))
        });
        // Its head: the name, and the fold that puts it away.
        let head = h_flex()
            .w_full()
            .pl_4()
            .pr_2()
            .pb_2()
            .justify_between()
            .text_xs()
            .text_color(muted)
            .child("OUTLINE")
            .child(
                div()
                    .id("fold-outline")
                    .px_1()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .child("‹")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_outline(&ToggleOutline, window, cx);
                    })),
            );
        v_flex()
            .id("outline")
            .w(OUTLINE_WIDTH)
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .py_3()
            .bg(background)
            .border_r_1()
            .border_color(border)
            .child(head)
            .when(headings.is_empty(), |this| {
                this.child(
                    div()
                        .px_4()
                        .text_sm()
                        .text_color(muted)
                        .child("No headings yet."),
                )
            })
            .children(entries)
            .into_any_element()
    }

    /// The outline folded away: a strip along the edge, a click unfolds it.
    fn render_outline_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let (muted, hover, border, background) = (
            theme.muted_foreground,
            theme.muted,
            theme.border,
            theme.title_bar,
        );
        div()
            .id("unfold-outline")
            .w(OUTLINE_STRIP)
            .flex_shrink_0()
            .h_full()
            .pt_3()
            .flex()
            .justify_center()
            .bg(background)
            .border_r_1()
            .border_color(border)
            .text_xs()
            .text_color(muted)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child("›")
            .on_click(cx.listener(|this, _, window, cx| {
                this.toggle_outline(&ToggleOutline, window, cx);
            }))
            .into_any_element()
    }
    /// The window title for a document: `● name — smep` while dirty.
    pub fn title_for(path: Option<&Path>, dirty: bool) -> String {
        let marker = if dirty { "● " } else { "" };
        format!("{marker}{} — smep", Document::display_name(path))
    }

    /// Whether any open document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.tabs.iter().any(DocumentTab::is_dirty)
    }

    fn refresh_title(&self, window: &mut Window) {
        let tab = self.tab();
        window.set_window_title(&Self::title_for(
            tab.named_path().as_deref(),
            tab.is_dirty(),
        ));
    }

    /// Replace the active tab's buffer with `document`, as after Open or
    /// Import into a blank tab.
    fn load(&mut self, document: Document, window: &mut Window, cx: &mut Context<Self>) {
        let ix = self.active;
        let tab = &mut self.tabs[ix];
        let id = tab.id;
        tab.path = document.path;
        tab.origin = document.origin;
        tab.source = document.text.clone().into();
        tab.saved = if tab.origin.is_some() {
            SharedString::default()
        } else {
            tab.source.clone()
        };
        // `set_value` emits no change event, so `source` is set by hand above
        // and the rendered view is told that its block is gone.
        let (rendered, editor) = (tab.rendered.clone(), tab.editor.clone());
        rendered.update(cx, |rendered, cx| rendered.deactivate(window, cx));
        editor.update(cx, |state, cx| state.set_value(document.text, window, cx));
        self.set_preview(id, document.format, window, cx);
        self.refresh_title(window);
    }

    /// Show `document`: in the active tab when that is blank, else in a new
    /// tab.
    fn place(&mut self, document: Document, window: &mut Window, cx: &mut Context<Self>) {
        if self.tab().is_pristine() {
            self.load(document, window, cx);
            self.show_active(window, cx);
        } else {
            let ix = self.push_tab(document, window, cx);
            self.activate(ix, window, cx);
        }
    }

    /// Write tab `id`'s buffer to `path` and adopt it as the document's path.
    fn write_to(
        &mut self,
        id: TabId,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(tab) = self.tab_by_id(id) else {
            return false;
        };
        let text = tab.editor.read(cx).value();
        match io::write(&path, &text) {
            Ok(()) => {
                let tab = self.tab_by_id_mut(id).expect("looked up above");
                tab.saved = text.clone();
                tab.source = text;
                let format = PreviewFormat::for_path(&path);
                let changed = format != tab.format;
                tab.path = Some(path);
                if changed {
                    self.set_preview(id, format, window, cx);
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

    /// Save tab `id` to its path, or ask for one. Resolves to whether it
    /// saved.
    fn save_task(&mut self, id: TabId, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        match self.tab_by_id(id).map(|tab| tab.path.clone()) {
            Some(Some(path)) => Task::ready(self.write_to(id, path, window, cx)),
            Some(None) => self.save_as_task(id, window, cx),
            None => Task::ready(false),
        }
    }

    /// Ask for a path, then save tab `id` there. Resolves to whether it
    /// saved.
    fn save_as_task(
        &mut self,
        id: TabId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<bool> {
        let Some(tab) = self.tab_by_id(id) else {
            return Task::ready(false);
        };
        let named = tab.named_path();
        let directory = named
            .as_deref()
            .and_then(Path::parent)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let suggested = named
            .as_deref()
            .map(|path| Document::display_name(Some(path)))
            .unwrap_or_else(|| "untitled.md".to_string());
        let chosen = cx.prompt_for_new_path(&directory, Some(&suggested));

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = chosen.await else {
                return false;
            };
            this.update_in(cx, |this, window, cx| {
                // One tab per file: two would overwrite each other.
                if this.tabs.iter().any(|tab| tab.id != id && tab.holds(&path)) {
                    this.report(
                        &format!("{} is open in another tab", path.display()),
                        "Close that tab first, or save under another name.",
                        window,
                        cx,
                    );
                    return false;
                }
                this.write_to(id, path, window, cx)
            })
            .unwrap_or(false)
        })
    }

    /// Ask before discarding the active tab's unsaved changes. Resolves to
    /// whether to proceed.
    fn confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        let tab = self.tab();
        if !tab.is_dirty() {
            return Task::ready(true);
        }
        let id = tab.id;
        let detail = format!(
            "{} has unsaved changes.",
            Document::display_name(tab.named_path().as_deref())
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "Save changes?",
            Some(&detail),
            &["Save", "Don't Save", "Cancel"],
            cx,
        );

        cx.spawn_in(window, async move |this, cx| match answer.await {
            Ok(0) => match this.update_in(cx, |this, window, cx| this.save_task(id, window, cx)) {
                Ok(saved) => saved.await,
                Err(_) => false,
            },
            Ok(1) => true,
            _ => false,
        })
    }

    /// Ask about every tab with unsaved changes, one after the other, each
    /// shown while it is asked about. Resolves to whether to proceed: false
    /// as soon as one is cancelled (or fails to save).
    fn confirm_discard_all(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Task<bool> {
        let dirty: Vec<TabId> = self
            .tabs
            .iter()
            .filter(|tab| tab.is_dirty())
            .map(|tab| tab.id)
            .collect();
        cx.spawn_in(window, async move |this, cx| {
            for id in dirty {
                let Ok(proceed) = this.update_in(cx, |this, window, cx| match this.tab_index(id) {
                    Some(ix) => {
                        this.activate(ix, window, cx);
                        this.confirm_discard(window, cx)
                    }
                    None => Task::ready(true),
                }) else {
                    return false;
                };
                if !proceed.await {
                    return false;
                }
            }
            true
        })
    }

    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                for path in paths {
                    this.load_path(path, window, cx);
                }
            });
        })
        .detach();
    }

    /// Open the files at `paths`, each in a tab (a blank tab is used up
    /// first, a file already open is shown); none opens an empty tab. This
    /// is what a double-click in the file manager, and `smep FILE` while
    /// smep runs, end up in.
    pub fn open_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        if paths.is_empty() {
            if !self.tab().is_pristine() {
                self.new_tab(window, cx);
            }
        } else {
            for path in paths {
                self.load_path(path, window, cx);
            }
        }
        window.activate_window();
    }

    /// Show `path`: its tab if it is open, else read into a tab (a file
    /// that does not exist yet is a new document that saves there). A PDF,
    /// Word or text file is converted into a new Markdown document instead.
    fn load_path(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.tabs.iter().position(|tab| tab.holds(&path)) {
            self.activate(ix, window, cx);
            return;
        }
        if let Some(format) = convert::Format::for_import(&path) {
            self.import_path(path, format, window, cx);
            return;
        }
        match Document::open(path.clone()) {
            Ok(document) => self.place(document, window, cx),
            Err(err) => self.report(
                &format!("Could not open {}", path.display()),
                &err.to_string(),
                window,
                cx,
            ),
        }
    }

    /// Convert `path` off the UI thread (a PDF can take a while), then
    /// show the Markdown as a new, unsaved document.
    fn import_path(
        &mut self,
        path: PathBuf,
        format: convert::Format,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let converted = cx
                .background_spawn({
                    let path = path.clone();
                    async move { Document::import(path, format) }
                })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match converted {
                Ok(document) => this.place(document, window, cx),
                Err(err) => this.report(
                    &format!("Could not import {}", path.display()),
                    &format!("{err:#}"),
                    window,
                    cx,
                ),
            });
        })
        .detach();
    }

    /// Pick a file to convert to Markdown: PDF, Word, HTML or text.
    fn import(&mut self, _: &Import, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = chosen.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = this.update_in(cx, |this, window, cx| {
                match convert::Format::for_explicit_import(&path) {
                    Some(format) => this.import_path(path, format, window, cx),
                    None => this.report(
                        &format!("Cannot import {}", path.display()),
                        "smep converts PDF (.pdf), Word (.docx), HTML (.html) and text (.txt) files.",
                        window,
                        cx,
                    ),
                }
            });
        })
        .detach();
    }

    /// Ask where to put an export of the document with `extension`: next to
    /// it, named after it. Resolves to the chosen path.
    fn ask_export_path(&self, extension: &str, cx: &mut Context<Self>) -> Task<Option<PathBuf>> {
        let named = self.tab().named_path();
        let directory = named
            .as_deref()
            .and_then(Path::parent)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let stem = named
            .as_deref()
            .and_then(Path::file_stem)
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled".to_string());
        let chosen = cx.prompt_for_new_path(&directory, Some(&format!("{stem}.{extension}")));
        cx.background_spawn(async move {
            match chosen.await {
                Ok(Ok(Some(path))) => Some(path),
                _ => None,
            }
        })
    }

    /// Typeset the document as a PDF (off the UI thread; Typst takes a
    /// moment) and write it where the user asks.
    fn export_pdf(&mut self, _: &ExportPdf, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = self.ask_export_path("pdf", cx);
        let text = self.tab().editor.read(cx).value();
        let base = self.tab().document_dir();
        // Print is ink on paper: the light theme of the preview's family,
        // unless the user wrote a preamble of their own.
        let preamble = export::user_preamble(self.settings.dir())
            .unwrap_or_else(|| export::preamble(&self.preview_theme().print_palette()));
        cx.spawn_in(window, async move |this, cx| {
            let Some(path) = chosen.await else {
                return;
            };
            let written = cx
                .background_spawn({
                    let path = path.clone();
                    async move {
                        let pdf = export::to_pdf(&text, base.as_deref(), &preamble)?;
                        std::fs::write(&path, pdf)?;
                        anyhow::Ok(())
                    }
                })
                .await;
            if let Err(err) = written {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.report(
                        &format!("Could not export {}", path.display()),
                        &format!("{err:#}"),
                        window,
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    /// The document as a standalone HTML page, written where the user asks.
    fn export_html(&mut self, _: &ExportHtml, window: &mut Window, cx: &mut Context<Self>) {
        let chosen = self.ask_export_path("html", cx);
        let text = self.tab().editor.read(cx).value();
        let title = export::first_heading(&text)
            .unwrap_or_else(|| Document::display_name(self.tab().named_path().as_deref()));
        // The page wears the preview's theme, dark or light, unless the
        // user wrote a stylesheet of their own.
        let stylesheet = export::user_stylesheet(self.settings.dir())
            .unwrap_or_else(|| export::stylesheet(&self.preview_theme().resolve(cx.theme())));
        cx.spawn_in(window, async move |this, cx| {
            let Some(path) = chosen.await else {
                return;
            };
            if let Err(err) = std::fs::write(&path, export::to_html(&text, &title, &stylesheet)) {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.report(
                        &format!("Could not export {}", path.display()),
                        &err.to_string(),
                        window,
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    /// Become the system's program for Markdown files, as far as it allows,
    /// and say what happened (or what is left to do).
    fn make_default(&mut self, _: &MakeDefault, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let outcome = cx
                .background_spawn(async { defaults::make_default() })
                .await;
            let _ = this.update_in(cx, |this, window, cx| match outcome {
                Ok(outcome) => {
                    let title = if outcome.done {
                        "smep is the default for Markdown"
                    } else {
                        "One step left"
                    };
                    let answered = window.prompt(
                        PromptLevel::Info,
                        title,
                        Some(&outcome.message),
                        &["OK"],
                        cx,
                    );
                    cx.spawn(async move |_, _| {
                        let _ = answered.await;
                    })
                    .detach();
                }
                Err(err) => this.report(
                    "Could not make smep the default for Markdown",
                    &format!("{err:#}"),
                    window,
                    cx,
                ),
            });
        })
        .detach();
    }

    /// Files dropped on the window: each opens in a tab (a PDF, Word or text
    /// file is converted).
    fn on_drop(&mut self, paths: &ExternalPaths, window: &mut Window, cx: &mut Context<Self>) {
        for path in paths.paths() {
            self.load_path(path.clone(), window, cx);
        }
    }

    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.tab().id;
        self.save_task(id, window, cx).detach();
    }

    fn save_as(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.tab().id;
        self.save_as_task(id, window, cx).detach();
    }

    /// A lone `/` on the cursor's line, with the cursor after it.
    fn slash_typed(state: &EditorState) -> bool {
        insert::cursor_line_trimmed(state) == "/"
            && state.cursor() == insert::cursor_line_range(state).end
    }

    /// The editor the user is typing in: the active block in the rendered
    /// view, otherwise the source editor.
    fn active_editor(&self, cx: &App) -> ActiveEditor {
        if self.layout() == ViewMode::Rendered
            && let Some(editor) = self.tab().rendered.read(cx).active_editor()
        {
            return ActiveEditor::Block(editor);
        }
        ActiveEditor::Source(self.tab().editor.clone())
    }

    /// Window-space bounds of the cursor's line, once the editor has laid out.
    fn cursor_line_bounds(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.active_editor(cx).cursor_line_bounds(cx)
    }

    /// Where a menu's actions are dispatched from, and where focus returns
    /// when it closes: the editor being typed in, or the rendered view while
    /// no block is being edited (the source editor is not on screen then,
    /// and an action sent through its focus handle would go nowhere).
    fn menu_focus(&self, cx: &App) -> FocusHandle {
        if !self.editing {
            return self.reading_focus.clone();
        }
        match self.active_editor(cx) {
            ActiveEditor::Source(_) if self.layout() == ViewMode::Rendered => {
                self.tab().rendered.read(cx).focus_handle().clone()
            }
            editor => editor.focus_handle(cx),
        }
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
        let editor_focus = self.menu_focus(cx);
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
                this.tab().editor.clone().update(cx, |state, cx| {
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
        let state = self.tab().editor.read(cx);
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
        let palette = self.preview_theme().resolve(cx.theme());
        let style = palette.text_view_style(cx.theme().font_size);
        let tab = self.tab();
        let base = tab.document_dir();
        let editing = self.editing;
        // Reading an empty document: a hint in place of the (empty) page.
        let hint = (!editing && tab.previewed.trim().is_empty()).then(|| {
            div()
                .text_color(palette.muted)
                .child("Nothing to read yet. Write (the button in the title bar) to start.")
        });
        // The one rendering of the whole document: the split view's preview
        // while editing, the page being read otherwise.
        // The paper fills the pane and the text runs in a column of the
        // chosen width, centred by the paper either side; the list, and its
        // scrollbar, span the pane.
        let margin = if editing {
            px(24.)
        } else {
            self.reading_margin(window)
        };
        let body = match hint {
            Some(hint) => div().px(margin).child(hint).into_any_element(),
            None => TextView::new(&tab.preview)
                .scrollable(true)
                .style(style)
                .image_source(rendered::image_resolver(base.clone()))
                .code_block_actions(rendered::code_block_actions(palette))
                .px(margin)
                .into_any_element(),
        };
        let preview_list = tab.preview.read(cx).list_state().clone();
        let preview = div()
            .id("preview")
            .size_full()
            .overflow_hidden()
            .bg(palette.background)
            .text_color(palette.foreground)
            .when_some(palette.font_family(), |this, family| {
                this.font_family(family)
            })
            // The text set loose: the blog's leading.
            .line_height(relative(theme::READING_LEADING))
            .py_4()
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.open_theme_menu(event.position, window, cx);
                }),
            )
            // The wheel over the paper beside the text scrolls the text, as
            // anywhere in a reader's window.
            .on_scroll_wheel(move |event: &ScrollWheelEvent, window, cx| {
                if preview_list.viewport_bounds().contains(&event.position) {
                    return;
                }
                let delta = event.delta.pixel_delta(window.line_height());
                preview_list.scroll_by(-delta.y);
                cx.refresh_windows();
            })
            // Only an editor on screen has a top line to follow.
            .when(editing, |this| this.on_prepaint(follow_editor))
            .child(body); // Right-click: the editor moves the cursor to the click, then hands
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
                Editor::new(&tab.editor)
                    .bordered(false)
                    .size_full()
                    .pl(px(28.))
                    .context_menu(context_menu),
            );
        let rendered_view = tab.rendered.clone();

        let plus = self.render_plus(window, cx);
        let menu = self.render_insert_menu(window, cx);
        let title_bar = self.render_title_bar(cx);
        let tab_bar = self.render_tab_bar(cx);
        let outline = if self.outline_shown() {
            self.render_outline(cx)
        } else {
            self.render_outline_strip(cx)
        };

        let content = if !editing {
            // Reading: the document as one page, scrolled by the keys from
            // the pane (or from the text inside it, which takes the focus
            // for selection).
            div()
                .id("reading")
                .size_full()
                .track_focus(&self.reading_focus)
                .on_key_down(cx.listener(Self::on_reading_key))
                .child(preview)
                .into_any_element()
        } else {
            match self.layout() {
                ViewMode::Source => editor.into_any_element(),
                ViewMode::Rendered => {
                    let measure = self.reading_column(window);
                    rendered_view.update(cx, |rendered, cx| {
                        rendered.set_palette(palette, cx);
                        rendered.set_base(base.clone(), cx);
                        rendered.set_measure(measure, cx);
                    });
                    // Right-click picks the theme here as on the split
                    // preview (an open block keeps its own menu and stops
                    // the event).
                    div()
                        .id("rendered-pane")
                        .size_full()
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                this.open_theme_menu(event.position, window, cx);
                            }),
                        )
                        .child(rendered_view)
                        .into_any_element()
                }
                ViewMode::Split => h_resizable("smep-split")
                    .child(resizable_panel().child(editor.into_any_element()))
                    .child(resizable_panel().child(preview.into_any_element()))
                    .into_any_element(),
            }
        };

        let drop_highlight = cx.theme().ring;
        window_border().child(
            v_flex()
                .size_full()
                .key_context(keymap::CONTEXT)
                .track_focus(&self.focus_handle)
                // Files from the desktop: a border while they hover, open on drop.
                .drag_over::<ExternalPaths>(move |style, _, _, _| {
                    style.border_2().border_color(drop_highlight)
                })
                .on_drop(cx.listener(Self::on_drop))
                .on_action(cx.listener(Self::open))
                .on_action(cx.listener(Self::import))
                .on_action(cx.listener(Self::export_pdf))
                .on_action(cx.listener(Self::export_html))
                .on_action(cx.listener(Self::make_default))
                .on_action(cx.listener(Self::save))
                .on_action(cx.listener(Self::save_as))
                .on_action(cx.listener(Self::quit))
                .on_action(cx.listener(Self::view_source))
                .on_action(cx.listener(Self::view_split))
                .on_action(cx.listener(Self::view_rendered))
                .on_action(cx.listener(Self::toggle_editing))
                .on_action(cx.listener(Self::toggle_menu_bar))
                .on_action(cx.listener(Self::toggle_outline))
                .on_action(cx.listener(Self::text_narrow))
                .on_action(cx.listener(Self::text_medium))
                .on_action(cx.listener(Self::text_wide))
                .on_action(cx.listener(Self::text_full))
                .on_action(cx.listener(Self::toggle_fullscreen))
                .on_action(cx.listener(Self::new_tab_action))
                .on_action(cx.listener(Self::close_tab_action))
                .on_action(cx.listener(Self::next_tab))
                .on_action(cx.listener(Self::previous_tab))
                .child(title_bar)
                .children(tab_bar)
                .child(
                    h_flex()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .child(outline)
                        .child(div().flex_1().min_w_0().h_full().child(content)),
                )
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
    use crate::settings::{Settings, TextWidth, ViewMode};
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
                origin: None,
            },
        )
    }

    fn open_document(
        cx: &mut TestAppContext,
        document: Document,
    ) -> (Entity<Smep>, &mut VisualTestContext) {
        open_with(cx, document, Settings::default())
    }

    /// A window in editing mode (what most tests exercise); smep itself
    /// starts reading, see `open_reading`.
    fn open_with(
        cx: &mut TestAppContext,
        document: Document,
        settings: Settings,
    ) -> (Entity<Smep>, &mut VisualTestContext) {
        let (smep, cx) = open_reading(cx, document, settings);
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.set_editing(true, window, cx));
        });
        (smep, cx)
    }

    /// A window as smep opens it: reading.
    fn open_reading(
        cx: &mut TestAppContext,
        document: Document,
        settings: Settings,
    ) -> (Entity<Smep>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::theme::install(cx);
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
        cx.read(|cx| smep.read(cx).tab().editor.clone())
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

        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().source.to_string()),
            "# Hello"
        );
    }

    #[gpui_kit::test]
    fn the_preview_follows_after_a_pause_in_typing(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        type_text(&editor, "# One", cx);
        type_text(&editor, "# One two", cx);
        assert_eq!(cx.read(|cx| smep.read(cx).tab().previewed.to_string()), "");

        cx.executor().advance_clock(PREVIEW_DEBOUNCE);
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().previewed.to_string()),
            "# One two"
        );
    }

    #[gpui_kit::test]
    fn scrolling_the_editor_scrolls_the_preview(cx: &mut TestAppContext) {
        let text: String = (1..=400).map(|n| format!("Paragraph {n}\n\n")).collect();
        let (smep, cx) = open(cx, &text);
        let editor = editor(&smep, cx);
        draw(cx);
        let list = cx.read(|cx| smep.read(cx).tab().preview.read(cx).list_state().clone());
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
        let top_line = cx.read(|cx| smep.read(cx).tab().scroll_sync);
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
            cx.read(|cx| smep.read(cx).tab().source.to_string()),
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
            cx.read(|cx| Smep::title_for(smep.read(cx).tab().path.as_deref(), true)),
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
        let saved = cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.save_task(this.tab().id, window, cx))
        });
        cx.simulate_new_path_selection({
            let target = target.clone();
            move |_| Some(target)
        });

        assert!(saved.await);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "fresh");
        assert!(!dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), Some(target));
    }

    #[gpui_kit::test]
    async fn cancelling_save_as_leaves_the_document_dirty(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        type_text(&editor, "fresh", cx);

        let saved = cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.save_task(this.tab().id, window, cx))
        });
        cx.simulate_new_path_selection(|_| None);

        assert!(!saved.await);
        assert!(dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), None);
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
    fn open_fills_a_blank_tab_and_resets_dirty(cx: &mut TestAppContext) {
        let dir = TempDir::new("open");
        let path = dir.path("other.html");
        std::fs::write(&path, "<b>bold</b>").unwrap();
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        cx.update(|window, cx| smep.update(cx, |this, cx| this.open(&Open, window, cx)));
        cx.run_until_parked();
        assert!(cx.did_prompt_for_paths());
        cx.simulate_path_prompt_response({
            let path = path.clone();
            move |_| Some(vec![path])
        });
        cx.run_until_parked();

        assert_eq!(value(&editor, cx), "<b>bold</b>");
        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().previewed.to_string()),
            "<b>bold</b>",
            "open refreshes the preview at once, no debounce"
        );
        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().source.to_string()),
            "<b>bold</b>"
        );
        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().format),
            PreviewFormat::Html
        );
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), Some(path));
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
            origin: None,
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

    #[gpui_kit::test]
    fn opens_reading_and_a_key_or_the_view_menu_starts_editing(cx: &mut TestAppContext) {
        let dir = TempDir::new("reading");
        let settings = Settings {
            path: Some(dir.path("settings.toml")),
            ..Settings::default()
        };
        let document = Document {
            path: None,
            text: "# Title\n\npara".into(),
            format: PreviewFormat::Markdown,
            origin: None,
        };
        let (smep, cx) = open_reading(cx, document, settings);
        draw(cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        let reading = |cx: &VisualTestContext| {
            cx.read(|cx| {
                let this = smep.read(cx);
                (
                    !this.is_editing(),
                    this.layout(),
                    this.tab().rendered.read(cx).is_read_only(),
                )
            })
        };
        assert_eq!(reading(cx), (true, ViewMode::Rendered, true));
        assert_eq!(cx.read(|cx| smep.read(cx).view_mode()), ViewMode::Split);
        assert!(
            cx.update(|window, cx| smep.read(cx).reading_focus().is_focused(window)),
            "the reading pane holds the focus while reading"
        );

        // A click on a block does nothing while reading.
        activate_block(&smep, 9..13, cx);
        assert!(cx.read(|cx| rendered.read(cx).active_editor()).is_none());

        // The editing key brings the remembered view mode (split) back.
        cx.simulate_keystrokes(&primary("e"));
        cx.run_until_parked();
        draw(cx);
        assert_eq!(reading(cx), (false, ViewMode::Split, false));
        let source = editor(&smep, cx);
        assert!(
            cx.update(|window, cx| source.read(cx).focus_handle(cx).is_focused(window)),
            "the source editor takes the focus for editing"
        );

        // And back to reading: an edit in progress ends.
        rendered_mode(&smep, cx);
        activate_block(&smep, 9..13, cx);
        assert!(cx.read(|cx| rendered.read(cx).active_editor()).is_some());
        cx.simulate_keystrokes(&primary("e"));
        cx.run_until_parked();
        draw(cx);
        assert_eq!(reading(cx), (true, ViewMode::Rendered, true));
        assert!(cx.read(|cx| rendered.read(cx).active_editor()).is_none());
        assert!(
            cx.update(|window, cx| smep.read(cx).reading_focus().is_focused(window)),
            "back to reading, the pane takes the focus again"
        );

        // A view-mode key while reading means editing in that mode.
        cx.simulate_keystrokes(&primary("1"));
        cx.run_until_parked();
        assert_eq!(reading(cx), (false, ViewMode::Source, false));

        // Reading is not a setting: the file knows the view mode only.
        let saved = std::fs::read_to_string(dir.path("settings.toml")).unwrap();
        assert!(saved.contains("view_mode = \"source\""), "{saved}");
        assert!(!saved.contains("editing"), "{saved}");
    }

    #[gpui_kit::test]
    fn an_html_document_is_one_html_block_in_the_rendered_view(cx: &mut TestAppContext) {
        let dir = TempDir::new("html-rendered");
        let page = dir.path("page.html");
        let html = "<h1>Title</h1>\n<p>Some <b>bold</b> text.</p>\n\n<ul><li>one</li></ul>\n";
        std::fs::write(&page, html).unwrap();
        let document = Document::read(page.clone()).unwrap();
        let settings = Settings::default();
        let (smep, cx) = open_reading(cx, document, settings);
        draw(cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());

        // Editing an .html file in the rendered view: the whole page is one
        // block, rendered as HTML, not Markdown's reading of the tags (which
        // would be several). (Reading shows the preview, which is HTML too.)
        rendered_mode(&smep, cx);
        assert_eq!(
            cx.read(|cx| rendered.read(cx).format()),
            PreviewFormat::Html
        );
        assert_eq!(
            cx.read(|cx| rendered.read(cx).block_ranges()),
            vec![0..html.trim_end().len()]
        );
        assert!(
            crate::highlight::block_ranges(html).len() > 1,
            "Markdown would split it"
        );

        // A Markdown document, in a tab of its own, is in blocks.
        let notes = dir.path("notes.md");
        std::fs::write(&notes, "# T\n\npara\n").unwrap();
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.load_path(notes.clone(), window, cx))
        });
        draw(cx);
        let page_view = rendered;
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        assert_eq!(
            cx.read(|cx| page_view.read(cx).format()),
            PreviewFormat::Html,
            "the page's tab is as it was"
        );
        assert_eq!(
            cx.read(|cx| rendered.read(cx).format()),
            PreviewFormat::Markdown
        );
        assert_eq!(
            cx.read(|cx| rendered.read(cx).block_ranges()),
            vec![0..3, 5..9]
        );

        // Save As to .html while a block is open: the edit ends, one block again.
        rendered_mode(&smep, cx);
        activate_block(&smep, 5..9, cx);
        let saved = cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.save_as_task(this.tab().id, window, cx))
        });
        let target = dir.path("notes.html");
        cx.simulate_new_path_selection({
            let target = target.clone();
            move |_| Some(target)
        });
        cx.run_until_parked();
        assert!(cx.read(|cx| rendered.read(cx).active_editor()).is_none());
        assert_eq!(
            cx.read(|cx| rendered.read(cx).format()),
            PreviewFormat::Html
        );
        assert_eq!(cx.read(|cx| rendered.read(cx).block_ranges()), vec![0..9]);
        drop(saved);
    }

    #[gpui_kit::test]
    fn the_text_runs_in_a_column_and_the_wheel_beside_it_scrolls(cx: &mut TestAppContext) {
        use gpui_kit::{ScrollDelta, ScrollWheelEvent};
        let text: String = (1..=200).map(|n| format!("Paragraph {n}\n\n")).collect();
        let document = Document {
            path: None,
            text,
            format: PreviewFormat::Markdown,
            origin: None,
        };
        let (smep, cx) = open_reading(cx, document, Settings::default());
        draw(cx);
        let (window, rem) =
            cx.update(|window, _| (window.viewport_size().width, window.rem_size()));
        let list = cx.read(|cx| smep.read(cx).reading_list(cx));
        let bounds = list.viewport_bounds();
        // The default column, centred in the pane right of the outline; the
        // paper either side is the list's own margin, so its scrollbar is at
        // the pane's right edge.
        assert_eq!(cx.read(|cx| smep.read(cx).text_width()), TextWidth::Medium);
        let column = rem * 56.;
        assert!(
            window > column + super::OUTLINE_WIDTH,
            "a wide window: {window:?}"
        );
        assert!((bounds.size.width - column).abs() < px(2.), "{bounds:?}");
        let left = bounds.left() - super::OUTLINE_WIDTH;
        let right = window - bounds.right();
        assert!(
            (left - right).abs() < px(2.),
            "centred: {left:?} vs {right:?}"
        );
        assert_eq!(list.logical_scroll_top().item_ix, 0);

        // The wheel over the paper beside the text scrolls the text.
        cx.simulate_event(ScrollWheelEvent {
            position: point(bounds.left() - px(10.), bounds.center().y),
            delta: ScrollDelta::Pixels(point(px(0.), px(-600.))),
            ..Default::default()
        });
        draw(cx);
        assert!(
            list.logical_scroll_top().item_ix > 0,
            "{:?}",
            list.logical_scroll_top()
        );

        // Full width: the text runs to the least margin either side.
        cx.update(|_, cx| smep.update(cx, |this, cx| this.set_text_width(TextWidth::Full, cx)));
        draw(cx);
        let bounds = list.viewport_bounds();
        assert_eq!(
            bounds.left(),
            super::OUTLINE_WIDTH + crate::theme::PAGE_MARGIN
        );
        assert_eq!(window - bounds.right(), crate::theme::PAGE_MARGIN);
        // Narrow, with the outline folded: the strip is all that is left of it.
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.set_text_width(TextWidth::Narrow, cx);
                this.toggle_outline(&crate::keymap::ToggleOutline, window, cx);
            })
        });
        draw(cx);
        let bounds = list.viewport_bounds();
        assert!((bounds.size.width - rem * 44.).abs() < px(2.), "{bounds:?}");
        let left = bounds.left() - super::OUTLINE_STRIP;
        assert!((left - (window - bounds.right())).abs() < px(2.));
    }
    #[gpui_kit::test]
    fn a_click_beside_a_blocks_text_edits_that_block(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "First paragraph.\n\nSecond paragraph.\n");
        rendered_mode(&smep, cx);
        draw(cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        let view = cx.read(|cx| rendered.read(cx).bounds());
        // The text sits in the page's column; left of it, in the first
        // block's row, is still the first block.
        assert!(view.size.width > px(56. * 16. + 100.), "{view:?}");
        mouse_down(
            cx,
            MouseButton::Left,
            f32::from(view.left()) + 30.,
            f32::from(view.top()) + 22.,
        );
        draw(cx);
        let block = cx
            .read(|cx| rendered.read(cx).active_editor())
            .expect("a block is open");
        assert_eq!(value(&block, cx), "First paragraph.");
        assert_eq!(
            value(&editor(&smep, cx), cx),
            "First paragraph.\n\nSecond paragraph.\n",
            "no new block at the end"
        );
    }
    #[gpui_kit::test]
    fn keys_scroll_the_document_while_reading(cx: &mut TestAppContext) {
        let text: String = (1..=200).map(|n| format!("Paragraph {n}\n\n")).collect();
        let document = Document {
            path: None,
            text,
            format: PreviewFormat::Markdown,
            origin: None,
        };
        let (smep, cx) = open_reading(cx, document, Settings::default());
        draw(cx);
        // Reading shows the one preview of the whole document, as one list
        // item per block; the keys move its scroll offset (negative = down).
        let list = cx.read(|cx| smep.read(cx).reading_list(cx));
        assert_eq!(list.item_count(), 200, "one item per paragraph");
        let offset = |cx: &mut VisualTestContext| {
            draw(cx);
            f32::from(list.scroll_px_offset_for_scrollbar().y)
        };
        let viewport = f32::from(list.viewport_bounds().size.height);
        assert!(viewport > 100., "the pane is laid out: {viewport}");
        assert_eq!(offset(cx), 0.);

        cx.simulate_keystrokes("pagedown");
        let after_page = offset(cx);
        assert_eq!(after_page, -(viewport - 40.), "a page less a line");
        cx.simulate_keystrokes("space");
        assert_eq!(offset(cx), 2. * after_page);
        cx.simulate_keystrokes("shift-space pageup");
        assert_eq!(offset(cx), 0.);
        cx.simulate_keystrokes("down down up");
        assert_eq!(offset(cx), -40.);
        cx.simulate_keystrokes("end");
        let end = offset(cx);
        assert!(end < 2. * after_page, "the end is far down: {end}");
        cx.simulate_keystrokes("pagedown");
        assert_eq!(offset(cx), end, "stays at the end");
        cx.simulate_keystrokes("up");
        assert_eq!(offset(cx), end + 40., "and comes back up a line from it");
        cx.simulate_keystrokes("home");
        assert_eq!(offset(cx), 0.);
        cx.simulate_keystrokes("up");
        assert_eq!(offset(cx), 0., "clamped at the top");

        // Ctrl+E is a shortcut, not a scroll: editing starts, the preview in
        // the split keeps its place.
        cx.simulate_keystrokes(&primary("e"));
        cx.run_until_parked();
        assert!(cx.read(|cx| smep.read(cx).is_editing()));

        // A block editor keeps its keys: Space types a space.
        rendered_mode(&smep, cx);
        activate_block(&smep, 0..11, cx);
        let block = active_block(&smep, cx);
        cx.simulate_keystrokes("space end");
        cx.run_until_parked();
        assert_eq!(value(&block, cx), "Paragraph 1 ");
    }

    #[gpui_kit::test]
    fn right_click_on_the_document_being_read_opens_the_theme_menu(cx: &mut TestAppContext) {
        let document = Document {
            path: None,
            text: "# T\n\npara".into(),
            format: PreviewFormat::Markdown,
            origin: None,
        };
        // Reading, as smep opens: the one place a right-click reaches a theme.
        let (smep, cx) = open_reading(cx, document, Settings::default());
        draw(cx);
        assert_eq!(
            cx.read(|cx| smep.read(cx).preview_theme()),
            PreviewTheme::System
        );

        mouse_down(cx, MouseButton::Right, 300., 300.);
        assert!(
            menu_open(&smep, cx),
            "the theme menu opens over the document"
        );
        draw(cx);
        // As on the split preview: the first `down` is System, the second Paper.
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        assert_eq!(
            cx.read(|cx| smep.read(cx).preview_theme()),
            PreviewTheme::Paper
        );
        assert!(!menu_open(&smep, cx));

        // Over an open block the right-click is the block editor's own.
        rendered_mode(&smep, cx);
        activate_block(&smep, 5..9, cx);
        let block = active_block(&smep, cx);
        draw(cx);
        let bounds = cx.read(|cx| block.read(cx).input_bounds());
        mouse_down(
            cx,
            MouseButton::Right,
            f32::from(bounds.left()) + 4.,
            f32::from(bounds.top()) + 4.,
        );
        assert!(!menu_open(&smep, cx), "no theme menu over the block");
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
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        cx.update(|window, cx| {
            rendered.update(cx, |rendered, cx| rendered.activate(range, window, cx))
        });
        draw(cx);
    }

    fn active_block(smep: &Entity<Smep>, cx: &VisualTestContext) -> Entity<TextareaState> {
        cx.read(|cx| smep.read(cx).tab().rendered.read(cx).active_editor())
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
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());

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
        let document = cx.read(|cx| smep.read(cx).tab().editor.clone());
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
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
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
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
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());

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
    async fn importing_a_file_makes_a_new_unsaved_markdown_document(cx: &mut TestAppContext) {
        let dir = TempDir::new("import");
        let source = dir.path("notes.txt");
        std::fs::write(&source, "plain notes\r\n\r\n\r\nmore\r\n").unwrap();
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);

        cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.load_path(source.clone(), window, cx))
        });
        cx.run_until_parked();

        assert_eq!(value(&editor, cx), "plain notes\n\nmore\n");
        assert!(dirty(&smep, cx), "converted text is not on disk yet");
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), None);
        assert_eq!(
            cx.read(|cx| Smep::title_for(smep.read(cx).tab().named_path().as_deref(), true)),
            "● notes.md — smep"
        );

        // Save As suggests notes.md next to the original; saving there
        // makes it the document's own path.
        let saved = cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.save_task(this.tab().id, window, cx))
        });
        let target = dir.path("notes.md");
        cx.simulate_new_path_selection({
            let target = target.clone();
            let expected_dir = dir.0.clone();
            move |directory| {
                assert_eq!(
                    directory, expected_dir,
                    "the dialog opens next to the original"
                );
                Some(target)
            }
        });
        assert!(saved.await);
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "plain notes\n\nmore\n"
        );
        assert!(!dirty(&smep, cx));
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), Some(target));
    }

    #[gpui_kit::test]
    fn dropped_files_open_in_tabs_beside_unsaved_changes(cx: &mut TestAppContext) {
        use gpui_kit::{ExternalPaths, FileDropEvent};

        let dir = TempDir::new("drop");
        let dropped = dir.path("dropped.md");
        std::fs::write(&dropped, "# Dropped").unwrap();
        let second = dir.path("second.md");
        std::fs::write(&second, "second").unwrap();
        let (smep, cx) = open(cx, "");
        let editor = editor(&smep, cx);
        type_text(&editor, "unsaved", cx);
        draw(cx);

        let position = point(px(300.), px(300.));
        let paths = ExternalPaths([dropped.clone(), second.clone()].into_iter().collect());
        cx.simulate_event(FileDropEvent::Entered { position, paths });
        cx.simulate_event(FileDropEvent::Submit { position });
        cx.run_until_parked();

        assert!(
            !cx.has_pending_prompt(),
            "the unsaved edit stays in its tab"
        );
        assert_eq!(value(&editor, cx), "unsaved");
        assert_eq!(tabs(&smep, cx), (3, 2));
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), Some(second));
        cx.update(|window, cx| smep.update(cx, |this, cx| this.activate(1, window, cx)));
        assert_eq!(value(&self::editor(&smep, cx), cx), "# Dropped");
    }

    #[gpui_kit::test]
    fn importing_something_unreadable_reports_instead_of_loading(cx: &mut TestAppContext) {
        let dir = TempDir::new("import-bad");
        let bad = dir.path("scan.pdf");
        std::fs::write(&bad, b"not a pdf").unwrap();
        let (smep, cx) = open(cx, "kept");
        let editor = editor(&smep, cx);

        cx.update(|window, cx| smep.update(cx, |this, cx| this.load_path(bad, window, cx)));
        cx.run_until_parked();

        assert!(cx.has_pending_prompt(), "an error dialog is shown");
        cx.simulate_prompt_answer("OK");
        assert_eq!(value(&editor, cx), "kept");
        assert!(!dirty(&smep, cx));
    }

    #[cfg(all(feature = "pdf-export", feature = "pdf-import"))]
    #[gpui_kit::test]
    fn exporting_writes_a_pdf_and_an_html_page_next_to_the_document(cx: &mut TestAppContext) {
        use crate::keymap::ExportHtml;

        let dir = TempDir::new("export");
        let path = dir.path("report.md");
        std::fs::write(&path, "# Report\n\nBody *text*.\n").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);

        // The dialog opens next to the document, suggesting report.pdf.
        cx.simulate_keystrokes(&primary("shift-e"));
        let pdf_path = dir.path("out.pdf");
        cx.simulate_new_path_selection({
            let pdf_path = pdf_path.clone();
            let expected_dir = dir.0.clone();
            move |directory| {
                assert_eq!(directory, expected_dir);
                Some(pdf_path)
            }
        });
        cx.run_until_parked();
        let pdf = std::fs::read(&pdf_path).expect("the PDF was written");
        assert!(pdf.starts_with(b"%PDF-"));
        let text = crate::convert::from_bytes(&pdf, crate::convert::Format::Pdf).unwrap();
        assert!(text.contains("Report") && text.contains("Body"), "{text:?}");

        cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.export_html(&ExportHtml, window, cx))
        });
        let html_path = dir.path("out.html");
        cx.simulate_new_path_selection({
            let html_path = html_path.clone();
            move |_| Some(html_path)
        });
        cx.run_until_parked();
        let html = std::fs::read_to_string(&html_path).expect("the HTML was written");
        assert!(html.contains("<title>Report</title>"));
        assert!(html.contains("<em>text</em>"));
        // The page wears the preview's look (the theme that follows the
        // application is Paper, light or dark).
        assert!(html.contains("<style>\n/* smep: the Paper"), "{html}");
        assert!(!dirty(&smep, cx), "exporting is not saving");
    }

    #[cfg(all(feature = "pdf-export", feature = "pdf-import"))]
    #[gpui_kit::test]
    fn the_users_own_stylesheet_and_preamble_dress_the_exports(cx: &mut TestAppContext) {
        use crate::keymap::ExportHtml;

        // (Not "export-look": the export module's own test owns that name
        // in the same temp directory, and the two run side by side.)
        let dir = TempDir::new("own-look");
        let config = dir.path("config");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join(crate::export::STYLESHEET_FILE),
            "body{color:rebeccapurple}\n",
        )
        .unwrap();
        // A preamble of the user's own: the page gets a header, which the
        // text read back out of the PDF shows.
        std::fs::write(
            config.join(crate::export::PREAMBLE_FILE),
            "#set page(paper: \"a4\", header: [OWN-PREAMBLE])\n#set text(size: 11pt)\n",
        )
        .unwrap();
        let settings = Settings {
            path: Some(config.join("settings.toml")),
            ..Settings::default()
        };
        let path = dir.path("report.md");
        std::fs::write(&path, "# Report\n\nBody.\n").unwrap();
        let (smep, cx) = open_with(cx, Document::read(path.clone()).unwrap(), settings);
        let editor = editor(&smep, cx);
        cx.update(|window, cx| editor.update(cx, |state, cx| state.focus(window, cx)));
        draw(cx);

        cx.simulate_keystrokes(&primary("shift-e"));
        let pdf_path = dir.path("own.pdf");
        cx.simulate_new_path_selection({
            let pdf_path = pdf_path.clone();
            move |_| Some(pdf_path)
        });
        cx.run_until_parked();
        assert!(
            !cx.has_pending_prompt(),
            "no error dialog: {:?}",
            cx.pending_prompt()
        );
        let pdf = std::fs::read(&pdf_path).expect("the PDF was written");
        let text = crate::convert::from_bytes(&pdf, crate::convert::Format::Pdf).unwrap();
        assert!(text.contains("OWN-PREAMBLE"), "{text:?}");
        assert!(text.contains("Report"), "{text:?}");

        cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.export_html(&ExportHtml, window, cx))
        });
        let html_path = dir.path("own.html");
        cx.simulate_new_path_selection({
            let html_path = html_path.clone();
            move |_| Some(html_path)
        });
        cx.run_until_parked();
        let html = std::fs::read_to_string(&html_path).expect("the HTML was written");
        assert!(
            html.contains("<style>\nbody{color:rebeccapurple}\n</style>"),
            "{html}"
        );
        assert!(
            !html.contains("/* smep:"),
            "the built-in stylesheet is replaced, not layered"
        );
    }

    #[gpui_kit::test]
    fn menus_dispatch_from_whatever_is_on_screen(cx: &mut TestAppContext) {
        let dir = TempDir::new("menu-focus");
        let path = dir.path("menu.md");
        std::fs::write(&path, "# T\n\npara").unwrap();
        let (smep, cx) = open_document(cx, Document::read(path.clone()).unwrap());
        let editor = editor(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        let focus = |cx: &VisualTestContext| cx.read(|cx| smep.read(cx).menu_focus(cx));

        assert_eq!(
            focus(cx),
            cx.read(|cx| editor.focus_handle(cx)),
            "split: the editor"
        );
        rendered_mode(&smep, cx);
        assert_eq!(
            focus(cx),
            cx.read(|cx| rendered.read(cx).focus_handle().clone()),
            "rendered, no block: the view itself"
        );
        activate_block(&smep, 5..9, cx);
        let block = active_block(&smep, cx);
        assert_eq!(focus(cx), cx.read(|cx| block.focus_handle(cx)), "the block");

        // What a menu does: focus its context, dispatch. With no block on
        // screen the action must still arrive (here: at the root's Save).
        type_text(&block, "para!", cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(dirty(&smep, cx));
        cx.update(|window, cx| {
            let handle = smep.read(cx).menu_focus(cx);
            handle.focus(window, cx);
            window.dispatch_action(Box::new(Save), cx);
        });
        cx.run_until_parked();
        assert!(!dirty(&smep, cx));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# T\n\npara!");
    }

    #[gpui_kit::test]
    fn a_large_document_is_highlighted_after_the_pause_not_on_the_keystroke(
        cx: &mut TestAppContext,
    ) {
        use crate::highlight::{self, MarkdownHighlighter};
        use gpui_kit::HighlightStyle;
        use gpui_kit::component::input::{HighlightStyleResolver, InputHighlighter};

        struct Faded;
        impl HighlightStyleResolver for Faded {
            fn style(&self, _: &str) -> Option<HighlightStyle> {
                Some(HighlightStyle {
                    fade_out: Some(0.5),
                    ..Default::default()
                })
            }
        }
        let styled = |highlighter: &MarkdownHighlighter| {
            highlighter
                .styles(&(0..9), &Faded)
                .iter()
                .any(|(_, style)| style.fade_out.is_some())
        };
        let update = |highlighter: &mut MarkdownHighlighter,
                      editor: &Entity<EditorState>,
                      cx: &mut VisualTestContext| {
            cx.update(|window, cx| {
                editor.update(cx, |state, cx| {
                    highlighter.update(None, state.text(), false, window, cx)
                })
            });
        };

        let big = "# Heading\n\ntext\n\n".repeat(highlight::SYNC_LIMIT / 16 + 1);
        assert!(big.len() > highlight::SYNC_LIMIT);
        let (smep, cx) = open(cx, &big);
        let source = editor(&smep, cx);
        let mut highlighter = MarkdownHighlighter::default();
        update(&mut highlighter, &source, cx);
        assert!(!styled(&highlighter), "nothing is parsed on the keystroke");
        cx.executor().advance_clock(highlight::DEBOUNCE);
        cx.run_until_parked();
        assert!(styled(&highlighter), "parsed once typing paused");

        // A small document is parsed on the spot.
        let (smep, cx) = open(cx, "# Heading");
        let source = editor(&smep, cx);
        let mut highlighter = MarkdownHighlighter::default();
        update(&mut highlighter, &source, cx);
        assert!(styled(&highlighter));
    }

    #[gpui_kit::test]
    fn block_edits_keep_the_block_list_equal_to_a_fresh_parse(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# Title\n\npara\n\n- x\n- y\n\n> quote");
        let document = editor(&smep, cx);
        rendered_mode(&smep, cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        let check = |cx: &mut VisualTestContext, when: &str| {
            draw(cx);
            let text = value(&document, cx);
            assert_eq!(
                cx.read(|cx| rendered.read(cx).block_ranges()),
                crate::highlight::block_ranges(&text),
                "{when}: {text:?}"
            );
        };

        // A middle block: grown, split in two, emptied, refilled.
        activate_block(&smep, 9..13, cx);
        let block = active_block(&smep, cx);
        for typed in ["para longer", "para\n\nsecond", "", "back"] {
            type_text(&block, typed, cx);
            check(cx, typed);
        }

        // The first block, then a new one at the end (with its gap).
        activate_block(&smep, 0..7, cx);
        type_text(&active_block(&smep, cx), "## Small", cx);
        check(cx, "first block");
        cx.update(|window, cx| {
            rendered.update(cx, |rendered, cx| rendered.activate_at_end(window, cx))
        });
        type_text(&active_block(&smep, cx), "tail", cx);
        check(cx, "new block at the end");

        // A change made elsewhere ends the edit and is parsed afresh.
        type_text(&document, "# Other\n\nthing", cx);
        check(cx, "source edit");

        // From nothing, and with CRLF line endings.
        for start in ["", "a\r\n\r\nb\r\n"] {
            let (smep, cx) = open(cx, start);
            let document = editor(&smep, cx);
            rendered_mode(&smep, cx);
            let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
            cx.update(|window, cx| {
                rendered.update(cx, |rendered, cx| rendered.activate_at_end(window, cx))
            });
            for typed in ["x", "x\r\n\r\ny", ""] {
                type_text(&active_block(&smep, cx), typed, cx);
                draw(cx);
                let text = value(&document, cx);
                assert_eq!(
                    cx.read(|cx| rendered.read(cx).block_ranges()),
                    crate::highlight::block_ranges(&text),
                    "{start:?} then {typed:?}: {text:?}"
                );
            }
        }
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
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
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
                        origin: None,
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

        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
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

    fn open_paths(smep: &Entity<Smep>, paths: Vec<PathBuf>, cx: &mut VisualTestContext) {
        cx.update(|window, cx| smep.update(cx, |this, cx| this.open_paths(paths, window, cx)));
        cx.run_until_parked();
    }

    fn tabs(smep: &Entity<Smep>, cx: &VisualTestContext) -> (usize, usize) {
        cx.read(|cx| (smep.read(cx).tab_count(), smep.read(cx).active_tab()))
    }

    #[gpui_kit::test]
    fn files_from_the_os_open_in_tabs_without_asking(cx: &mut TestAppContext) {
        let dir = TempDir::new("os-open");
        let path = dir.path("incoming.md");
        std::fs::write(&path, "# From Finder").unwrap();
        let (smep, cx) = open(cx, "");
        let first = editor(&smep, cx);

        // The blank tab takes the first file.
        open_paths(&smep, vec![path.clone()], cx);
        assert_eq!(value(&first, cx), "# From Finder");
        assert_eq!(tabs(&smep, cx), (1, 0));
        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().path.clone()),
            Some(path.clone())
        );

        // An edited document stays as it is: the next file gets a tab of its
        // own, with no prompt.
        type_text(&first, "edited", cx);
        let other = dir.path("other.md");
        std::fs::write(&other, "other").unwrap();
        open_paths(&smep, vec![other.clone()], cx);
        assert!(!cx.has_pending_prompt());
        assert_eq!(tabs(&smep, cx), (2, 1));
        assert_eq!(value(&editor(&smep, cx), cx), "other");
        assert_eq!(value(&first, cx), "edited");
        assert!(
            dirty(&smep, cx),
            "the window has unsaved changes, in a tab not shown"
        );

        // A file already open is shown, not opened again.
        open_paths(&smep, vec![path.clone()], cx);
        assert_eq!(tabs(&smep, cx), (2, 0));

        // Nothing: an empty tab; nothing again while that tab is blank: no
        // second one.
        open_paths(&smep, Vec::new(), cx);
        assert_eq!(tabs(&smep, cx), (3, 2));
        assert_eq!(value(&editor(&smep, cx), cx), "");
        open_paths(&smep, Vec::new(), cx);
        assert_eq!(tabs(&smep, cx), (3, 2));

        // A file in a folder that does not exist is reported.
        open_paths(&smep, vec![dir.path("no/such.md")], cx);
        assert!(cx.has_pending_prompt(), "an error dialog is shown");
        cx.simulate_prompt_answer("OK");
        assert_eq!(tabs(&smep, cx), (3, 2));

        // A file not there yet, in a folder that is: a new document that
        // saves there, in the blank tab.
        open_paths(&smep, vec![dir.path("new.md")], cx);
        assert_eq!(tabs(&smep, cx), (3, 2));
        assert_eq!(
            cx.read(|cx| smep.read(cx).tab().path.clone()),
            Some(dir.path("new.md"))
        );
    }

    #[gpui_kit::test]
    fn tabs_switch_by_keyboard_and_close_after_asking(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "one");
        draw(cx);
        cx.simulate_keystrokes(&primary("t"));
        assert_eq!(tabs(&smep, cx), (2, 1));
        let second = editor(&smep, cx);
        assert_eq!(value(&second, cx), "");
        type_text(&second, "two", cx);
        draw(cx);

        cx.simulate_keystrokes("ctrl-tab");
        assert_eq!(tabs(&smep, cx), (2, 0));
        assert_eq!(value(&editor(&smep, cx), cx), "one");
        cx.simulate_keystrokes("ctrl-shift-tab");
        assert_eq!(tabs(&smep, cx), (2, 1), "back, round the end");
        cx.simulate_keystrokes("ctrl-tab");
        cx.simulate_keystrokes("ctrl-tab");
        assert_eq!(tabs(&smep, cx), (2, 1), "forward, round the end");

        // Closing the edited tab asks; Cancel keeps it.
        cx.simulate_keystrokes(&primary("w"));
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert_eq!(tabs(&smep, cx), (2, 1));
        cx.simulate_keystrokes(&primary("w"));
        cx.simulate_prompt_answer("Don't Save");
        cx.run_until_parked();
        assert_eq!(tabs(&smep, cx), (1, 0));
        assert_eq!(value(&editor(&smep, cx), cx), "one");
        assert!(!dirty(&smep, cx));

        // The last tab closed leaves a blank one; the window stays.
        cx.simulate_keystrokes(&primary("w"));
        cx.run_until_parked();
        assert!(
            !cx.has_pending_prompt(),
            "a clean tab closes without asking"
        );
        assert_eq!(tabs(&smep, cx), (1, 0));
        assert_eq!(value(&editor(&smep, cx), cx), "");
    }

    #[gpui_kit::test]
    async fn closing_the_window_asks_about_each_unsaved_tab(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "");
        type_text(&editor(&smep, cx), "first", cx);
        cx.update(|window, cx| smep.update(cx, |this, cx| this.new_tab(window, cx)));
        type_text(&editor(&smep, cx), "second", cx);
        cx.update(|window, cx| smep.update(cx, |this, cx| this.new_tab(window, cx)));
        assert_eq!(tabs(&smep, cx), (3, 2));

        // Each unsaved tab is shown while it is asked about; a Cancel stops.
        let proceed = cx
            .update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard_all(window, cx)));
        cx.run_until_parked();
        assert_eq!(tabs(&smep, cx), (3, 0));
        cx.simulate_prompt_answer("Don't Save");
        cx.run_until_parked();
        assert_eq!(tabs(&smep, cx), (3, 1));
        cx.simulate_prompt_answer("Cancel");
        assert!(!proceed.await);

        let proceed = cx
            .update(|window, cx| smep.update(cx, |this, cx| this.confirm_discard_all(window, cx)));
        cx.run_until_parked();
        cx.simulate_prompt_answer("Don't Save");
        cx.run_until_parked();
        cx.simulate_prompt_answer("Don't Save");
        assert!(proceed.await, "every tab answered");
        assert!(!cx.has_pending_prompt(), "the blank tab is not asked about");
    }

    #[gpui_kit::test]
    fn only_the_tab_on_screen_edits_blocks(cx: &mut TestAppContext) {
        let (smep, cx) = open(cx, "# A\n\nfirst\n");
        rendered_mode(&smep, cx);
        activate_block(&smep, 5..10, cx);
        let first = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        assert!(cx.read(|cx| first.read(cx).active_editor().is_some()));

        cx.update(|window, cx| smep.update(cx, |this, cx| this.new_tab(window, cx)));
        let second = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        assert!(cx.read(|cx| first.read(cx).is_read_only()), "the tab left");
        assert!(
            cx.read(|cx| first.read(cx).active_editor().is_none()),
            "its edit ended"
        );
        assert!(!cx.read(|cx| second.read(cx).is_read_only()));

        cx.update(|window, cx| smep.update(cx, |this, cx| this.activate(0, window, cx)));
        assert!(!cx.read(|cx| first.read(cx).is_read_only()));
        assert!(cx.read(|cx| second.read(cx).is_read_only()));
        cx.update(|window, cx| smep.update(cx, |this, cx| this.set_editing(false, window, cx)));
        assert!(cx.read(|cx| first.read(cx).is_read_only()), "reading");
    }

    #[gpui_kit::test]
    fn several_files_on_the_command_line_open_in_tabs(cx: &mut TestAppContext) {
        let dir = TempDir::new("cli-tabs");
        let (a, b) = (dir.path("a.md"), dir.path("b.md"));
        std::fs::write(&a, "a").unwrap();
        std::fs::write(&b, "b").unwrap();
        let (smep, cx) = open_document(cx, Document::read(a.clone()).unwrap());
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                let documents = vec![
                    Document::read(b.clone()).unwrap(),
                    Document::read(a.clone()).unwrap(),
                ];
                this.add_documents(documents, window, cx)
            })
        });
        assert_eq!(tabs(&smep, cx), (2, 0), "a.md twice is one tab");
        assert_eq!(value(&editor(&smep, cx), cx), "a");

        // Saving a.md's tab as b.md, which the other tab holds, is refused:
        // one tab per file.
        type_text(&editor(&smep, cx), "a, edited", cx);
        let saved = cx.update(|window, cx| {
            smep.update(cx, |this, cx| this.save_as_task(this.tab().id, window, cx))
        });
        cx.simulate_new_path_selection({
            let b = b.clone();
            move |_| Some(b)
        });
        cx.run_until_parked();
        assert!(cx.has_pending_prompt(), "the refusal is reported");
        cx.simulate_prompt_answer("OK");
        drop(saved);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "b", "b.md untouched");
        assert_eq!(cx.read(|cx| smep.read(cx).tab().path.clone()), Some(a));
        assert!(dirty(&smep, cx));
    }

    #[gpui_kit::test]
    fn the_outline_lists_the_headings_and_jumps_to_them(cx: &mut TestAppContext) {
        let filler = "Some text in a paragraph.\n\n".repeat(40);
        let text = format!("# One\n\n{filler}## Two\n\n{filler}> ### Three, quoted\n\n{filler}");
        let (smep, cx) = open_reading(
            cx,
            Document {
                path: None,
                text: text.clone(),
                format: PreviewFormat::Markdown,
                origin: None,
            },
            Settings::default(),
        );
        draw(cx);
        // Shown from the start; the key folds it and unfolds it.
        assert!(cx.read(|cx| smep.read(cx).outline_shown()));
        cx.simulate_keystrokes(&primary("shift-l"));
        assert!(!cx.read(|cx| smep.read(cx).outline_shown()));
        cx.simulate_keystrokes(&primary("shift-l"));
        assert!(cx.read(|cx| smep.read(cx).outline_shown()));
        // Folded, a click on the strip left of it unfolds it.
        cx.simulate_keystrokes(&primary("shift-l"));
        draw(cx);
        cx.simulate_click(point(px(9.), px(300.)), Default::default());
        draw(cx);
        assert!(
            cx.read(|cx| smep.read(cx).outline_shown()),
            "the strip unfolds it"
        );
        draw(cx);
        let headings = cx.read(|cx| smep.read(cx).tab().headings.clone());
        let titles: Vec<&str> = headings.iter().map(|h| h.title.as_str()).collect();
        assert_eq!(titles, ["One", "Two", "Three, quoted"]);

        // Reading: the page scrolls to the heading's block.
        cx.update(|window, cx| smep.update(cx, |this, cx| this.jump_to_heading(2, window, cx)));
        draw(cx);
        let list = cx.read(|cx| smep.read(cx).reading_list(cx));
        assert_eq!(list.logical_scroll_top().item_ix, headings[2].block);
        assert_eq!(cx.read(|cx| smep.read(cx).current_heading(cx)), Some(2));
        cx.update(|window, cx| smep.update(cx, |this, cx| this.jump_to_heading(1, window, cx)));
        draw(cx);
        assert_eq!(list.logical_scroll_top().item_ix, headings[1].block);
        assert_eq!(cx.read(|cx| smep.read(cx).current_heading(cx)), Some(1));

        // Editing in the split: the editor's cursor goes to the heading.
        cx.update(|window, cx| {
            smep.update(cx, |this, cx| {
                this.set_view_mode(ViewMode::Split, window, cx)
            })
        });
        cx.update(|window, cx| smep.update(cx, |this, cx| this.jump_to_heading(2, window, cx)));
        draw(cx);
        let editor = editor(&smep, cx);
        assert_eq!(
            cx.read(|cx| editor.read(cx).cursor_position().line as usize),
            headings[2].line
        );

        // The block view: no edit left open, the heading's block on top.
        rendered_mode(&smep, cx);
        cx.update(|window, cx| smep.update(cx, |this, cx| this.jump_to_heading(1, window, cx)));
        draw(cx);
        draw(cx);
        let rendered = cx.read(|cx| smep.read(cx).tab().rendered.clone());
        assert_eq!(
            cx.read(|cx| rendered.read(cx).top_block()),
            headings[1].block
        );
        // Out of range: nothing happens.
        cx.update(|window, cx| smep.update(cx, |this, cx| this.jump_to_heading(9, window, cx)));
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
            cx.read(|cx| smep.read(cx).tab().source.to_string()),
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
            origin: None,
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

        // First `down` selects System (the current one), the second Paper.
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();

        assert_eq!(
            cx.read(|cx| smep.read(cx).preview_theme()),
            PreviewTheme::Paper
        );
        assert!(!menu_open(&smep, cx));
        assert!(
            std::fs::read_to_string(dir.path("settings.toml"))
                .unwrap()
                .contains("preview_theme = \"paper\"")
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
        let crlf_editor = cx.read(|cx| smep.read(cx).tab().editor.clone());
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
