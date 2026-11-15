//! Vim mode: modal editing for the editors.
//!
//! The engine is pure. It reads the text and the cursor and answers with
//! the edits and moves to make ([`Op`]s); it never holds the text. `app.rs`
//! feeds it the keys the focused editor would get and applies what comes
//! back, or lets a key through to the editor ([`Outcome::Pass`]).
//!
//! Offsets are UTF-8 byte offsets into the text, always on a character
//! boundary. In Normal mode the cursor sits on a character (never past the
//! last one of a line, unless the line is empty); in Visual mode `head` and
//! `anchor` are characters, and the selection shown covers both.

use std::ops::Range;

use gpui_kit::Keystroke;

/// The modes, as Vim names them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
            Self::Visual => "VISUAL",
            Self::VisualLine => "V-LINE",
        }
    }

    fn is_visual(self) -> bool {
        matches!(self, Self::Visual | Self::VisualLine)
    }
}

/// A key as the engine sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// A printable character, shift already applied.
    Char(char),
    Escape,
    Enter,
    Backspace,
    Delete,
    Tab,
    /// The arrows, Home and End, without modifiers: `h` `l` `k` `j` `0`
    /// `$` outside Insert mode.
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    /// Control with a letter or `[`.
    Ctrl(char),
}

impl Key {
    /// The key for a keystroke, or `None` for one the editor and the
    /// window keep: arrows, function keys, Cmd/Ctrl shortcuts.
    pub fn from_keystroke(keystroke: &Keystroke) -> Option<Self> {
        let modifiers = &keystroke.modifiers;
        if modifiers.platform || modifiers.function {
            return None;
        }
        // Ctrl+Alt is AltGr on Windows: a character, handled below.
        if modifiers.control && !modifiers.alt {
            return match keystroke.key.as_str() {
                key @ ("r" | "d" | "u" | "[") => key.chars().next().map(Self::Ctrl),
                _ => None,
            };
        }
        match keystroke.key.as_str() {
            "escape" => return Some(Self::Escape),
            "enter" => return Some(Self::Enter),
            "backspace" => return Some(Self::Backspace),
            "delete" => return Some(Self::Delete),
            "tab" => return Some(Self::Tab),
            "space" => return Some(Self::Char(' ')),
            _ => {}
        }
        // Shift with an arrow selects, Alt with one moves by words: the
        // editor's.
        if !modifiers.shift && !modifiers.alt {
            match keystroke.key.as_str() {
                "left" => return Some(Self::Left),
                "right" => return Some(Self::Right),
                "up" => return Some(Self::Up),
                "down" => return Some(Self::Down),
                "home" => return Some(Self::Home),
                "end" => return Some(Self::End),
                _ => {}
            }
        }
        let mut chars = keystroke.key_char.as_deref()?.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) if !c.is_control() => Some(Self::Char(c)),
            _ => None,
        }
    }

    fn is_escape(&self) -> bool {
        matches!(self, Self::Escape | Self::Ctrl('['))
    }

    fn show(&self) -> String {
        match self {
            Self::Char(c) => c.to_string(),
            Self::Escape => "<Esc>".into(),
            Self::Enter => "<CR>".into(),
            Self::Backspace => "<BS>".into(),
            Self::Delete => "<Del>".into(),
            Self::Tab => "<Tab>".into(),
            Self::Left => "<Left>".into(),
            Self::Right => "<Right>".into(),
            Self::Up => "<Up>".into(),
            Self::Down => "<Down>".into(),
            Self::Home => "<Home>".into(),
            Self::End => "<End>".into(),
            Self::Ctrl(c) => format!("<C-{c}>"),
        }
    }
}

/// One thing for the editor to do. The ops of a key apply in order; an
/// edit leaves the cursor after the new text, and a batch that edits ends
/// with the cursor's place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    /// Replace `range` with `text`.
    Edit {
        range: Range<usize>,
        text: String,
    },
    /// Put the cursor at the offset, with nothing selected.
    Cursor(usize),
    /// Select the range (Visual mode).
    Select(Range<usize>),
    Undo,
    Redo,
    PageDown,
    PageUp,
    /// Open the editor's find bar.
    Find,
}

/// What became of a key.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Not Vim's: the editor or the window handles it as usual.
    Pass,
    /// Vim's; the editor must not see it. The ops may be none.
    Handled(Vec<Op>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Operator {
    Delete,
    Change,
    Yank,
}

impl Operator {
    fn for_char(c: char) -> Option<Self> {
        match c {
            'd' => Some(Self::Delete),
            'c' => Some(Self::Change),
            'y' => Some(Self::Yank),
            _ => None,
        }
    }
}

/// `f`, `F`, `t` and `T`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FindKind {
    forward: bool,
    /// `t` / `T`: stop next to the character.
    till: bool,
}

impl FindKind {
    fn reversed(self) -> Self {
        Self {
            forward: !self.forward,
            ..self
        }
    }
}

/// A command waiting for one more key.
#[derive(Clone, Copy, Debug)]
enum Pending {
    Find(FindKind),
    /// After `g`.
    G,
    /// After `i` / `a` with an operator or in Visual mode.
    Object {
        inner: bool,
    },
    /// After `r`.
    Replace,
}

#[derive(Clone, Copy, Debug)]
enum Motion {
    Left,
    Right,
    Up,
    Down,
    /// Backspace: left, on to the line above.
    BackWrap,
    /// Space: right, on to the line below.
    ForwardWrap,
    LineStart,
    FirstNonBlank,
    LineEnd,
    /// `+` and Enter.
    NextLine,
    /// `-`.
    PreviousLine,
    WordStart {
        big: bool,
    },
    WordEnd {
        big: bool,
    },
    WordBack {
        big: bool,
    },
    Paragraph {
        forward: bool,
    },
    Bracket,
    Find {
        kind: FindKind,
        ch: char,
        repeat: bool,
    },
    /// `G`.
    LastLine,
    /// `gg`.
    FirstLine,
}

impl Motion {
    fn for_char(c: char) -> Option<Self> {
        Some(match c {
            'h' => Self::Left,
            'l' => Self::Right,
            'k' => Self::Up,
            'j' => Self::Down,
            ' ' => Self::ForwardWrap,
            '0' => Self::LineStart,
            '^' => Self::FirstNonBlank,
            '$' => Self::LineEnd,
            '+' => Self::NextLine,
            '-' => Self::PreviousLine,
            'w' => Self::WordStart { big: false },
            'W' => Self::WordStart { big: true },
            'e' => Self::WordEnd { big: false },
            'E' => Self::WordEnd { big: true },
            'b' => Self::WordBack { big: false },
            'B' => Self::WordBack { big: true },
            '}' => Self::Paragraph { forward: true },
            '{' => Self::Paragraph { forward: false },
            '%' => Self::Bracket,
            'G' => Self::LastLine,
            _ => return None,
        })
    }

    fn is_vertical(self) -> bool {
        matches!(self, Self::Up | Self::Down)
    }
}

/// How a motion's span is taken by an operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Up to the target, not including it.
    Exclusive,
    /// Up to and including the target's character.
    Inclusive,
    /// Whole lines.
    Linewise,
}

/// What an operator works on.
#[derive(Clone, Debug)]
enum Span {
    Chars(Range<usize>),
    /// The lines holding the two offsets, and those between.
    Lines(usize, usize),
}

/// The register `y`, `d` and `c` fill and `p` puts.
#[derive(Clone, Debug, Default)]
struct Register {
    text: String,
    /// Whole lines, each ending with a newline.
    linewise: bool,
}

/// The last change, for `.`: the keys of its command and, when it went
/// into Insert mode, the text typed there.
#[derive(Clone, Debug)]
struct Change {
    keys: Vec<Key>,
    insert: Option<String>,
}

/// The result of the keys typed so far for a command.
enum Step {
    /// The command needs more keys.
    More,
    /// The command is complete. `bool`: it changed the text (for `.`).
    Done(Vec<Op>, bool),
    /// Not Vim's.
    Pass,
}

fn done(ops: Vec<Op>) -> Step {
    Step::Done(ops, false)
}

fn changed(ops: Vec<Op>) -> Step {
    Step::Done(ops, true)
}

fn abort() -> Step {
    Step::Done(Vec::new(), false)
}

/// The state of Vim mode for the window.
#[derive(Debug, Default)]
pub struct Vim {
    mode: Mode,
    /// The count typed before the operator (or before a motion).
    count: Option<usize>,
    /// The count typed after the operator: `d3w`.
    op_count: Option<usize>,
    operator: Option<Operator>,
    pending: Option<Pending>,
    /// Visual mode: where the selection started, and where it is now.
    anchor: usize,
    head: usize,
    /// The selection last shown, to notice one made by the mouse.
    shown: Range<usize>,
    /// The column `j` and `k` keep to; `usize::MAX` after `$`.
    want_column: Option<usize>,
    /// Whether the command being typed moved up or down.
    vertical: bool,
    last_find: Option<(FindKind, char)>,
    register: Register,
    /// The keys of the command being typed.
    keys: Vec<Key>,
    last_change: Option<Change>,
    /// The keys of a change still in Insert mode, and where its typing began.
    recording: Option<(Vec<Key>, Option<usize>)>,
    /// Replaying `.`: nothing is recorded.
    replaying: bool,
}

impl Vim {
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The keys of the command being typed, as Vim shows them: `2d`, `f`.
    pub fn pending_keys(&self) -> String {
        self.keys.iter().map(Key::show).collect()
    }

    /// Back to Normal mode with nothing pending. The register and the last
    /// change are kept.
    pub fn reset(&mut self) {
        self.mode = Mode::Normal;
        self.end_command();
        self.recording = None;
        self.shown = 0..0;
    }

    /// Keys now go to another editor: what belonged to the last one (a
    /// selection, a half-typed command, typing being recorded) ends there.
    /// Insert mode carries over.
    pub fn retarget(&mut self) {
        if self.mode.is_visual() {
            self.mode = Mode::Normal;
        }
        self.end_command();
        self.recording = None;
        self.shown = 0..0;
    }

