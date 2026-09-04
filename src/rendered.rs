//! The rendered view: the document as rendered blocks, where the block you
//! click turns into its Markdown source for editing and renders again when
//! you move on (a click elsewhere, or Escape). The document itself stays in
//! the main editor state; every keystroke in a block is written through to it.

use std::ops::Range;

use gpui_kit::base::text::TextView;
use gpui_kit::component::input::{EditorState, Escape, InputEvent, Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::highlight;
use crate::insert;
use crate::theme::Palette;

/// What the rendered view tells its owner.
pub enum RenderedEvent {
    /// A lone `/` was typed on an empty line of the active block.
    SlashTyped,
}

/// The block being edited: an editor holding a slice of the document.
///
/// A plain text area rather than a code editor: it is the one input whose
/// height follows its text (auto-grow, wrapped lines included), and a block
/// is only briefly source, so it does without highlighting.
struct ActiveBlock {
    /// Where the editor's text starts in the document.
    start: usize,
    /// How many document bytes the editor's text currently stands for.
    committed_len: usize,
    editor: Entity<TextareaState>,
    _subscriptions: Vec<Subscription>,
}

pub struct RenderedView {
    /// The document. Shared with the source view.
    source: Entity<EditorState>,
    active: Option<ActiveBlock>,
    palette: Option<Palette>,
    focus_handle: FocusHandle,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<RenderedEvent> for RenderedView {}

impl RenderedView {
    pub fn new(source: Entity<EditorState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&source, |_, _, cx| cx.notify()),
            // The active block stands for a range of the document; a change
            // that did not come through it (typing in the source view, undo
            // there) leaves the range stale, so the edit ends.
            cx.subscribe_in(
                &source,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if let InputEvent::Change = event
                        && !this.active_matches_document(cx)
                    {
                        this.deactivate(window, cx);
                    }
                },
            ),
        ];
        Self {
            source,
            active: None,
            palette: None,
            focus_handle: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    /// The editor holding the block being edited, if any.
    pub fn active_editor(&self) -> Option<Entity<TextareaState>> {
        self.active.as_ref().map(|active| active.editor.clone())
    }

    pub fn set_palette(&mut self, palette: Option<Palette>, cx: &mut Context<Self>) {
        if self.palette != palette {
            self.palette = palette;
            cx.notify();
        }
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    fn text(&self, cx: &App) -> SharedString {
        self.source.read(cx).value()
    }

    /// The document range the active editor stands for.
    fn active_range(&self) -> Option<Range<usize>> {
        self.active
            .as_ref()
            .map(|active| active.start..active.start + active.committed_len)
    }

    /// Whether the document still holds the active editor's text where the
    /// editor stands. True with no active block.
    fn active_matches_document(&self, cx: &App) -> bool {
        let Some(active) = self.active.as_ref() else {
            return true;
        };
        let range = active.start..active.start + active.committed_len;
        let document = self.text(cx);
        document.get(range) == Some(active.editor.read(cx).value().as_ref())
    }

    /// Edit the block whose text sits at `range` in the document.
    pub fn activate(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_range() == Some(range.clone()) {
            return;
        }
        self.deactivate(window, cx);

        let text = self.text(cx);
        let block = text.get(range.clone()).unwrap_or_default().to_string();
        let editor = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(block.clone())
                .auto_grow(1, usize::MAX)
                .soft_wrap(true)
                .placeholder("Type here. / inserts a block.")
        });
        editor.update(cx, |state, cx| {
            let end = state.text().len();
            state.set_selected_range(end..end, cx);
            state.focus(window, cx);
        });

        let subscriptions = vec![
            cx.subscribe_in(
                &editor,
                window,
                |this, editor, event: &InputEvent, window, cx| {
                    if let InputEvent::Change = event {
                        this.commit(window, cx);
                        let state = editor.read(cx);
                        if insert::cursor_line_trimmed(state) == "/"
                            && state.cursor() == insert::cursor_line_range(state).end
                        {
                            cx.emit(RenderedEvent::SlashTyped);
                        }
                        cx.notify();
                    }
                },
            ),
            cx.observe(&editor, |_, _, cx| cx.notify()),
        ];

        self.active = Some(ActiveBlock {
            start: range.start,
            committed_len: range.len(),
            editor,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    /// Start a new block after the last one (a click below the text).
    pub fn activate_at_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let end = self.text(cx).len();
        self.activate(end..end, window, cx);
    }

    /// Stop editing; the text is already in the document. Focus comes back
    /// to the view so the window's shortcuts keep working.
    pub fn deactivate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.active.take().is_some() {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    /// Write the active editor's text into the document in place of what it
    /// stood for. A block that starts right after other text gets the blank
    /// line Markdown needs to keep it a block of its own.
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let typed = active.editor.read(cx).value();
        let range = active.start..active.start + active.committed_len;
        let document = self.source.read(cx).value();
        let before = &document[..range.start];
        let needs_gap =
            !before.is_empty() && !before.ends_with("\n\n") && !before.ends_with("\r\n\r\n");
        let text = if needs_gap && !typed.is_empty() && active.committed_len == 0 {
            // The first commit into a brand-new block at the end.
            let newline = if before.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let missing = if before.ends_with(newline) { 1 } else { 2 };
            format!("{}{}", newline.repeat(missing), typed)
        } else {
            typed.to_string()
        };

        // Since a gap was inserted, the editor's text starts later.
        active.start += text.len() - typed.len();
        active.committed_len = typed.len();
        self.source.update(cx, |state, cx| {
            state.set_selected_range(range, cx);
            state.replace(text, window, cx);
        });
    }
}

impl Render for RenderedView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text = self.text(cx);
        let ranges = highlight::block_ranges(&text);
        let active = self.active_range();
        let palette = self.palette;
        let background = palette.map_or(cx.theme().background, |p| p.background);
        let foreground = palette.map_or(cx.theme().foreground, |p| p.foreground);
        let muted = cx.theme().muted_foreground;

        // The editor takes the place of the blocks its text parses into,
        // and sits between its neighbours even while its text is empty.
        let mut items: Vec<AnyElement> = Vec::with_capacity(ranges.len() + 1);
        let mut editor_placed = false;
        for (ix, range) in ranges.iter().enumerate() {
            if let Some(active) = &active {
                let overlaps = range.start < active.end && range.end > active.start;
                if overlaps {
                    if !editor_placed {
                        items.push(self.render_active(cx));
                        editor_placed = true;
                    }
                    continue;
                }
                if !editor_placed && range.start >= active.end {
                    items.push(self.render_active(cx));
                    editor_placed = true;
                }
            }
            items.push(self.render_block(ix, range.clone(), &text, palette, cx));
        }
        if active.is_some() && !editor_placed {
            items.push(self.render_active(cx));
        }
        if items.is_empty() {
            items.push(
                div()
                    .id("empty")
                    .text_color(muted)
                    .child("Click here to start writing.")
                    .into_any_element(),
            );
        }

        v_flex()
            .id("rendered")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .track_focus(&self.focus_handle)
            .bg(background)
            .text_color(foreground)
            .when_some(palette.and_then(|p| p.font_family()), |this, family| {
                this.font_family(family)
            })
            .px_6()
            .py_4()
            .gap_3()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.activate_at_end(window, cx);
                    // A focusable element takes focus on click after its own
                    // listeners; that would take it back from the new block.
                    window.prevent_default();
                }),
            )
            // The block editor lets a plain Escape through; it ends the edit.
            .on_action(cx.listener(|this, _: &Escape, window, cx| this.deactivate(window, cx)))
            .children(items)
    }
}

impl RenderedView {
    fn render_block(
        &self,
        ix: usize,
        range: Range<usize>,
        text: &str,
        palette: Option<Palette>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let source: SharedString = text[range.clone()].to_string().into();
        div()
            .id(("block", ix))
            .w_full()
            .cursor_text()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.activate(range.clone(), window, cx);
                }),
            )
            .child(
                TextView::markdown(("block-text", ix), source)
                    .when_some(palette, |view, p| view.style(p.text_view_style())),
            )
            .into_any_element()
    }

    fn render_active(&self, _cx: &mut Context<Self>) -> AnyElement {
        let Some(active) = self.active.as_ref() else {
            return div().into_any_element();
        };
        div()
            .id("active-block")
            .w_full()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                Textarea::new(&active.editor)
                    .bordered(false)
                    .appearance(false),
            )
            .into_any_element()
    }
}
