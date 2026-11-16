//! Vim's search patterns and `:s` replacements, on top of the `regex`
//! crate: a pattern in Vim's syntax is translated to the crate's, and a
//! replacement string is expanded against a match's captures.
//!
//! Patterns run with `(?mR)`: `^` and `$` are line anchors, and a line ends
//! before `\n` or `\r\n`, so CRLF documents behave as LF ones. `.` matches
//! neither `\r` nor `\n`.

use regex::{Captures, Regex, RegexBuilder};

/// How letter case is matched: Vim's 'ignorecase' and 'smartcase'.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaseRule {
    pub ignorecase: bool,
    /// With `ignorecase`: a pattern with an upper-case letter matches case.
    pub smartcase: bool,
}

/// A search pattern, compiled.
#[derive(Clone, Debug)]
pub struct Pattern {
    source: String,
    ignore_case: bool,
    regex: Regex,
}

/// Two patterns are the same when they match the same: the same source,
/// case for case or not.
impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.ignore_case == other.ignore_case
    }
}

impl Eq for Pattern {}

impl Pattern {
    /// Compile `source`, in Vim's syntax. `previous` is the last `:s`
    /// replacement, which `~` matches. The error is Vim's message.
    pub fn new(source: &str, case: CaseRule, previous: Option<&str>) -> Result<Self, String> {
        let translated = translate(source, previous)?;
        let ignore_case = match translated.case {
            Some(ignore) => ignore,
            None => case.ignorecase && !(case.smartcase && has_upper(source)),
        };
        let regex = RegexBuilder::new(&translated.regex)
            .multi_line(true)
            .crlf(true)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|_| format!("E383: Invalid search string: {source}"))?;
        Ok(Self {
            source: source.to_string(),
            ignore_case,
            regex,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn regex(&self) -> &Regex {
        &self.regex
    }
}

/// `text` as a pattern that matches it literally (for `*` and `#`).
pub fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '.' | '*' | '[' | '~' | '^' | '$') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Whether `source` has an upper-case letter, a character after `\` aside
/// (`\S` is a class, not an `S`): what 'smartcase' looks at.
fn has_upper(source: &str) -> bool {
    let mut chars = source.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
        } else if c.is_uppercase() {
            return true;
        }
    }
    false
}

/// How much of the syntax is special without a backslash: `\v`, `\m`,
/// `\M` and `\V` (the last two alike here: only `^` and `$` are).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Syntax {
    VeryMagic,
    Magic,
    NoMagic,
}

/// The characters that mean something in some mode.
const META: &str = "()|+?={<>%.*[~@";

fn special(mode: Syntax, c: char) -> bool {
    match mode {
        Syntax::VeryMagic => META.contains(c),
        Syntax::Magic => matches!(c, '.' | '*' | '[' | '~'),
        Syntax::NoMagic => false,
    }
}

struct Translated {
    regex: String,
    /// `\c` (`Some(true)`) or `\C` (`Some(false)`) in the pattern.
    case: Option<bool>,
}

fn unsupported(what: &str) -> String {
    format!("E: {what} is not supported in a pattern")
}

/// Translate a Vim pattern to the `regex` crate's syntax.
fn translate(source: &str, previous: Option<&str>) -> Result<Translated, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len() + 8);
    let mut mode = Syntax::Magic;
    let mut case = None;
    // At the start of a branch: `^` anchors, and a quantifier has nothing
    // to repeat (so it is taken literally, as Vim does with `*`).
    let mut branch_start = true;
    // The last thing written can take a quantifier.
    let mut can_repeat = false;
    let mut depth = 0usize;
    let mut ix = 0;

    // Whether the pattern goes on with a branch end at `at`: the end, or
    // `\|` / `\)` (`|` / `)` very magic), so a `$` there anchors.
    let ends_branch = |at: usize, mode: Syntax| -> bool {
        match chars.get(at) {
            None => true,
            Some('\\') => match chars.get(at + 1) {
                Some('|' | ')') => mode != Syntax::VeryMagic,
                _ => false,
            },
            Some('|' | ')') => mode == Syntax::VeryMagic,
            _ => false,
        }
    };

    while ix < chars.len() {
        let c = chars[ix];
        let escaped = c == '\\';
        let (c, next) = if escaped {
            match chars.get(ix + 1) {
                Some(&c) => (c, ix + 2),
                // A trailing backslash: a backslash.
                None => {
                    out.push_str("\\\\");
                    break;
                }
            }
        } else {
            (c, ix + 1)
        };
        ix = next;

        // Anchors: special unescaped where Vim takes them so; escaped, the
        // character.
        if !escaped && c == '^' && branch_start {
            out.push('^');
            can_repeat = false;
            continue;
        }
        if !escaped && c == '$' && ends_branch(ix, mode) {
            out.push('$');
            can_repeat = false;
            branch_start = false;
            continue;
        }

        let meta = META.contains(c) && escaped != special(mode, c);
        if escaped && !META.contains(c) {
            // Classes, modes, case, the rest taken literally.
            let piece = match c {
                'v' => {
                    mode = Syntax::VeryMagic;
                    continue;
                }
                'm' => {
                    mode = Syntax::Magic;
                    continue;
                }
                'M' | 'V' => {
                    mode = Syntax::NoMagic;
                    continue;
                }
                'c' => {
                    case = Some(true);
                    continue;
                }
                'C' => {
                    case = Some(false);
                    continue;
                }
                'd' | 'D' | 's' | 'S' | 'w' | 'W' => format!("\\{c}"),
                'a' => "[A-Za-z]".into(),
                'A' => "[^A-Za-z]".into(),
                'l' => "[a-z]".into(),
                'L' => "[^a-z]".into(),
                'u' => "[A-Z]".into(),
                'U' => "[^A-Z]".into(),
                'x' => "[0-9A-Fa-f]".into(),
                'X' => "[^0-9A-Fa-f]".into(),
                'o' => "[0-7]".into(),
                'O' => "[^0-7]".into(),
                'h' => "[A-Za-z_]".into(),
                'H' => "[^A-Za-z_]".into(),
                'n' => "(?:\r?\n)".into(),
                't' => "\\t".into(),
                'r' => "\\r".into(),
                'e' => "\\x1b".into(),
                '1'..='9' => return Err(unsupported("a back reference")),
                'z' | '_' | '#' | '&' | 'i' | 'I' | 'k' | 'K' | 'f' | 'F' | 'p' | 'P' => {
                    return Err(unsupported(&format!("\\{c}")));
                }
                _ => regex::escape(&c.to_string()),
            };
            out.push_str(&piece);
            can_repeat = true;
            branch_start = false;
            continue;
        }
        if !meta {
            out.push_str(&regex::escape(&c.to_string()));
            can_repeat = true;
            branch_start = false;
            continue;
        }

        match c {
            '*' | '+' | '=' | '?' | '{' => {
                let quantifier = if c == '{' {
                    let (quantifier, after) = brace(&chars, ix)?;
                    ix = after;
                    quantifier
                } else if c == '*' {
                    "*".to_string()
                } else if c == '+' {
                    "+".to_string()
                } else {
                    "?".to_string()
                };
                if !can_repeat {
                    if branch_start || out.ends_with('^') {
                        // Nothing before it: the character itself.
                        out.push_str(&regex::escape(&c.to_string()));
                        can_repeat = true;
                        branch_start = false;
                        continue;
                    }
                    return Err(format!("E61: Nested {c}"));
                }
                out.push_str(&quantifier);
                can_repeat = false;
            }
            '(' => {
                out.push('(');
                depth += 1;
                branch_start = true;
                can_repeat = false;
            }
            ')' => {
                if depth == 0 {
                    return Err("E54: Unmatched \\)".into());
                }
                depth -= 1;
                out.push(')');
                can_repeat = true;
                branch_start = false;
            }
            '|' => {
                out.push('|');
                branch_start = true;
                can_repeat = false;
            }
            '%' => {
                if chars.get(ix) != Some(&'(') {
                    return Err(unsupported("\\%"));
                }
                ix += 1;
                out.push_str("(?:");
                depth += 1;
                branch_start = true;
                can_repeat = false;
            }
            '<' => {
                out.push_str("\\b{start}");
                can_repeat = false;
                branch_start = false;
            }
            '>' => {
                out.push_str("\\b{end}");
                can_repeat = false;
                branch_start = false;
            }
            '.' => {
                out.push('.');
                can_repeat = true;
                branch_start = false;
            }
            '[' => match bracket(&chars, ix) {
                Some((class, after)) => {
                    out.push_str(&class);
                    ix = after;
                    can_repeat = true;
                    branch_start = false;
                }
                None => {
                    // No closing `]`: a `[`.
                    out.push_str("\\[");
                    can_repeat = true;
                    branch_start = false;
                }
            },
            '~' => {
                let Some(previous) = previous else {
                    return Err("E33: No previous substitute regular expression".into());
                };
                out.push_str(&format!("(?:{})", regex::escape(previous)));
                can_repeat = true;
                branch_start = false;
            }
            '@' => return Err(unsupported("\\@")),
            _ => {
                out.push_str(&regex::escape(&c.to_string()));
                can_repeat = true;
                branch_start = false;
            }
        }
    }
    if depth > 0 {
        return Err("E54: Unmatched \\(".into());
    }
    Ok(Translated { regex: out, case })
}

/// `\{n,m}` after its `{` at `at`: the quantifier, and where the pattern
/// goes on. `\{-…}` is the lazy form.
fn brace(chars: &[char], at: usize) -> Result<(String, usize), String> {
    let mut ix = at;
    let lazy = chars.get(ix) == Some(&'-');
    if lazy {
        ix += 1;
    }
    let mut body = String::new();
    while let Some(&c) = chars.get(ix) {
        if c.is_ascii_digit() || c == ',' {
            body.push(c);
            ix += 1;
        } else {
            break;
        }
    }
    // The closing `}`, a backslash before it allowed (and usual unless
    // very magic).
    if chars.get(ix) == Some(&'\\') {
        ix += 1;
    }
    if chars.get(ix) != Some(&'}') {
        return Err("E554: Syntax error in \\{...}".into());
    }
    ix += 1;
    let quantifier = match body.split_once(',') {
        None if body.is_empty() => "*".to_string(),
        None => format!("{{{body}}}"),
        Some((low, high)) => {
            if body.matches(',').count() > 1 {
                return Err("E554: Syntax error in \\{...}".into());
            }
            let low = if low.is_empty() { "0" } else { low };
            format!("{{{low},{high}}}")
        }
    };
    Ok((
        if lazy {
            format!("{quantifier}?")
        } else {
            quantifier
        },
        ix,
    ))
}

/// A `[…]` class after its `[` at `at`, in the crate's syntax, and where the
/// pattern goes on; `None` without a closing `]`.
fn bracket(chars: &[char], at: usize) -> Option<(String, usize)> {
    let mut ix = at;
    let mut class = String::from("[");
    if chars.get(ix) == Some(&'^') {
        class.push('^');
        ix += 1;
    }
    // A `]` first is a member.
    if chars.get(ix) == Some(&']') {
        class.push_str("\\]");
        ix += 1;
    }
    let mut previous = None;
    while let Some(&c) = chars.get(ix) {
        ix += 1;
        match c {
            ']' => {
                class.push(']');
                return Some((class, ix));
            }
            '[' if chars.get(ix) == Some(&':') => {
                // `[:alpha:]`: the same in both syntaxes.
                let close = (ix..chars.len().saturating_sub(1))
                    .find(|&k| chars[k] == ':' && chars[k + 1] == ']')?;
                class.extend(&chars[ix - 1..close + 2]);
                ix = close + 2;
            }
            '\\' => {
                let escaped = *chars.get(ix)?;
                ix += 1;
                let known = match escaped {
                    'n' => "\\n",
                    't' => "\\t",
                    'r' => "\\r",
                    'e' => "\\x1b",
                    'd' => "\\d",
                    's' => "\\s",
                    'w' => "\\w",
                    '\\' => "\\\\",
                    ']' => "\\]",
                    '^' => "\\^",
                    '-' => "\\-",
                    _ => "",
                };
                if known.is_empty() {
                    class.push('\\');
                    class.push(escaped);
                } else {
                    class.push_str(known);
                }
            }
            // What the crate reads as set operations or nesting.
            '[' | '&' | '~' => {
                class.push('\\');
                class.push(c);
            }
            '-' if previous == Some('-') => class.push_str("\\-"),
            _ => class.push(c),
        }
        previous = Some(c);
    }
    None
}

/// Expand a `:s` replacement for one match: `&` and `\0` the match, `\1` …
/// `\9` its groups, `\r` and `\n` a line break (`newline`), `\t` a tab,
/// `\u` `\l` the next character's case, `\U` `\L` … `\E` (`\e`) a run's,
/// `\&` and `\\` the characters.
pub fn expand(template: &str, captures: &Captures<'_>, newline: &str) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum Case {
        None,
        Upper,
        Lower,
    }
    struct Out {
        text: String,
        one: Case,
        run: Case,
    }
    impl Out {
        fn push(&mut self, piece: &str) {
            for c in piece.chars() {
                let case = if self.one != Case::None {
                    std::mem::replace(&mut self.one, Case::None)
                } else {
                    self.run
                };
                match case {
                    Case::Upper => self.text.extend(c.to_uppercase()),
                    Case::Lower => self.text.extend(c.to_lowercase()),
                    Case::None => self.text.push(c),
                }
            }
        }
    }
    let group = |n: usize| captures.get(n).map_or("", |m| m.as_str());
    let mut out = Out {
        text: String::new(),
        one: Case::None,
        run: Case::None,
    };
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match c {
            '&' => out.push(group(0)),
            '\\' => match chars.next() {
                Some(d @ '0'..='9') => out.push(group(d as usize - '0' as usize)),
                Some('r' | 'n') => out.text.push_str(newline),
                Some('t') => out.push("\t"),
                Some('u') => out.one = Case::Upper,
                Some('l') => out.one = Case::Lower,
                Some('U') => out.run = Case::Upper,
                Some('L') => out.run = Case::Lower,
                Some('E' | 'e') => out.run = Case::None,
                Some(other) => out.push(&other.to_string()),
                None => out.push("\\"),
            },
            _ => out.push(&c.to_string()),
        }
    }
    out.text
}

/// A replacement with its `~` (not `\~`) taken as `previous`, the last
/// replacement used, as Vim does when the replacement is read.
pub fn with_previous(template: &str, previous: Option<&str>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                out.push('\\');
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '~' => out.push_str(previous.unwrap_or("")),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regex(source: &str) -> String {
        translate(source, None)
            .map(|t| t.regex)
            .unwrap_or_else(|e| e)
    }

    fn found(source: &str, text: &str) -> Vec<String> {
        let pattern = Pattern::new(source, CaseRule::default(), None).expect(source);
        pattern
            .regex()
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    #[test]
    fn magic_syntax_translates() {
        assert_eq!(regex("a.b"), "a.b");
        assert_eq!(regex("a\\.b"), "a\\.b");
        assert_eq!(regex("a*"), "a*");
        assert_eq!(regex("*a"), "\\*a", "nothing to repeat: the character");
        assert_eq!(regex("^*"), "^\\*");
        assert_eq!(regex("a\\+b\\?c\\="), "a+b?c?");
        assert_eq!(regex("a+b?(c)|d{1}"), "a\\+b\\?\\(c\\)\\|d\\{1\\}");
        assert_eq!(regex("\\(a\\|b\\)"), "(a|b)");
        assert_eq!(regex("\\%(a\\)"), "(?:a)");
        assert_eq!(regex("\\<a\\>"), "\\b{start}a\\b{end}");
        assert_eq!(regex("a\\{2,3}"), "a{2,3}");
        assert_eq!(regex("a\\{-}"), "a*?");
        assert_eq!(regex("a\\{-1,}"), "a{1,}?");
        assert_eq!(regex("a\\{,2}"), "a{0,2}");
        assert_eq!(regex("a\\{3}"), "a{3}");
        assert_eq!(regex("a^b$c"), "a\\^b\\$c", "anchors only at the ends");
        assert_eq!(regex("^a$"), "^a$");
        assert_eq!(regex("\\(^a$\\)"), "(^a$)");
        assert_eq!(regex("a\\nb"), "a(?:\r?\n)b");
        assert_eq!(regex("\\d\\s\\w\\a\\u"), "\\d\\s\\w[A-Za-z][A-Z]");
        assert_eq!(regex("[a-z]x"), "[a-z]x");
        assert_eq!(regex("[^]a]"), "[^\\]a]");
        assert_eq!(regex("[[:alpha:]_]"), "[[:alpha:]_]");
        assert_eq!(regex("[a&&b]"), "[a\\&\\&b]");
        assert_eq!(regex("[a"), "\\[a", "an unclosed class is a `[`");
        assert_eq!(regex("a\\"), "a\\\\");
        assert_eq!(regex("中文"), "中文");
    }

    #[test]
    fn very_magic_and_no_magic() {
        assert_eq!(
            regex("\\v(a|b)+c?d{2}<e>"),
            "(a|b)+c?d{2}\\b{start}e\\b{end}"
        );
        assert_eq!(regex("\\v\\(a\\)"), "\\(a\\)");
        assert_eq!(regex("\\va%(b)"), "a(?:b)");
        assert_eq!(regex("\\Va.b*"), "a\\.b\\*");
        assert_eq!(regex("\\Va\\.b"), "a.b");
        assert_eq!(regex("\\Ma*\\*"), "a\\**");
        assert_eq!(regex("\\vab\\ma+"), "aba\\+");
    }

    #[test]
    fn errors_are_vims() {
        assert_eq!(regex("\\(a"), "E54: Unmatched \\(");
        assert_eq!(regex("a\\)"), "E54: Unmatched \\)");
        assert_eq!(regex("a**"), "E61: Nested *");
        assert_eq!(regex("a\\{x}"), "E554: Syntax error in \\{...}");
        assert_eq!(
            regex("\\(a\\)\\1"),
            "E: a back reference is not supported in a pattern"
        );
        assert_eq!(regex("a\\zsb"), "E: \\z is not supported in a pattern");
        assert_eq!(
            regex("a~"),
            "E33: No previous substitute regular expression"
        );
        assert_eq!(
            translate("a~", Some("x.y"))
                .map(|t| t.regex)
                .ok()
                .as_deref(),
            Some("a(?:x\\.y)")
        );
    }

    #[test]
    fn patterns_match_as_in_vim() {
        assert_eq!(found("a.c", "abc a.c ac"), ["abc", "a.c"]);
        assert_eq!(found("\\<is\\>", "this is his"), ["is"]);
        assert_eq!(found("^\\s*-", "- a\n  - b\nc - d"), ["-", "  -"]);
        assert_eq!(found("o$", "foo\r\nbar o\nzo"), ["o", "o", "o"]);
        assert_eq!(found("a\\nb", "a\r\nb a\nb"), ["a\r\nb", "a\nb"]);
        assert_eq!(found("x\\{2}", "x xx xxx"), ["xx", "xx"]);
        assert_eq!(found(".", "\r\n"), Vec::<String>::new(), "no line break");
        assert_eq!(found("中.", "中文 中x"), ["中文", "中x"]);
    }

    #[test]
    fn case_follows_the_rules() {
        let matches = |source: &str, case: CaseRule| {
            Pattern::new(source, case, None)
                .unwrap()
                .regex()
                .is_match("Foo")
        };
        let ic = CaseRule {
            ignorecase: true,
            smartcase: false,
        };
        let scs = CaseRule {
            ignorecase: true,
            smartcase: true,
        };
        assert!(!matches("foo", CaseRule::default()));
        assert!(matches("foo", ic));
        assert!(matches("foo", scs));
        assert!(!matches("fOo", scs), "an upper-case letter: case counts");
        assert!(
            matches("\\Sfoo\\|foo", scs),
            "`\\S` is no upper-case letter"
        );
        assert!(matches("foo\\c", CaseRule::default()));
        assert!(!matches("foo\\C", ic));
        let a = Pattern::new("x", ic, None).unwrap();
        assert_ne!(a, Pattern::new("x", CaseRule::default(), None).unwrap());
        assert_eq!(a, Pattern::new("x", ic, None).unwrap());
    }

    #[test]
    fn escape_round_trips() {
        for text in ["a.b*c", "[x]~^$", "a/b?c\\d", "中文"] {
            assert_eq!(found(&escape(text), text), [text], "{text:?}");
        }
    }

    #[test]
    fn replacements_expand() {
        let regex = Regex::new("(\\w+) (\\w+)").unwrap();
        let caps = regex.captures("hello world").unwrap();
        let expand = |template: &str| expand(template, &caps, "\n");
        assert_eq!(expand("\\2 \\1"), "world hello");
        assert_eq!(expand("[&]"), "[hello world]");
        assert_eq!(expand("\\&\\\\"), "&\\");
        assert_eq!(expand("\\u\\1"), "Hello");
        assert_eq!(expand("\\U\\1\\E \\2"), "HELLO world");
        assert_eq!(expand("\\L\\uABC"), "Abc");
        assert_eq!(expand("a\\rb\\tc"), "a\nb\tc");
        assert_eq!(expand("\\9"), "", "a group that is not there");
        assert_eq!(with_previous("<~>\\~", Some("x")), "<x>\\~");
        assert_eq!(with_previous("~", None), "");
    }
}