    /// Whether `key` goes straight to the editor, without a look at the
    /// text: typing in Insert mode.
    pub fn passes(&self, key: &Key) -> bool {
        self.mode == Mode::Insert && !key.is_escape()
    }

    /// Handle `key` in an editor holding `text` with the cursor at `cursor`
    /// and `selection` selected.
    pub fn key(&mut self, key: Key, text: &str, cursor: usize, selection: Range<usize>) -> Outcome {
        let cursor = clip(text, cursor);
        match self.mode {
            Mode::Insert => self.insert_key(key, text, cursor),
            Mode::Visual | Mode::VisualLine if selection != self.shown => {
                // A click or a drag changed the selection under Visual mode.
                if selection.is_empty() {
                    self.mode = Mode::Normal;
                    self.end_command();
                } else {
                    self.select_from(text, selection, cursor);
                }
                self.normal_key(key, text, cursor)
            }
            Mode::Normal if !selection.is_empty() && self.idle() => {
                // Text selected with the mouse (or Select All): Visual mode.
                self.select_from(text, selection, cursor);
                self.normal_key(key, text, cursor)
            }
            _ => self.normal_key(key, text, cursor),
        }
    }

    fn idle(&self) -> bool {
        self.operator.is_none()
            && self.pending.is_none()
            && self.count.is_none()
            && self.keys.is_empty()
    }

    /// Visual mode over a selection made outside Vim. Its end is exclusive;
    /// the cursor is at one end or the other.
    fn select_from(&mut self, text: &str, selection: Range<usize>, cursor: usize) {
        let last = prev(text, selection.end).max(selection.start);
        (self.anchor, self.head) = if cursor == selection.start && cursor != selection.end {
            (last, selection.start)
        } else {
            (selection.start, last)
        };
        self.mode = Mode::Visual;
        self.shown = selection;
    }

    fn insert_key(&mut self, key: Key, text: &str, cursor: usize) -> Outcome {
        if !key.is_escape() {
            return Outcome::Pass;
        }
        self.mode = Mode::Normal;
        if let Some((keys, from)) = self.recording.take() {
            let insert = from
                .filter(|&from| from <= cursor && text.is_char_boundary(from))
                .map(|from| text[from..cursor].to_string());
            self.last_change = Some(Change { keys, insert });
        }
        Outcome::Handled(vec![Op::Cursor(step_back_in_line(text, cursor))])
    }

    fn normal_key(&mut self, key: Key, text: &str, cursor: usize) -> Outcome {
        let visual = self.mode.is_visual();
        let cur = if visual {
            clip(text, self.head)
        } else {
            clamp_normal(text, cursor)
        };
        if !self.replaying {
            self.keys.push(key.clone());
        }
        match self.command(key, text, cur) {
            Step::More => Outcome::Handled(Vec::new()),
            Step::Pass => {
                self.end_command();
                Outcome::Pass
            }
            Step::Done(ops, change) => {
                let keys = std::mem::take(&mut self.keys);
                self.end_command();
                if change && !visual && !self.replaying {
                    if self.mode == Mode::Insert {
                        self.recording = Some((keys, final_cursor(&ops)));
                    } else {
                        self.last_change = Some(Change { keys, insert: None });
                    }
                }
                Outcome::Handled(ops)
            }
        }
    }

    fn end_command(&mut self) {
        self.count = None;
        self.op_count = None;
        self.operator = None;
        self.pending = None;
        self.keys.clear();
        if !self.vertical {
            self.want_column = None;
        }
        self.vertical = false;
    }

    fn total_count(&self) -> usize {
        self.count.unwrap_or(1) * self.op_count.unwrap_or(1)
    }

    fn explicit_count(&self) -> Option<usize> {
        (self.count.is_some() || self.op_count.is_some()).then(|| self.total_count())
    }

    fn command(&mut self, key: Key, text: &str, cur: usize) -> Step {
        if let Some(pending) = self.pending.take() {
            return self.pending_key(pending, key, text, cur);
        }
        let visual = self.mode.is_visual();
        let c = match key {
            Key::Char(c) => c,
            Key::Escape | Key::Ctrl('[') => return self.escape(text),
            Key::Enter => return self.motion(Motion::NextLine, text, cur),
            Key::Backspace => return self.motion(Motion::BackWrap, text, cur),
            Key::Left => return self.motion(Motion::Left, text, cur),
            Key::Right => return self.motion(Motion::Right, text, cur),
            Key::Up => return self.motion(Motion::Up, text, cur),
            Key::Down => return self.motion(Motion::Down, text, cur),
            Key::Home => return self.motion(Motion::LineStart, text, cur),
            Key::End => return self.motion(Motion::LineEnd, text, cur),
            Key::Delete if visual => 'd',
            Key::Delete => 'x',
            Key::Tab => return abort(),
            Key::Ctrl('r') if !visual => return done(vec![Op::Redo]),
            Key::Ctrl('d') if !visual => return done(vec![Op::PageDown]),
            Key::Ctrl('u') if !visual => return done(vec![Op::PageUp]),
            Key::Ctrl(_) => return abort(),
        };

        if let Some(digit) = c.to_digit(10)
            && (digit != 0 || self.counting())
        {
            let slot = if self.operator.is_some() {
                &mut self.op_count
            } else {
                &mut self.count
            };
            *slot = Some(
                slot.unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit as usize)
                    .min(99_999),
            );
            return Step::More;
        }
        if let Some(motion) = Motion::for_char(c) {
            return self.motion(motion, text, cur);
        }
        match c {
            'f' | 'F' | 't' | 'T' => {
                self.pending = Some(Pending::Find(FindKind {
                    forward: c.is_lowercase(),
                    till: c.eq_ignore_ascii_case(&'t'),
                }));
                return Step::More;
            }
            ';' | ',' => {
                let Some((kind, ch)) = self.last_find else {
                    return abort();
                };
                let kind = if c == ',' { kind.reversed() } else { kind };
                return self.motion(
                    Motion::Find {
                        kind,
                        ch,
                        repeat: true,
                    },
                    text,
                    cur,
                );
            }
            'g' => {
                self.pending = Some(Pending::G);
                return Step::More;
            }
            'i' | 'a' if visual || self.operator.is_some() => {
                self.pending = Some(Pending::Object { inner: c == 'i' });
                return Step::More;
            }
            _ => {}
        }
        if visual {
            return self.visual_command(c, text, cur);
        }
        if let Some(operator) = Operator::for_char(c) {
            return match self.operator {
                // `dd`, `cc`, `yy`: the count's lines from the cursor's.
                Some(current) if current == operator => {
                    let last = down_lines(text, cur, self.total_count() - 1);
                    self.operate(operator, Span::Lines(cur, last), text, cur)
                }
                Some(_) => abort(),
                None => {
                    self.operator = Some(operator);
                    Step::More
                }
            };
        }
        if self.operator.is_some() {
            return abort();
        }
        self.normal_command(c, text, cur)
    }

    /// Whether a `0` continues a count rather than going to the line start.
    fn counting(&self) -> bool {
        if self.operator.is_some() {
            self.op_count.is_some()
        } else {
            self.count.is_some()
        }
    }

    fn escape(&mut self, text: &str) -> Step {
        if self.mode.is_visual() {
            self.mode = Mode::Normal;
            self.shown = 0..0;
            return done(vec![Op::Cursor(clamp_normal(text, self.head))]);
        }
        if self.operator.is_some() || self.count.is_some() {
            return abort();
        }
        Step::Pass
    }

    /// The commands of Normal mode that are neither motions nor operators.
    fn normal_command(&mut self, c: char, text: &str, cur: usize) -> Step {
        let count = self.total_count();
        match c {
            'i' => self.insert_at(cur),
            'a' => self.insert_at(if line_is_empty(text, cur) {
                cur
            } else {
                next(text, cur)
            }),
            'I' => self.insert_at(first_non_blank(text, cur)),
            'A' => self.insert_at(line_end(text, cur)),
            'o' => {
                let (end, newline) = (line_end(text, cur), newline(text));
                self.mode = Mode::Insert;
                changed(vec![
                    edit(end..end, newline),
                    Op::Cursor(end + newline.len()),
                ])
            }
            'O' => {
                let start = line_start(text, cur);
                self.mode = Mode::Insert;
                changed(vec![edit(start..start, newline(text)), Op::Cursor(start)])
            }
            'x' | 's' | 'X' => {
                let (motion, operator) = match c {
                    'x' => (Motion::Right, Operator::Delete),
                    's' => (Motion::Right, Operator::Change),
                    _ => (Motion::Left, Operator::Delete),
                };
                self.operator = Some(operator);
                self.motion(motion, text, cur)
            }
            'D' | 'C' => {
                self.operator = Some(if c == 'D' {
                    Operator::Delete
                } else {
                    Operator::Change
                });
                self.motion(Motion::LineEnd, text, cur)
            }
            'S' => {
                let last = down_lines(text, cur, count - 1);
                self.operate(Operator::Change, Span::Lines(cur, last), text, cur)
            }
            'Y' => {
                let last = down_lines(text, cur, count - 1);
                self.operate(Operator::Yank, Span::Lines(cur, last), text, cur)
            }
            'p' | 'P' => self.paste(c == 'p', text, cur, count),
            'r' => {
                self.pending = Some(Pending::Replace);
                Step::More
            }
            'J' => self.join(text, cur, count.max(2) - 1),
            '~' => {
                let end = forward_in_line(text, cur, count);
                if end == cur {
                    return abort();
                }
                let after = clamp_normal(text, end);
                let ops = vec![
                    edit(cur..end, &toggle_case(&text[cur..end])),
                    Op::Cursor(after),
                ];
                changed(ops)
            }
            'u' => done(vec![Op::Undo]),
            'v' | 'V' => {
                self.mode = if c == 'v' {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
                self.anchor = cur;
                self.head = cur;
                done(vec![self.show(text)])
            }
            '.' => self.repeat(text, cur),
            '/' => done(vec![Op::Find]),
            _ => abort(),
        }
    }

    fn insert_at(&mut self, at: usize) -> Step {
        self.mode = Mode::Insert;
        changed(vec![Op::Cursor(at)])
    }

    fn pending_key(&mut self, pending: Pending, key: Key, text: &str, cur: usize) -> Step {
        let Key::Char(ch) = key else {
            return abort();
        };
        match pending {
            Pending::Find(kind) => {
                self.last_find = Some((kind, ch));
                self.motion(
                    Motion::Find {
                        kind,
                        ch,
                        repeat: false,
                    },
                    text,
                    cur,
                )
            }
            Pending::G if ch == 'g' => self.motion(Motion::FirstLine, text, cur),
            Pending::G => abort(),
            Pending::Object { inner } => {
                let Some(range) = text_object(text, cur, ch, inner) else {
                    return abort();
                };
                if self.mode.is_visual() {
                    self.mode = Mode::Visual;
                    self.anchor = range.start;
                    self.head = prev(text, range.end).max(range.start);
                    return done(vec![self.show(text)]);
                }
                match self.operator {
                    Some(operator) => self.operate(operator, Span::Chars(range), text, cur),
                    None => abort(),
                }
            }
            Pending::Replace if self.mode.is_visual() => {
                let range = self.visual_chars(text);
                self.leave_visual();
                let replaced: String = text[range.clone()]
                    .chars()
                    .map(|c| if c == '\n' || c == '\r' { c } else { ch })
                    .collect();
                done(vec![
                    edit(range.clone(), &replaced),
                    Op::Cursor(range.start),
                ])
            }
            Pending::Replace => {
                let count = self.total_count();
                let end = forward_in_line(text, cur, count);
                if text[cur..end].chars().count() < count {
                    return abort();
                }
                let replaced = ch.to_string().repeat(count);
                let last = cur + replaced.len() - ch.len_utf8();
                changed(vec![edit(cur..end, &replaced), Op::Cursor(last)])
            }
        }
    }

    /// A motion: move, extend the selection, or give an operator its span.
    fn motion(&mut self, motion: Motion, text: &str, cur: usize) -> Step {
        let Some((target, kind)) = self.target(motion, text, cur) else {
            return abort();
        };
        if let Some(operator) = self.operator {
            let span = match kind {
                Kind::Linewise => Span::Lines(cur, target),
                Kind::Exclusive => Span::Chars(cur.min(target)..cur.max(target)),
                Kind::Inclusive => {
                    let (low, high) = (cur.min(target), cur.max(target));
                    Span::Chars(low..inclusive_end(text, high))
                }
            };
            return self.operate(operator, span, text, cur);
        }
        if self.mode.is_visual() {
            self.head = clamp_normal(text, target);
            return done(vec![self.show(text)]);
        }
        done(vec![Op::Cursor(clamp_normal(text, target))])
    }

    /// Where `motion` goes from `cur`, and how an operator takes it; `None`
    /// when it cannot go anywhere.
    fn target(&mut self, motion: Motion, text: &str, cur: usize) -> Option<(usize, Kind)> {
        let count = self.total_count();
        let operating = self.operator.is_some();
        if motion.is_vertical() {
            self.vertical = true;
        }
        Some(match motion {
            Motion::Left => {
                let start = line_start(text, cur);
                let mut at = cur;
                for _ in 0..count {
                    if at <= start {
                        break;
                    }
                    at = prev(text, at);
                }
                (at, Kind::Exclusive)
            }
            Motion::Right => {
                // An operator may take the last character; a move stops on it.
                let limit = if operating {
                    line_end(text, cur)
                } else {
                    last_in_line(text, cur)
                };
                let mut at = cur;
                for _ in 0..count {
                    if at >= limit {
                        break;
                    }
                    at = next(text, at);
                }
                (at, Kind::Exclusive)
            }
            Motion::BackWrap => {
                let mut at = cur;
                for _ in 0..count {
                    at = clip(text, prev(text, at));
                }
                (at, Kind::Exclusive)
            }
            Motion::ForwardWrap => {
                let mut at = cur;
                for _ in 0..count {
                    let end = line_end(text, at);
                    let after = next(text, at);
                    at = if after >= end {
                        after_break(text, end)
                    } else {
                        after
                    };
                }
                (at, Kind::Exclusive)
            }
            Motion::Up | Motion::Down => {
                let start = line_start(text, cur);
                let line = if matches!(motion, Motion::Down) {
                    down_lines(text, cur, count)
                } else {
                    up_lines(text, cur, count)
                };
                if line == start {
                    return None;
                }
                let column = *self.want_column.get_or_insert_with(|| column(text, cur));
                (at_column(text, line, column), Kind::Linewise)
            }
            Motion::LineStart => (line_start(text, cur), Kind::Exclusive),
            Motion::FirstNonBlank => (first_non_blank(text, cur), Kind::Exclusive),
            Motion::LineEnd => {
                self.vertical = true;
                self.want_column = Some(usize::MAX);
                let line = down_lines(text, cur, count - 1);
                (last_in_line(text, line), Kind::Inclusive)
            }
            Motion::NextLine | Motion::PreviousLine => {
                let line = if matches!(motion, Motion::NextLine) {
                    down_lines(text, cur, count)
                } else {
                    up_lines(text, cur, count)
                };
                if line == line_start(text, cur) {
                    return None;
                }
                (first_non_blank(text, line), Kind::Linewise)
            }
            Motion::WordStart { big } => {
                // `cw` on a word changes to its end, like `ce`.
                if self.operator == Some(Operator::Change)
                    && char_at(text, cur).is_some_and(|c| !c.is_whitespace())
                {
                    let mut at = cur;
                    for ix in 0..count {
                        at = if ix == 0 && word_ends_here(text, cur, big) {
                            cur
                        } else {
                            word_end_forward(text, at, big)
                        };
                    }
                    return Some((at, Kind::Inclusive));
                }
                let mut at = cur;
                for _ in 0..count {
                    at = word_start_forward(text, at, big);
                }
                // An operator stops at the end of the line its last word
                // was on, not at the first word of the next line.
                let target_line = line_start(text, at);
                if operating && target_line > line_start(text, cur) {
                    at = break_before(text, target_line).max(cur);
                }
                (at, Kind::Exclusive)
            }
            Motion::WordEnd { big } => {
                let mut at = cur;
                for _ in 0..count {
                    at = word_end_forward(text, at, big);
                }
                (at, Kind::Inclusive)
            }
            Motion::WordBack { big } => {
                let mut at = cur;
                for _ in 0..count {
                    at = word_start_backward(text, at, big);
                }
                (at, Kind::Exclusive)
            }
            Motion::Paragraph { forward } => {
                let mut at = cur;
                for _ in 0..count {
                    at = if forward {
                        paragraph_forward(text, at)
                    } else {
                        paragraph_backward(text, at)
                    };
                }
                (at, Kind::Exclusive)
            }
            Motion::Bracket => (matching_bracket(text, cur)?, Kind::Inclusive),
            Motion::Find { kind, ch, repeat } => (
                find_in_line(text, cur, kind, ch, count, repeat)?,
                if kind.forward {
                    Kind::Inclusive
                } else {
                    Kind::Exclusive
                },
            ),
            Motion::LastLine | Motion::FirstLine => {
                let line = match self.explicit_count() {
                    Some(n) => nth_line_start(text, n),
                    None if matches!(motion, Motion::LastLine) => line_start(text, text.len()),
                    None => 0,
                };
                (first_non_blank(text, line), Kind::Linewise)
            }
        })
    }

    /// Apply an operator to a span.
    fn operate(&mut self, operator: Operator, span: Span, text: &str, cur: usize) -> Step {
        match (operator, span) {
            (Operator::Yank, Span::Chars(range)) => {
                self.register = Register {
                    text: text[range.clone()].to_string(),
                    linewise: false,
                };
                done(vec![Op::Cursor(clamp_normal(text, range.start))])
            }
            (Operator::Yank, Span::Lines(a, b)) => {
                let (first, last) = (line_start(text, a.min(b)), line_end(text, a.max(b)));
                self.register = Register {
                    text: lines_text(text, first, last),
                    linewise: true,
                };
                let to = if line_start(text, cur) == first {
                    cur
                } else {
                    at_column(text, first, column(text, cur))
                };
                done(vec![Op::Cursor(to)])
            }
            (Operator::Delete, Span::Chars(range)) => {
                if range.is_empty() {
                    return abort();
                }
                self.register = Register {
                    text: text[range.clone()].to_string(),
                    linewise: false,
                };
                let rest = spliced(text, range.clone(), "");
                let to = clamp_normal(&rest, range.start);
                changed(vec![edit(range, ""), Op::Cursor(to)])
            }
            (Operator::Delete, Span::Lines(a, b)) => {
                let (first, last) = (line_start(text, a.min(b)), line_end(text, a.max(b)));
                self.register = Register {
                    text: lines_text(text, first, last),
                    linewise: true,
                };
                let range = if last < text.len() {
                    first..after_break(text, last)
                } else {
                    // The last line has no line break after it: take the one
                    // before, so no empty line is left behind.
                    break_before(text, first)..last
                };
                let rest = spliced(text, range.clone(), "");
                let to = first_non_blank(&rest, range.start.min(rest.len()));
                changed(vec![edit(range, ""), Op::Cursor(clamp_normal(&rest, to))])
            }
            (Operator::Change, Span::Chars(range)) => {
                self.register = Register {
                    text: text[range.clone()].to_string(),
                    linewise: false,
                };
                self.mode = Mode::Insert;
                changed(vec![edit(range.clone(), ""), Op::Cursor(range.start)])
            }
            (Operator::Change, Span::Lines(a, b)) => {
                let (first, last) = (line_start(text, a.min(b)), line_end(text, a.max(b)));
                self.register = Register {
                    text: lines_text(text, first, last),
                    linewise: true,
                };
                // The first line's indent stays, as with 'autoindent'.
                let from = first_non_blank(text, first).min(last);
                self.mode = Mode::Insert;
                changed(vec![edit(from..last, ""), Op::Cursor(from)])
            }
        }
    }

    fn paste(&mut self, after: bool, text: &str, cur: usize, count: usize) -> Step {
        if self.register.text.is_empty() {
            return abort();
        }
        let content = self.register.text.repeat(count);
        if self.register.linewise {
            let end = line_end(text, cur);
            let (at, insert, first_line) = if !after {
                let start = line_start(text, cur);
                (start, content, start)
            } else if end < text.len() {
                let next_line = after_break(text, end);
                (next_line, content, next_line)
            } else {
                // After the last line, which has no line break of its own.
                let newline = newline(text);
                let body = content.trim_end_matches(['\r', '\n']);
                (end, format!("{newline}{body}"), end + newline.len())
            };
            let result = spliced(text, at..at, &insert);
            let to = clamp_normal(&result, first_non_blank(&result, first_line));
            return changed(vec![edit(at..at, &insert), Op::Cursor(to)]);
        }
        let at = if after && !line_is_empty(text, cur) {
            next(text, cur)
        } else {
            cur
        };
        let result = spliced(text, at..at, &content);
        let to = clamp_normal(&result, prev(&result, at + content.len()));
        changed(vec![edit(at..at, &content), Op::Cursor(to)])
    }

    /// `J`: join `joins` lines onto the cursor's, a space between each.
    fn join(&mut self, text: &str, cur: usize, joins: usize) -> Step {
        let mut result = text.to_string();
        let mut ops = Vec::new();
        let mut at = cur;
        for _ in 0..joins {
            let end = line_end(&result, at);
            if end >= result.len() {
                break;
            }
            let next_start = after_break(&result, end);
            let body_start = first_non_blank(&result, next_start);
            let next_line_empty = body_start == line_end(&result, next_start);
            let ends_blank =
                end == line_start(&result, end) || result[..end].ends_with([' ', '\t']);
            let separator =
                if next_line_empty || ends_blank || char_at(&result, body_start) == Some(')') {
                    ""
                } else {
                    " "
                };
            ops.push(edit(end..body_start, separator));
            result.replace_range(end..body_start, separator);
            at = end;
        }
        if ops.is_empty() {
            return abort();
        }
        ops.push(Op::Cursor(clamp_normal(&result, at)));
        changed(ops)
    }

    /// `.`: the last change again, at the cursor.
    fn repeat(&mut self, text: &str, cur: usize) -> Step {
        let Some(change) = self.last_change.clone() else {
            return abort();
        };
        let mut keys = change.keys;
        if let Some(count) = self.count {
            let digits = keys
                .iter()
                .take_while(|key| matches!(key, Key::Char(c) if c.is_ascii_digit()))
                .count();
            keys.splice(..digits, count.to_string().chars().map(Key::Char));
        }
        self.end_command();
        self.replaying = true;
        let mut buffer = text.to_string();
        let mut cursor = cur;
        let mut ops = Vec::new();
        for key in keys {
            if let Outcome::Handled(step) = self.normal_key(key, &buffer, cursor) {
                cursor = simulate(&mut buffer, cursor, &step);
                ops.extend(step);
            }
        }
        if self.mode == Mode::Insert {
            if let Some(insert) = change.insert {
                let typed = vec![edit(cursor..cursor, &insert)];
                cursor = simulate(&mut buffer, cursor, &typed);
                ops.extend(typed);
            }
            self.mode = Mode::Normal;
            ops.push(Op::Cursor(step_back_in_line(&buffer, cursor)));
        }
        self.replaying = false;
        done(ops)
    }

    /// The commands of Visual mode that are neither motions nor objects.
    fn visual_command(&mut self, c: char, text: &str, cur: usize) -> Step {
        let linewise = self.mode == Mode::VisualLine;
        let span = if linewise {
            Span::Lines(self.anchor, self.head)
        } else {
            Span::Chars(self.visual_chars(text))
        };
        let start = match &span {
            Span::Chars(range) => range.start,
            Span::Lines(a, b) => line_start(text, *a.min(b)),
        };
        match c {
            'o' => {
                std::mem::swap(&mut self.anchor, &mut self.head);
                done(vec![self.show(text)])
            }
            'v' | 'V' => {
                let wanted = if c == 'v' {
                    Mode::Visual
                } else {
                    Mode::VisualLine
                };
                if self.mode == wanted {
                    self.leave_visual();
                    return done(vec![Op::Cursor(cur)]);
                }
                self.mode = wanted;
                done(vec![self.show(text)])
            }
            'd' | 'x' | 'X' | 'D' | 'c' | 's' | 'y' => {
                let operator = match c {
                    'c' | 's' => Operator::Change,
                    'y' => Operator::Yank,
                    _ => Operator::Delete,
                };
                self.leave_visual();
                // `X` and `D` take whole lines in Visual mode, as in Vim.
                let span = match (c, span) {
                    ('X' | 'D', Span::Chars(_)) => Span::Lines(self.anchor, self.head),
                    (_, span) => span,
                };
                let at = if operator == Operator::Yank {
                    start
                } else {
                    cur
                };
                self.operate(operator, span, text, at)
            }
            'p' | 'P' => {
                if self.register.text.is_empty() {
                    return abort();
                }
                let range = match span {
                    Span::Chars(range) => range,
                    Span::Lines(a, b) => line_start(text, a.min(b))..line_end(text, a.max(b)),
                };
                let content = match (linewise, self.register.linewise) {
                    (true, true) => self
                        .register
                        .text
                        .trim_end_matches(['\r', '\n'])
                        .to_string(),
                    (false, true) => format!("{}{}", newline(text), self.register.text),
                    _ => self.register.text.clone(),
                };
                self.leave_visual();
                let result = spliced(text, range.clone(), &content);
                let to = clamp_normal(&result, range.start);
                done(vec![edit(range, &content), Op::Cursor(to)])
            }
            'J' => {
                let (a, b) = (self.anchor.min(self.head), self.anchor.max(self.head));
                let joins = text[line_start(text, a)..line_end(text, b)]
                    .matches('\n')
                    .count()
                    .max(1);
                self.leave_visual();
                self.join(text, a, joins)
            }
            '~' | 'u' | 'U' => {
                let range = match span {
                    Span::Chars(range) => range,
                    Span::Lines(a, b) => line_start(text, a.min(b))..line_end(text, a.max(b)),
                };
                let original = &text[range.clone()];
                let mapped = match c {
                    '~' => toggle_case(original),
                    'u' => original.to_lowercase(),
                    _ => original.to_uppercase(),
                };
                self.leave_visual();
                done(vec![edit(range.clone(), &mapped), Op::Cursor(range.start)])
            }
            'r' => {
                self.pending = Some(Pending::Replace);
                Step::More
            }
            _ => done(Vec::new()),
        }
    }

    fn leave_visual(&mut self) {
        self.mode = Mode::Normal;
        self.shown = 0..0;
    }

    /// The characters a charwise Visual selection covers, both ends in.
    fn visual_chars(&self, text: &str) -> Range<usize> {
        let (low, high) = (self.anchor.min(self.head), self.anchor.max(self.head));
        clip(text, low)..char_end(text, clip(text, high))
    }

    /// Show the Visual selection.
    fn show(&mut self, text: &str) -> Op {
        let range = if self.mode == Mode::VisualLine {
            let (low, high) = (self.anchor.min(self.head), self.anchor.max(self.head));
            line_start(text, low)..after_break(text, line_end(text, high))
        } else {
            self.visual_chars(text)
        };
        self.shown = range.clone();
        Op::Select(range)
    }
}

fn edit(range: Range<usize>, text: &str) -> Op {
    Op::Edit {
        range,
        text: text.to_string(),
    }
}

/// The last cursor place in `ops`.
fn final_cursor(ops: &[Op]) -> Option<usize> {
    ops.iter().rev().find_map(|op| match op {
        Op::Cursor(at) => Some(*at),
        _ => None,
    })
}

/// Apply `ops` to a copy of the text, as the editor would; returns the
/// cursor after them. Undo and the other actions are not modelled.
fn simulate(text: &mut String, mut cursor: usize, ops: &[Op]) -> usize {
    for op in ops {
        match op {
            Op::Edit { range, text: new } => {
                text.replace_range(range.clone(), new);
                cursor = range.start + new.len();
            }
            Op::Cursor(at) => cursor = *at,
            Op::Select(range) => cursor = range.end,
            Op::Undo | Op::Redo | Op::PageDown | Op::PageUp | Op::Find => {}
        }
    }
    cursor
}

fn spliced(text: &str, range: Range<usize>, insert: &str) -> String {
    let mut result = text.to_string();
    result.replace_range(range, insert);
    result
}

// --- Positions -------------------------------------------------------------

fn clip(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    // Never between the two characters of a CRLF line break.
    if at > 0 && text.as_bytes()[at - 1] == b'\r' && text.as_bytes().get(at) == Some(&b'\n') {
        at -= 1;
    }
    at
}

fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..)?.chars().next()
}

fn next(text: &str, at: usize) -> usize {
    at + char_at(text, at).map_or(0, char::len_utf8)
}

fn prev(text: &str, at: usize) -> usize {
    text[..at]
        .chars()
        .next_back()
        .map_or(at, |c| at - c.len_utf8())
}

fn line_start(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |ix| ix + 1)
}

/// The end of the line's text: its line break (`\n`, or the `\r` of
/// `\r\n`), or the end of the text.
fn line_end(text: &str, at: usize) -> usize {
    let at = clip(text, at);
    match text[at..].find('\n') {
        Some(ix) if ix > 0 && text.as_bytes()[at + ix - 1] == b'\r' => at + ix - 1,
        Some(ix) => at + ix,
        None => text.len(),
    }
}

/// The start of the next line, past the line break at `end` (a
/// [`line_end`]); `end` itself on the last line.
fn after_break(text: &str, end: usize) -> usize {
    if text[end..].starts_with("\r\n") {
        end + 2
    } else if text[end..].starts_with('\n') {
        end + 1
    } else {
        end
    }
}

/// Where the line break before the line starting at `start` begins.
fn break_before(text: &str, start: usize) -> usize {
    if text[..start].ends_with("\r\n") {
        start - 2
    } else if text[..start].ends_with('\n') {
        start - 1
    } else {
        start
    }
}

/// The text's line break: CRLF when it has one, else LF.
fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The lines from `first` (a line start) to `last` (a line end), as a
/// linewise register holds them: ending with a line break.
fn lines_text(text: &str, first: usize, last: usize) -> String {
    format!("{}{}", &text[first..last], newline(text))
}

/// After the character at `at`; a CRLF line break counts as one.
fn char_end(text: &str, at: usize) -> usize {
    if text[at..].starts_with("\r\n") {
        at + 2
    } else {
        next(text, at)
    }
}

fn line_is_empty(text: &str, at: usize) -> bool {
    line_start(text, at) == line_end(text, at)
}

/// The last character of the line, or its start when it is empty.
fn last_in_line(text: &str, at: usize) -> usize {
    let (start, end) = (line_start(text, at), line_end(text, at));
    if start == end { start } else { prev(text, end) }
}

/// Where Normal mode may put the cursor: on a character of the line.
fn clamp_normal(text: &str, at: usize) -> usize {
    let at = clip(text, at);
    at.min(last_in_line(text, at))
}

/// One character left, but not onto the line above: leaving Insert mode.
fn step_back_in_line(text: &str, at: usize) -> usize {
    if at > line_start(text, at) {
        prev(text, at)
    } else {
        at
    }
}

/// `count` characters right of `at`, stopping at the end of the line.
fn forward_in_line(text: &str, at: usize, count: usize) -> usize {
    let end = line_end(text, at);
    let mut to = at;
    for _ in 0..count {
        if to >= end {
            break;
        }
        to = next(text, to);
    }
    to
}

/// The end of an inclusive span ending on the character at `at`. A newline
/// stays: an inclusive motion onto an empty line takes nothing of it.
fn inclusive_end(text: &str, at: usize) -> usize {
    if at == line_end(text, at) {
        at
    } else {
        next(text, at)
    }
}

fn first_non_blank(text: &str, at: usize) -> usize {
    let (start, end) = (line_start(text, at), line_end(text, at));
    start
        + text[start..end]
            .find(|c: char| c != ' ' && c != '\t')
            .unwrap_or(end - start)
}

/// The cursor's column, in characters.
fn column(text: &str, at: usize) -> usize {
    text[line_start(text, at)..at].chars().count()
}

/// The offset `column` characters into the line starting at `start`, or
/// its last character.
fn at_column(text: &str, start: usize, column: usize) -> usize {
    let end = line_end(text, start);
    let at = text[start..end]
        .char_indices()
        .nth(column)
        .map_or(end, |(ix, _)| start + ix);
    clamp_normal(text, at)
}

/// The start of the line `count` lines below `at`'s, or of the last line.
fn down_lines(text: &str, at: usize, count: usize) -> usize {
    let mut start = line_start(text, at);
    for _ in 0..count {
        let end = line_end(text, start);
        if end >= text.len() {
            break;
        }
        start = after_break(text, end);
    }
    start
}

/// The start of the line `count` lines above `at`'s, or of the first line.
fn up_lines(text: &str, at: usize, count: usize) -> usize {
    let mut start = line_start(text, at);
    for _ in 0..count {
        if start == 0 {
            break;
        }
        start = line_start(text, start - 1);
    }
    start
}

/// The start of line `n` (1-based), or of the last line.
fn nth_line_start(text: &str, n: usize) -> usize {
    down_lines(text, 0, n.saturating_sub(1))
}

// --- Words -----------------------------------------------------------------

/// Han, kana and Hangul run as words of their own, apart from Latin ones.
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
        | 0xAC00..=0xD7AF | 0x20000..=0x2FA1F)
}

/// Vim's character classes: 0 blank, 1 punctuation, 2 word, 3 CJK. A
/// WORD (`big`) is any run of non-blanks.
fn class(c: char, big: bool) -> u8 {
    if c.is_whitespace() {
        0
    } else if big {
        2
    } else if is_cjk(c) {
        3
    } else if c == '_' || c.is_alphanumeric() {
        2
    } else {
        1
    }
}

/// `w`: the start of the next word, stopping at an empty line.
fn word_start_forward(text: &str, at: usize, big: bool) -> usize {
    let mut ix = at;
    if let Some(c) = char_at(text, ix) {
        let own = class(c, big);
        if own != 0 {
            while let Some(c) = char_at(text, ix) {
                if class(c, big) != own {
                    break;
                }
                ix += c.len_utf8();
            }
        }
    }
    let mut crossed_line = false;
    while let Some(c) = char_at(text, ix) {
        if c == '\n' {
            if crossed_line && line_is_empty(text, ix) {
                return line_start(text, ix);
            }
            crossed_line = true;
        } else if !c.is_whitespace() {
            break;
        }
        ix += c.len_utf8();
    }
    ix
}

/// Whether the character at `at` is the last of its word.
fn word_ends_here(text: &str, at: usize, big: bool) -> bool {
    let Some(c) = char_at(text, at) else {
        return true;
    };
    char_at(text, next(text, at)).is_none_or(|after| class(after, big) != class(c, big))
}

/// `e`: the end of the word, or of the next one when already there.
fn word_end_forward(text: &str, at: usize, big: bool) -> usize {
    let mut ix = next(text, at);
    while let Some(c) = char_at(text, ix) {
        if !c.is_whitespace() {
            break;
        }
        ix += c.len_utf8();
    }
    let Some(c) = char_at(text, ix) else {
        return prev(text, text.len()).max(at);
    };
    let own = class(c, big);
    loop {
        let after = next(text, ix);
        match char_at(text, after) {
            Some(d) if class(d, big) == own => ix = after,
            _ => return ix,
        }
    }
}

/// `b`: the start of the word, or of the one before when already there,
/// stopping at an empty line.
fn word_start_backward(text: &str, at: usize, big: bool) -> usize {
    let mut ix = at;
    while ix > 0 {
        let before = prev(text, ix);
        let c = char_at(text, before).unwrap_or(' ');
        if !c.is_whitespace() {
            break;
        }
        if (c == '\n' || c == '\r')
            && line_start(text, before) != line_start(text, at)
            && line_is_empty(text, before)
        {
            return line_start(text, before);
        }
        ix = before;
    }
    if ix == 0 {
        return 0;
    }
    ix = prev(text, ix);
    let own = class(char_at(text, ix).unwrap_or(' '), big);
    while ix > 0 {
        let before = prev(text, ix);
        if char_at(text, before).is_none_or(|c| class(c, big) != own) {
            break;
        }
        ix = before;
    }
    ix
}

// --- Paragraphs and brackets -------------------------------------------------

/// `}`: the next empty line after a paragraph, or the end.
fn paragraph_forward(text: &str, at: usize) -> usize {
    let mut start = line_start(text, at);
    let mut seen_text = false;
    loop {
        let end = line_end(text, start);
        if start == end && seen_text && start > line_start(text, at) {
            return start;
        }
        if start != end {
            seen_text = true;
        }
        if end >= text.len() {
            return text.len();
        }
        start = after_break(text, end);
    }
}

/// `{`: the empty line before the paragraph, or the start.
fn paragraph_backward(text: &str, at: usize) -> usize {
    let mut start = line_start(text, at);
    let mut seen_text = false;
    loop {
        let blank = start == line_end(text, start);
        if blank && seen_text && start < line_start(text, at) {
            return start;
        }
        if !blank {
            seen_text = true;
        }
        if start == 0 {
            return 0;
        }
        start = line_start(text, start - 1);
    }
}

const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

/// `%`: the bracket matching the first one at or after the cursor on its
/// line.
fn matching_bracket(text: &str, at: usize) -> Option<usize> {
    let end = line_end(text, at);
    let (ix, c) = text[at..end]
        .char_indices()
        .find(|(_, c)| PAIRS.iter().any(|(open, close)| c == open || c == close))
        .map(|(ix, c)| (at + ix, c))?;
    let &(open, close) = PAIRS.iter().find(|(o, cl)| c == *o || c == *cl)?;
    if c == open {
        closing_after(text, ix, open, close)
    } else {
        opening_before(text, ix, open, close)
    }
}

/// The bracket closing the one opened at `open_at`.
fn closing_after(text: &str, open_at: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    for (ix, c) in text[open_at..].char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(open_at + ix);
            }
        }
    }
    None
}

/// The bracket opening the one closed at `close_at`.
fn opening_before(text: &str, close_at: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    for (ix, c) in text[..=close_at].char_indices().rev() {
        if c == close {
            depth += 1;
        } else if c == open {
            depth -= 1;
            if depth == 0 {
                return Some(ix);
            }
        }
    }
    None
}

// --- Finding and text objects ------------------------------------------------

/// `f`, `t`, `F`, `T`: the `count`th `ch` on the line. A repeated `t`
/// skips a match right beside the cursor, or it would never move.
fn find_in_line(
    text: &str,
    cur: usize,
    kind: FindKind,
    ch: char,
    count: usize,
    repeat: bool,
) -> Option<usize> {
    let (start, end) = (line_start(text, cur), line_end(text, cur));
    if kind.forward {
        let mut from = next(text, cur).min(end);
        if kind.till && repeat {
            from = next(text, from).min(end);
        }
        let found = text[from..end]
            .match_indices(ch)
            .nth(count - 1)
            .map(|(ix, _)| from + ix)?;
        Some(if kind.till { prev(text, found) } else { found })
    } else {
        let mut to = cur;
        if kind.till && repeat {
            to = prev(text, to).max(start);
        }
        let found = text[start..to]
            .rmatch_indices(ch)
            .nth(count - 1)
            .map(|(ix, _)| start + ix)?;
        Some(if kind.till { next(text, found) } else { found })
    }
}

/// `iw`, `aw`, `i"`, `a(`, …: the range the object covers around `at`.
fn text_object(text: &str, at: usize, c: char, inner: bool) -> Option<Range<usize>> {
    match c {
        'w' | 'W' => word_object(text, at, inner, c == 'W'),
        '"' | '\'' | '`' => quote_object(text, at, c, inner),
        '(' | ')' | 'b' => bracket_object(text, at, '(', ')', inner),
        '[' | ']' => bracket_object(text, at, '[', ']', inner),
        '{' | '}' | 'B' => bracket_object(text, at, '{', '}', inner),
        '<' | '>' => bracket_object(text, at, '<', '>', inner),
        _ => None,
    }
}

fn word_object(text: &str, at: usize, inner: bool, big: bool) -> Option<Range<usize>> {
    let breaks = |c: char| c == '\n' || c == '\r';
    let c = char_at(text, at).filter(|&c| !breaks(c))?;
    let own = class(c, big);
    let same = |c: char| !breaks(c) && class(c, big) == own;
    let run_start = |mut ix: usize| {
        while ix > 0 && char_at(text, prev(text, ix)).is_some_and(same) {
            ix = prev(text, ix);
        }
        ix
    };
    let run_end = |mut ix: usize, test: &dyn Fn(char) -> bool| {
        while char_at(text, ix).is_some_and(test) {
            ix = next(text, ix);
        }
        ix
    };
    let (start, end) = (run_start(at), run_end(at, &same));
    if inner {
        return Some(start..end);
    }
    let blank = |c: char| c == ' ' || c == '\t';
    if own == 0 {
        // Blanks, then the word after them.
        let word = char_at(text, end).filter(|&c| !breaks(c));
        let after = match word {
            Some(w) => run_end(end, &|c: char| !breaks(c) && class(c, big) == class(w, big)),
            None => end,
        };
        return Some(start..after);
    }
    let trailing = run_end(end, &blank);
    if trailing > end {
        return Some(start..trailing);
    }
    let mut leading = start;
    while leading > line_start(text, at) && char_at(text, prev(text, leading)).is_some_and(blank) {
        leading = prev(text, leading);
    }
    Some(leading..end)
}

fn quote_object(text: &str, at: usize, quote: char, inner: bool) -> Option<Range<usize>> {
    let (start, end) = (line_start(text, at), line_end(text, at));
    let quotes: Vec<usize> = text[start..end]
        .match_indices(quote)
        .map(|(ix, _)| start + ix)
        .collect();
    // Quotes pair up from the start of the line: 1st with 2nd, 3rd with 4th.
    let pairs = quotes.as_chunks::<2>().0;
    let &[open, close] = pairs
        .iter()
        .find(|&&[open, close]| open <= at && at <= close)
        .or_else(|| pairs.iter().find(|&&[open, _]| open > at))?;
    Some(if inner {
        open + quote.len_utf8()..close
    } else {
        open..close + quote.len_utf8()
    })
}

fn bracket_object(
    text: &str,
    at: usize,
    open: char,
    close: char,
    inner: bool,
) -> Option<Range<usize>> {
    // Back to the bracket that opens around the cursor; a closing bracket
    // under the cursor is its own pair's end.
    let mut depth = 0usize;
    let mut start = None;
    let upto = next(text, at).min(text.len());
    for (ix, c) in text[..upto].char_indices().rev() {
        if c == close && ix != at {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                start = Some(ix);
                break;
            }
            depth -= 1;
        }
    }
    let start = start?;
    let end = closing_after(text, start, open, close)?;
    Some(if inner {
        start + open.len_utf8()..end
    } else {
        start..end + close.len_utf8()
    })
}

fn toggle_case(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            let swapped: Vec<char> = if c.is_lowercase() {
                c.to_uppercase().collect()
            } else {
                c.to_lowercase().collect()
            };
            swapped
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse keys: characters, with `<Esc>`, `<CR>`, `<BS>`, `<Del>`,
    /// `<Tab>` and `<C-x>`.
    fn keys(spec: &str) -> Vec<Key> {
        let mut keys = Vec::new();
        let mut rest = spec;
        while let Some(c) = rest.chars().next() {
            if c == '<'
                && let Some(close) = rest.find('>')
                && close > 1
            {
                let name = &rest[1..close];
                keys.push(match name {
                    "Esc" => Key::Escape,
                    "CR" => Key::Enter,
                    "BS" => Key::Backspace,
                    "Del" => Key::Delete,
                    "Tab" => Key::Tab,
                    "Left" => Key::Left,
                    "Right" => Key::Right,
                    "Up" => Key::Up,
                    "Down" => Key::Down,
                    "Home" => Key::Home,
                    "End" => Key::End,
                    _ if name.starts_with("C-") => Key::Ctrl(name.chars().nth(2).unwrap()),
                    _ => panic!("unknown key {name}"),
                });
                rest = &rest[close + 1..];
                continue;
            }
            keys.push(Key::Char(c));
            rest = &rest[c.len_utf8()..];
        }
        keys
    }

    /// An editor in miniature: the text, the cursor, the selection.
    struct Editor {
        vim: Vim,
        text: String,
        cursor: usize,
        selection: Range<usize>,
        /// Actions asked for (undo and the like), by name.
        actions: Vec<Op>,
    }

    impl Editor {
        /// `|` marks the cursor.
        fn new(marked: &str) -> Self {
            let cursor = marked.find('|').expect("a cursor mark");
            let text = marked.replacen('|', "", 1);
            Self {
                vim: Vim::default(),
                text,
                cursor,
                selection: cursor..cursor,
                actions: Vec::new(),
            }
        }

        /// Type `spec`; a key Vim passes types its character, as the
        /// editor would.
        fn type_keys(&mut self, spec: &str) -> &mut Self {
            for key in keys(spec) {
                let outcome =
                    self.vim
                        .key(key.clone(), &self.text, self.cursor, self.selection.clone());
                match outcome {
                    Outcome::Pass => {
                        if let Key::Char(c) = key {
                            let at = self.cursor;
                            self.text.insert(at, c);
                            self.cursor = at + c.len_utf8();
                        } else if key == Key::Enter {
                            self.text.insert(self.cursor, '\n');
                            self.cursor += 1;
                        } else if key == Key::Backspace && self.cursor > 0 {
                            // As the editor does, a CRLF goes as one.
                            let at = if self.text[..self.cursor].ends_with("\r\n") {
                                self.cursor - 2
                            } else {
                                prev(&self.text, self.cursor)
                            };
                            self.text.replace_range(at..self.cursor, "");
                            self.cursor = at;
                        }
                        self.selection = self.cursor..self.cursor;
                    }
                    Outcome::Handled(ops) => {
                        for op in &ops {
                            match op {
                                Op::Select(range) => self.selection = range.clone(),
                                Op::Edit { .. } | Op::Cursor(_) => {}
                                other => self.actions.push(other.clone()),
                            }
                            if !matches!(op, Op::Select(_)) {
                                self.cursor =
                                    simulate(&mut self.text, self.cursor, std::slice::from_ref(op));
                                self.selection = self.cursor..self.cursor;
                            } else {
                                self.cursor = self.selection.end;
                            }
                        }
                    }
                }
            }
            self
        }

        /// The text with `|` at the cursor.
        fn marked(&self) -> String {
            let mut text = self.text.clone();
            text.insert(self.cursor, '|');
            text
        }

        /// The text with `[` `]` around the selection.
        fn selected(&self) -> String {
            let mut text = self.text.clone();
            text.insert(self.selection.end, ']');
            text.insert(self.selection.start, '[');
            text
        }

        fn mode(&self) -> Mode {
            self.vim.mode()
        }
    }

    /// Start from `marked`, type `spec`, expect `expected` (with `|`).
    #[track_caller]
    fn check(marked: &str, spec: &str, expected: &str) {
        let mut editor = Editor::new(marked);
        editor.type_keys(spec);
        assert_eq!(editor.marked(), expected, "{marked:?} + {spec:?}");
    }

    #[test]
    fn the_motions_move_the_cursor() {
        check("he|llo world", "l", "hel|lo world");
        check("he|llo world", "h", "h|ello world");
        check("he|llo world", "0", "|hello world");
        check("  he|llo", "^", "  |hello");
        check("he|llo world", "$", "hello worl|d");
        check("he|llo world", "w", "hello |world");
        check("hello |world", "b", "|hello world");
        check("he|llo world", "e", "hell|o world");
        check("he|llo world", "3l", "hello| world");
        check("|one two three four", "2w", "one two |three four");
        check("|a.b c", "w", "a|.b c");
        check("|a.b c", "W", "a.b |c");
        check("he|llo", "100l", "hell|o");
        check("|hello", "h", "|hello");
    }

    #[test]
    fn the_arrows_are_motions_outside_insert_mode() {
        check("he|llo", "<Right>", "hel|lo");
        check("he|llo", "<Left>", "h|ello");
        check("he|llo", "<End><Right>", "hell|o");
        check("he|llo", "<Home>", "|hello");
        check("ab|c\nabcdef", "<Down>", "abc\nab|cdef");
        check("abc\nab|cdef", "<Up>", "ab|c\nabcdef");
        check("he|llo", "2<Right>", "hell|o");
        check("he|llo", "d<End>", "h|e");
        // Visual mode keeps its selection and extends it.
        let mut editor = Editor::new("he|llo");
        editor.type_keys("v<Right><Right>");
        assert_eq!(editor.mode(), Mode::Visual);
        assert_eq!(editor.selected(), "he[llo]");
        // Insert mode leaves them to the editor.
        let mut vim = Vim::default();
        vim.key(Key::Char('i'), "abc", 0, 0..0);
        assert_eq!(vim.key(Key::Left, "abc", 1, 1..1), Outcome::Pass);
    }

    #[test]
    fn up_and_down_keep_the_column() {
        check("abc|def\nx\nabcdef", "j", "abcdef\n|x\nabcdef");
        check("abc|def\nx\nabcdef", "jj", "abcdef\nx\nabc|def");
        check("abcdef\nx\nabc|def", "kk", "abc|def\nx\nabcdef");
        check("ab|c\nabcdef", "$j", "abc\nabcde|f");
        check("one\n|two", "j", "one\n|two");
        check("|one\ntwo\nthree", "G", "one\ntwo\n|three");
        check("one\ntwo\nthr|ee", "gg", "|one\ntwo\nthree");
        check("|one\ntwo\nthree", "2G", "one\n|two\nthree");
        check("|one\n  two", "<CR>", "one\n  |two");
    }

    #[test]
    fn words_and_paragraphs_stop_where_vim_does() {
        check("foo|\n\nbar", "w", "foo\n|\nbar");
        check("foo\n\n|bar", "b", "foo\n|\nbar");
        check("|a\nb\n\nc\nd", "}", "a\nb\n|\nc\nd");
        check("a\nb\n\nc\n|d", "{", "a\nb\n|\nc\nd");
        check("|a\nb", "}", "a\n|b");
        check("|中文测试 abc", "w", "中文测试 |abc");
        check("|abc中文", "w", "abc|中文");
        check("|中文，测试", "w", "中文|，测试");
    }

    #[test]
    fn find_and_brackets() {
        check("|a,b,c", "f,", "a|,b,c");
        check("|a,b,c", "2f,", "a,b|,c");
        check("|a,b,c", "t,", "|a,b,c");
        check("|a,b,c", "f,;", "a,b|,c");
        check("|a,b,c", "t,;", "a,|b,c");
        check("a,b,|c", "F,", "a,b|,c");
        check("a,b,|c", "T,", "a,b,|c");
        check("|a,b,c", "2f,,", "a|,b,c");
        check("|f(a(b)c)", "%", "f(a(b)c|)");
        check("f(a(b)c|)", "%", "f|(a(b)c)");
        check("|abc", "fz", "|abc");
    }

    #[test]
    fn typing_in_normal_mode_inserts_nothing() {
        let mut editor = Editor::new("he|llo");
        editor.type_keys("qzQ<Tab>");
        assert_eq!(editor.marked(), "he|llo");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn insert_mode_types_and_escape_steps_back() {
        check("he|llo", "iXY<Esc>", "heX|Yllo");
        check("he|llo", "aX<Esc>", "hel|Xlo");
        check("  he|llo", "IX<Esc>", "  |Xhello");
        check("he|llo", "AX<Esc>", "hello|X");
        check("o|ne\ntwo", "oX<Esc>", "one\n|X\ntwo");
        check("one\nt|wo", "OX<Esc>", "one\n|X\ntwo");
        check("|", "iab<Esc>", "a|b");
        let mut editor = Editor::new("|x");
        editor.type_keys("i");
        assert_eq!(editor.mode(), Mode::Insert);
        editor.type_keys("<Esc>");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn delete_change_and_yank_take_their_motions() {
        check("one |two three", "dw", "one |three");
        check("one t|wo three", "dw", "one t|three");
        check("one |two three", "d2w", "one| ");
        check("one |two three", "2dw", "one| ");
        check("one |two three", "de", "one | three");
        check("one |two three", "d$", "one| ");
        check("one |two three", "D", "one| ");
        check("one |two three", "d0", "|two three");
        check("one |two three", "x", "one |wo three");
        check("one |two three", "3x", "one | three");
        check("one t|wo", "X", "one |wo");
        check("ab|c", "x", "a|b");
        check("one |two three", "cwX<Esc>", "one |X three");
        check("one |two three", "ceX<Esc>", "one |X three");
        check("one |two three", "CX<Esc>", "one |X");
        check("one |two three", "sX<Esc>", "one |Xwo three");
        check("one |two\nnext", "dw", "one| \nnext");
        check("a,b,|c,d", "dt,", "a,b,|,d");
        check("a,b,|c,d", "df,", "a,b,|d");
        check("|abc", "dfz", "|abc");
    }

    #[test]
    fn whole_lines() {
        check("one\nt|wo\nthree", "dd", "one\n|three");
        check("one\ntwo\nthr|ee", "dd", "one\n|two");
        check("|one", "dd", "|");
        check("one\n|two\nthree\nfour", "2dd", "one\n|four");
        check("one\n|two\nthree", "dj", "|one");
        check("one\ntwo\n|three", "dk", "|one");
        check("|one\ntwo\nthree", "dG", "|");
        check("one\n  t|wo\nthree", "ccX<Esc>", "one\n  |X\nthree");
        check("one\nt|wo\nthree", "SX<Esc>", "one\n|X\nthree");
        check("one\nt|wo\nthree", "yyp", "one\ntwo\n|two\nthree");
        check("one\nt|wo\nthree", "yyP", "one\n|two\ntwo\nthree");
        check("one\nt|wo", "yyp", "one\ntwo\n|two");
        check("one\nt|wo", "Yp", "one\ntwo\n|two");
        check("|one\ntwo", "ddp", "two\n|one");
    }

    #[test]
    fn put_after_and_before() {
        check("|abc", "ylp", "a|abc");
        check("|abc", "ylP", "|aabc");
        check("one |two", "yiw$p", "one twotw|o");
        check("|ab", "xp", "b|a");
        check("|ab", "yl3p", "aaa|ab");
        let mut editor = Editor::new("|abc");
        editor.type_keys("p");
        assert_eq!(editor.marked(), "|abc", "nothing yanked yet");
    }

    #[test]
    fn text_objects() {
        check("one t|wo three", "diw", "one | three");
        check("one t|wo three", "daw", "one |three");
        check("one two thr|ee", "daw", "one tw|o");
        check("say \"hel|lo\" now", "di\"", "say \"|\" now");
        check("say \"hel|lo\" now", "da\"", "say | now");
        check("f(a, |b)", "di(", "f(|)");
        check("f(a, |b)", "da(", "|f");
        check("f(a, (|b))", "dib", "f(a, (|))");
        check("f(a, (b|))", "di)", "f(a, (|))");
        check("[x |y]", "ci]Z<Esc>", "[|Z]");
        check("{\n  |a\n}", "di{", "{|}");
        check("say |\"x\"", "ci\"y<Esc>", "say \"|y\"");
        check("no |quotes", "di\"", "no |quotes");
    }

    #[test]
    fn replace_join_and_case() {
        check("|abc", "rX", "|Xbc");
        check("|abc", "2rX", "X|Xc");
        check("|abc", "4rX", "|abc");
        check("o|ne\ntwo", "J", "one| two");
        check("o|ne\n   two", "J", "one| two");
        check("o|ne\n\ntwo", "J", "on|e\ntwo");
        check("|a\nb\nc", "3J", "a b| c");
        check("|abc", "~", "A|bc");
        check("|abc", "3~", "AB|C");
    }

    #[test]
    fn counts_multiply_and_zero_is_a_motion_alone() {
        check("|a b c d e f g", "2d2w", "|e f g");
        check("abc|def", "0", "|abcdef");
        check("|a b c d e f g h i j k", "10w", "a b c d e f g h i j |k");
    }

    #[test]
    fn dot_repeats_the_last_change() {
        check("|a b c d", "dw..", "|d");
        check("|abcd", "x2.", "|d");
        check("|one two three", "cwX<Esc>w.", "X |X three");
        check("|one\ntwo", "AX<Esc>j.", "oneX\ntwo|X");
        check("|a\nb\nc", "ddj.", "|b");
        check("|ab", "~.", "A|B");
        // A Visual-mode change is not the one `.` repeats.
        check("|abcdef", "xvld.", "|ef");
        let mut editor = Editor::new("|abc");
        editor.type_keys("lll.");
        assert_eq!(editor.marked(), "ab|c", "no change yet: nothing to repeat");
    }

    #[test]
    fn visual_mode_selects_and_operates() {
        let mut editor = Editor::new("one |two three");
        editor.type_keys("v");
        assert_eq!(editor.mode(), Mode::Visual);
        assert_eq!(editor.selected(), "one [t]wo three");
        editor.type_keys("e");
        assert_eq!(editor.selected(), "one [two] three");
        editor.type_keys("d");
        assert_eq!(editor.marked(), "one | three");
        assert_eq!(editor.mode(), Mode::Normal);

        check("one |two three", "vey$p", "one two threetw|o");
        check("one |two three", "vecX<Esc>", "one |X three");
        check("one |two three", "vbd", "|wo three");
        check("one |two three", "veU", "one |TWO three");
        check("one |TWO three", "veu", "one |two three");
        check("one |two three", "viwd", "one | three");
        check("one |two three", "verx", "one |xxx three");
        check("one |two three", "ve<Esc>", "one tw|o three");

        let mut editor = Editor::new("one\nt|wo\nthree");
        editor.type_keys("Vj");
        assert_eq!(editor.mode(), Mode::VisualLine);
        assert_eq!(editor.selected(), "one\n[two\nthree]");
        editor.type_keys("d");
        assert_eq!(editor.marked(), "|one");

        check("one\nt|wo\nthree", "Vy", "one\n|two\nthree");
        check("one\nt|wo\nthree", "VyjVp", "one\ntwo\n|two");
        check("|one\ntwo", "VJ", "one| two");
        let mut editor = Editor::new("ab|cd");
        editor.type_keys("vlo");
        assert_eq!(editor.selected(), "ab[cd]");
        editor.type_keys("h");
        assert_eq!(editor.selected(), "a[bcd]");
        editor.type_keys("v");
        assert_eq!(editor.mode(), Mode::Normal);
    }

    #[test]
    fn a_mouse_selection_becomes_visual_mode() {
        let mut editor = Editor::new("|hello world");
        editor.selection = 0..5;
        editor.cursor = 5;
        editor.type_keys("d");
        assert_eq!(editor.marked(), "| world");

        // Dragged backwards: the cursor is at the start.
        let mut editor = Editor::new("|hello world");
        editor.selection = 6..11;
        editor.cursor = 6;
        editor.type_keys("y");
        assert_eq!(editor.marked(), "hello |world");
        editor.type_keys("P");
        assert_eq!(editor.marked(), "hello worl|dworld");

        // A click while in Visual mode ends it there.
        let mut editor = Editor::new("|hello world");
        editor.type_keys("vl");
        editor.selection = 8..8;
        editor.cursor = 8;
        editor.type_keys("x");
        assert_eq!(editor.mode(), Mode::Normal);
        assert_eq!(editor.marked(), "hello wo|ld");
    }

    #[test]
    fn escape_cancels_and_otherwise_passes() {
        let mut editor = Editor::new("|abc");
        assert_eq!(
            editor.vim.key(Key::Escape, "abc", 0, 0..0),
            Outcome::Pass,
            "Escape in Normal mode is the window's"
        );
        editor.type_keys("d<Esc>x");
        assert_eq!(editor.marked(), "|bc", "d was cancelled; x deleted");
        editor.type_keys("2<Esc>x");
        assert_eq!(editor.marked(), "|c");
        editor.type_keys("f<Esc>x");
        assert_eq!(editor.marked(), "|");
    }

    #[test]
    fn the_actions_are_asked_for() {
        let mut editor = Editor::new("|abc");
        editor.type_keys("u<C-r><C-d><C-u>/");
        assert_eq!(
            editor.actions,
            [Op::Undo, Op::Redo, Op::PageDown, Op::PageUp, Op::Find]
        );
        assert_eq!(editor.marked(), "|abc");
    }

    #[test]
    fn pending_keys_show_while_typed() {
        let mut vim = Vim::default();
        assert_eq!(vim.pending_keys(), "");
        vim.key(Key::Char('2'), "abc", 0, 0..0);
        vim.key(Key::Char('d'), "abc", 0, 0..0);
        assert_eq!(vim.pending_keys(), "2d");
        vim.key(Key::Char('l'), "abc", 0, 0..0);
        assert_eq!(vim.pending_keys(), "");
    }

    #[test]
    fn insert_mode_passes_every_key_but_escape() {
        let mut vim = Vim::default();
        vim.key(Key::Char('i'), "abc", 0, 0..0);
        for key in [
            Key::Char('x'),
            Key::Enter,
            Key::Backspace,
            Key::Tab,
            Key::Ctrl('r'),
        ] {
            assert!(vim.passes(&key));
            assert_eq!(vim.key(key, "abc", 0, 0..0), Outcome::Pass);
        }
        assert!(!vim.passes(&Key::Ctrl('[')));
        assert_eq!(
            vim.key(Key::Ctrl('['), "abc", 1, 1..1),
            Outcome::Handled(vec![Op::Cursor(0)])
        );
        assert_eq!(vim.mode(), Mode::Normal);
    }

    #[test]
    fn retarget_and_reset() {
        let mut vim = Vim::default();
        vim.key(Key::Char('v'), "abc", 0, 0..0);
        vim.retarget();
        assert_eq!(vim.mode(), Mode::Normal);
        vim.key(Key::Char('i'), "abc", 0, 0..0);
        vim.retarget();
        assert_eq!(vim.mode(), Mode::Insert, "Insert mode carries over");
        vim.reset();
        assert_eq!(vim.mode(), Mode::Normal);
    }

    #[test]
    fn keystrokes_map_to_keys() {
        let key =
            |spec: &str| Key::from_keystroke(&Keystroke::parse(spec).unwrap().with_simulated_ime());
        assert_eq!(key("j"), Some(Key::Char('j')));
        assert_eq!(key("shift-g"), Some(Key::Char('G')));
        assert_eq!(key("$"), Some(Key::Char('$')));
        assert_eq!(key("escape"), Some(Key::Escape));
        assert_eq!(key("space"), Some(Key::Char(' ')));
        assert_eq!(key("ctrl-r"), Some(Key::Ctrl('r')));
        assert_eq!(key("ctrl-["), Some(Key::Ctrl('[')));
        assert_eq!(key("ctrl-s"), None, "the window's shortcuts pass");
        assert_eq!(key("cmd-s"), None);
        assert_eq!(key("left"), Some(Key::Left));
        assert_eq!(key("end"), Some(Key::End));
        assert_eq!(key("shift-left"), None, "Shift+arrow selects in the editor");
        assert_eq!(key("alt-right"), None);
        assert_eq!(key("ctrl-home"), None, "the document's start, the editor's");
        assert_eq!(key("pagedown"), None);
        assert_eq!(key("f5"), None);
    }

    #[test]
    fn crlf_line_breaks_stay_whole() {
        check("o|ne\r\ntwo", "$", "on|e\r\ntwo");
        check("o|ne\r\ntwo", "AX<Esc>", "one|X\r\ntwo");
        check("o|ne\r\ntwo", "D", "|o\r\ntwo");
        check("o|ne\r\ntwo", "dd", "|two");
        check("one\r\nt|wo", "dd", "|one");
        check("o|ne\r\ntwo", "yyp", "one\r\n|one\r\ntwo");
        check("one\r\nt|wo", "yyp", "one\r\ntwo\r\n|two");
        check("o|ne\r\ntwo", "oX<Esc>", "one\r\n|X\r\ntwo");
        check("one\r\nt|wo", "OX<Esc>", "one\r\n|X\r\ntwo");
        check("o|ne\r\n  two", "J", "one| two");
        check("|one\r\n\r\ntwo", "w", "one\r\n|\r\ntwo");
        check("one\r\n\r\n|two", "b", "one\r\n|\r\ntwo");
        check("|a\r\nb\r\n\r\nc", "}", "a\r\nb\r\n|\r\nc");
        check("a\r\n\r\nb\r\n|c", "{", "a\r\n|\r\nb\r\nc");
        check("|ab\r\ncd", "   ", "ab\r\nc|d");
        check("ab\r\n|cd", "<BS>", "a|b\r\ncd");
        check("o|ne\r\ntwo", "jdk", "|");
        check("o|ne\r\ntwo", "vjd", "o|o");
        let mut editor = Editor::new("o|ne\r\ntwo");
        editor.type_keys("V");
        assert_eq!(editor.selected(), "[one\r\n]two");
        editor.type_keys("y");
        editor.type_keys("jp");
        assert_eq!(editor.marked(), "one\r\ntwo\r\n|one");
    }

    /// Random keys over texts of every kind (a fixed seed: the same run each
    /// time): no panic, the cursor on a boundary, line breaks whole.
    #[test]
    fn random_keys_keep_the_invariants() {
        const ALPHABET: &str = "hjklwbeWBE0^$GgfFtT;,%{}dcyxXsSDCYpPrJ~uvVoOiaIA.iw\"()1234 ";
        let alphabet: Vec<char> = ALPHABET.chars().collect();
        let texts = [
            "one two three\nfour (five) \"six\"\n\nseven",
            "中文，测试 abc\n  缩进行\n\n",
            "a\r\nb c\r\n\r\n(d [e] {f})\r\n",
            "",
            "x",
        ];
        let mut seed: u64 = 0x5eed_1234_abcd_0001;
        let mut random = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut sequences = 0;
        for text in texts {
            for _ in 0..400 {
                let mut editor = Editor {
                    vim: Vim::default(),
                    text: text.to_string(),
                    cursor: 0,
                    selection: 0..0,
                    actions: Vec::new(),
                };
                let mut spec = String::new();
                for _ in 0..16 {
                    let pick = random() as usize;
                    match pick % 20 {
                        0 => spec.push_str("<Esc>"),
                        1 => spec.push_str("<CR>"),
                        2 => spec.push_str("<BS>"),
                        _ => spec.push(alphabet[pick / 20 % alphabet.len()]),
                    }
                }
                editor.type_keys(&spec);
                sequences += 1;
                assert!(
                    editor.text.is_char_boundary(editor.cursor),
                    "{text:?} {spec:?}"
                );
                assert!(
                    editor.selection.end <= editor.text.len(),
                    "{text:?} {spec:?}"
                );
                let crlf = editor.text.matches("\r\n").count();
                assert_eq!(editor.text.matches('\r').count(), crlf, "{text:?} {spec:?}");
                if text.contains('\r') {
                    // Typed Enters (the test editor's) are LF; count those out.
                    let typed = editor.text.matches('\n').count() - crlf;
                    assert!(
                        !editor.text.contains("\r\r") && typed <= spec.matches("<CR>").count(),
                        "{text:?} {spec:?}"
                    );
                }
            }
        }
        assert_eq!(sequences, 2000);
    }

    /// Normal mode reads the whole text at each key; on a 2 MB document a
    /// key still takes well under a frame.
    #[test]
    fn a_large_document_stays_quick() {
        let line = "Some words on a line, and (a bracket) for good measure.\n";
        let text = line.repeat(2 * 1024 * 1024 / line.len());
        let middle = text.len() / 2 - text.len() / 2 % line.len();
        let mut worst = std::time::Duration::ZERO;
        for spec in [
            "j", "k", "w", "b", "dd", "yyp", "}", "{", "G", "gg", "%", "x", "o",
        ] {
            let mut vim = Vim::default();
            let started = std::time::Instant::now();
            for key in keys(spec) {
                let _ = vim.key(key, &text, middle, middle..middle);
            }
            worst = worst.max(started.elapsed());
        }
        eprintln!("worst key on 2 MB: {worst:?}");
        assert!(worst < std::time::Duration::from_millis(100), "{worst:?}");
    }

    #[test]
    fn degenerate_texts_do_not_panic() {
        for text in [
            "", "\n", "\n\n", "a", "中", "a\n", "\na", "\r\n", "a\r\nb", "\r\n\r\n", "a\r\n",
        ] {
            for spec in [
                "x", "dd", "dw", "de", "db", "D", "J", "p", "P", "yyp", "w", "b", "e", "G", "gg",
                "}", "{", "%", "$", "0", "j", "k", "~", "rX", "vd", "Vd", "viwd", "da(", "ci\"",
                "o<Esc>", "O<Esc>", "A<Esc>", ".", "3.", "<CR>", "<BS>", " ",
            ] {
                for start in 0..=text.len() {
                    // The editor's cursor is never inside a character or
                    // between the two of a CRLF.
                    if !text.is_char_boundary(start) || clip(text, start) != start {
                        continue;
                    }
                    let mut editor = Editor {
                        vim: Vim::default(),
                        text: text.to_string(),
                        cursor: start,
                        selection: start..start,
                        actions: Vec::new(),
                    };
                    editor.type_keys(spec);
                    assert!(
                        editor.text.is_char_boundary(editor.cursor),
                        "{text:?} {spec:?}"
                    );
                    // Line breaks stay whole: a CRLF text keeps only CRLFs,
                    // an LF text gets no CR.
                    let crlf = editor.text.matches("\r\n").count();
                    assert_eq!(editor.text.matches('\r').count(), crlf, "{text:?} {spec:?}");
                    if text.contains('\r') {
                        assert_eq!(editor.text.matches('\n').count(), crlf, "{text:?} {spec:?}");
                    }
                    assert!(
                        !(editor.cursor > 0
                            && editor.text[..editor.cursor].ends_with('\r')
                            && editor.text[editor.cursor..].starts_with('\n')),
                        "the cursor never splits a CRLF: {text:?} {spec:?}"
                    );
                }
            }
        }
    }
}
