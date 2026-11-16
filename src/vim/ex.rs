//! Ex command lines: line ranges, `:s`'s arguments, and `:set`'s options.
//! Lines are numbered from 1, as Vim numbers them.

/// What a range is read against.
pub struct Lines {
    /// The cursor's line.
    pub current: usize,
    /// The last line (the count of lines).
    pub last: usize,
    /// `'<` and `'>`: the last Visual selection's first and last lines.
    pub marks: Option<(usize, usize)>,
}

/// A range's first and last lines.
pub type LineRange = (usize, usize);

/// Read the range at the start of `command`: `%`, `.`, `$`, `12`, `'<`,
/// `'>`, with `+N` / `-N` offsets, one address or two (`,` or `;`). Returns
/// the first and last lines (`None` without a range) and the rest.
pub fn parse_range<'a>(
    command: &'a str,
    lines: &Lines,
) -> Result<(Option<LineRange>, &'a str), String> {
    let command = command.trim_start_matches([' ', ':']);
    if let Some(rest) = command.strip_prefix('%') {
        return Ok((Some((1, lines.last)), rest));
    }
    let (first, mut rest) = address(command, lines, lines.current)?;
    let Some(separator) = rest.chars().next().filter(|c| matches!(c, ',' | ';')) else {
        let range = first.map(|line| check(line, lines).map(|line| (line, line)));
        return Ok((range.transpose()?, rest));
    };
    rest = &rest[1..];
    let first = first.unwrap_or(lines.current);
    let current = if separator == ';' {
        first
    } else {
        lines.current
    };
    let (second, after) = address(rest, lines, current)?;
    let (a, b) = (
        check(first, lines)?,
        check(second.unwrap_or(current), lines)?,
    );
    // A backwards range: Vim asks to swap it; here it is swapped.
    Ok((Some((a.min(b), a.max(b))), after))
}

fn check(line: usize, lines: &Lines) -> Result<usize, String> {
    if line > lines.last {
        Err("E16: Invalid range".into())
    } else {
        Ok(line)
    }
}

/// One address and its offsets; `None` when there is neither.
fn address<'a>(
    text: &'a str,
    lines: &Lines,
    current: usize,
) -> Result<(Option<usize>, &'a str), String> {
    let mut rest = text;
    let digits = |s: &str| s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let mut base = if rest.starts_with(|c: char| c.is_ascii_digit()) {
        let n = digits(rest);
        let value = rest[..n].parse::<usize>().unwrap_or(usize::MAX);
        rest = &rest[n..];
        Some(value)
    } else if let Some(after) = rest.strip_prefix('.') {
        rest = after;
        Some(current)
    } else if let Some(after) = rest.strip_prefix('$') {
        rest = after;
        Some(lines.last)
    } else if let Some(after) = rest.strip_prefix('\'') {
        let (mark, after) = match after.chars().next() {
            Some(m @ ('<' | '>')) => (m, &after[1..]),
            _ => return Err("E: only the marks '< and '> are supported".into()),
        };
        let Some((low, high)) = lines.marks else {
            return Err("E20: Mark not set".into());
        };
        rest = after;
        Some(if mark == '<' { low } else { high })
    } else {
        None
    };
    while let Some(sign) = rest.chars().next().filter(|c| matches!(c, '+' | '-')) {
        rest = &rest[1..];
        let n = digits(rest);
        let step = if n == 0 {
            1
        } else {
            rest[..n].parse::<usize>().unwrap_or(usize::MAX)
        };
        rest = &rest[n..];
        let from = base.unwrap_or(current);
        base = Some(if sign == '+' {
            from.saturating_add(step)
        } else {
            from.checked_sub(step)
                .ok_or_else(|| "E16: Invalid range".to_string())?
        });
    }
    Ok((base, rest))
}

/// `:s`'s flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    /// `g`: every match in a line, not the first only.
    pub global: bool,
    /// `i` (`Some(true)`), `I` (`Some(false)`): case, whatever the options.
    pub ignore_case: Option<bool>,
    /// `n`: count the matches, change nothing.
    pub count_only: bool,
    /// `e`: no error when nothing matches.
    pub quiet: bool,
}

/// A `:s` command line, after the command's name.
#[derive(Debug, PartialEq, Eq)]
pub enum Substitute {
    /// `:s/pattern/replacement/flags`; an empty pattern is the last search.
    New {
        pattern: String,
        replacement: String,
        flags: Flags,
    },
    /// `:s [flags]`, `:&`, `:&&`: the last substitution again, with its
    /// flags when `keep`.
    Repeat { flags: Flags, keep: bool },
}

/// Read `:s`'s arguments: a delimiter (any character but a letter, a digit,
/// `\`, `"` and `|`), the pattern, the replacement, the flags.
pub fn parse_substitute(args: &str) -> Result<Substitute, String> {
    let Some(delimiter) = args
        .chars()
        .next()
        .filter(|c| !c.is_whitespace() && *c != '&' && !"gciIne".contains(*c))
    else {
        let (flags, keep) = parse_flags(args.trim_start())?;
        return Ok(Substitute::Repeat { flags, keep });
    };
    if delimiter.is_alphanumeric() || matches!(delimiter, '\\' | '"' | '|') {
        return Err("E146: Regular expressions can't be delimited by letters".into());
    }
    let rest = &args[delimiter.len_utf8()..];
    let (pattern, rest) = split_at_delimiter(rest, delimiter);
    let Some(rest) = rest else {
        return Ok(Substitute::New {
            pattern,
            replacement: String::new(),
            flags: Flags::default(),
        });
    };
    let (replacement, rest) = split_at_delimiter(rest, delimiter);
    let (flags, _) = match rest {
        Some(rest) => parse_flags(rest)?,
        None => (Flags::default(), false),
    };
    Ok(Substitute::New {
        pattern,
        replacement,
        flags,
    })
}

/// The text up to an unescaped `delimiter` (`\` + delimiter is the
/// delimiter itself; other escapes stay), and what follows it, if it came.
fn split_at_delimiter(text: &str, delimiter: char) -> (String, Option<&str>) {
    let mut part = String::new();
    let mut chars = text.char_indices();
    while let Some((ix, c)) = chars.next() {
        if c == delimiter {
            return (part, Some(&text[ix + c.len_utf8()..]));
        }
        if c == '\\' {
            match chars.next() {
                Some((_, next)) if next == delimiter => part.push(next),
                Some((_, next)) => {
                    part.push('\\');
                    part.push(next);
                }
                None => part.push('\\'),
            }
        } else {
            part.push(c);
        }
    }
    (part, None)
}

/// `[&][g][i][I][n][e]`; `&` first keeps the last substitution's flags.
fn parse_flags(text: &str) -> Result<(Flags, bool), String> {
    let mut flags = Flags::default();
    let (keep, text) = match text.strip_prefix('&') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    for (ix, c) in text.char_indices() {
        match c {
            'g' => flags.global = !flags.global,
            'i' => flags.ignore_case = Some(true),
            'I' => flags.ignore_case = Some(false),
            'n' => flags.count_only = true,
            'e' => flags.quiet = true,
            'c' => return Err("E: :s with c (confirm each) is not supported".into()),
            _ => {
                let rest = text[ix..].trim();
                if rest.is_empty() {
                    break;
                }
                return Err(format!("E488: Trailing characters: {rest}"));
            }
        }
    }
    Ok((flags, keep))
}

/// Merge the flags of a `:&&`: the last substitution's, and those typed.
pub fn merge(kept: Flags, typed: Flags) -> Flags {
    Flags {
        global: kept.global != typed.global,
        ignore_case: typed.ignore_case.or(kept.ignore_case),
        count_only: kept.count_only || typed.count_only,
        quiet: kept.quiet || typed.quiet,
    }
}

/// The largest 'shiftwidth' `:set` takes.
pub const MAX_SHIFTWIDTH: usize = 99;

/// The options `:set` changes. They last while smep runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// 'hlsearch': highlight the last search's matches.
    pub hlsearch: bool,
    pub ignorecase: bool,
    pub smartcase: bool,
    /// 'wrapscan': searches go round the end of the document.
    pub wrapscan: bool,
    /// 'shiftwidth': the columns of `>`, `<` and Tab.
    pub shiftwidth: usize,
    /// 'number': line numbers beside the editor.
    pub number: bool,
    /// 'wrap': long lines wrap.
    pub wrap: bool,
}

impl Default for Options {
    /// smep's own look: highlighting on, two-space shifts, lines wrapped,
    /// no line numbers.
    fn default() -> Self {
        Self {
            hlsearch: true,
            ignorecase: false,
            smartcase: false,
            wrapscan: true,
            shiftwidth: super::SHIFT_WIDTH,
            number: false,
            wrap: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Opt {
    Hlsearch,
    Ignorecase,
    Smartcase,
    Wrapscan,
    Shiftwidth,
    Number,
    Wrap,
}

const NAMES: [(Opt, &str, &str); 7] = [
    (Opt::Hlsearch, "hlsearch", "hls"),
    (Opt::Ignorecase, "ignorecase", "ic"),
    (Opt::Smartcase, "smartcase", "scs"),
    (Opt::Wrapscan, "wrapscan", "ws"),
    (Opt::Shiftwidth, "shiftwidth", "sw"),
    (Opt::Number, "number", "nu"),
    (Opt::Wrap, "wrap", "wrap"),
];

fn lookup(name: &str) -> Option<Opt> {
    NAMES
        .iter()
        .find(|(_, long, short)| name == *long || name == *short)
        .map(|(opt, _, _)| *opt)
}

impl Options {
    fn flag(&mut self, opt: Opt) -> Option<&mut bool> {
        Some(match opt {
            Opt::Hlsearch => &mut self.hlsearch,
            Opt::Ignorecase => &mut self.ignorecase,
            Opt::Smartcase => &mut self.smartcase,
            Opt::Wrapscan => &mut self.wrapscan,
            Opt::Number => &mut self.number,
            Opt::Wrap => &mut self.wrap,
            Opt::Shiftwidth => return None,
        })
    }

    fn show(&self, opt: Opt) -> String {
        let name = NAMES.iter().find(|(o, _, _)| *o == opt).map_or("", |n| n.1);
        let mut this = self.clone();
        match this.flag(opt) {
            Some(on) if *on => name.to_string(),
            Some(_) => format!("no{name}"),
            None => format!("{name}={}", self.shiftwidth),
        }
    }

    /// `:set` with `args`: `opt`, `noopt`, `invopt`, `opt!`, `opt?`, `opt&`,
    /// `sw=4`, `sw+=2`, `sw-=2`, several at once; none (or `all`) lists them.
    /// Returns what to say, if anything; stops at the first error, the
    /// arguments before it applied.
    pub fn set(&mut self, args: &str) -> Result<Option<String>, String> {
        let args = args.trim();
        if args.is_empty() || args == "all" {
            let defaults = Options::default();
            let shown: Vec<String> = NAMES
                .iter()
                .filter(|(opt, _, _)| args == "all" || self.show(*opt) != defaults.show(*opt))
                .map(|(opt, _, _)| self.show(*opt))
                .collect();
            return Ok(Some(
                format!("--- Options --- {}", shown.join("  "))
                    .trim_end()
                    .into(),
            ));
        }
        let mut said = Vec::new();
        for arg in args.split_whitespace() {
            if let Some(name) = arg.strip_suffix('?') {
                let opt = lookup(name).ok_or_else(|| format!("E518: Unknown option: {name}"))?;
                said.push(self.show(opt));
                continue;
            }
            if let Some(name) = arg.strip_suffix('&') {
                let opt = lookup(name).ok_or_else(|| format!("E518: Unknown option: {name}"))?;
                let defaults = Options::default();
                match self.flag(opt) {
                    Some(on) => *on = defaults.clone().flag(opt).is_some_and(|d| *d),
                    None => self.shiftwidth = defaults.shiftwidth,
                }
                continue;
            }
            if let Some(at) = arg.find(['=', ':']) {
                let (name, op) = match arg[..at].strip_suffix(['+', '-', '^']) {
                    Some(name) => (name, arg[name.len()..=at].to_string()),
                    None => (&arg[..at], arg[at..=at].to_string()),
                };
                let opt = lookup(name).ok_or_else(|| format!("E518: Unknown option: {name}"))?;
                if opt != Opt::Shiftwidth {
                    return Err(format!("E474: Invalid argument: {arg}"));
                }
                let value: usize = arg[at + 1..]
                    .parse()
                    .map_err(|_| format!("E521: Number required after =: {arg}"))?;
                let width = match op.as_str() {
                    "+=" => self.shiftwidth.saturating_add(value),
                    "-=" => self.shiftwidth.saturating_sub(value),
                    "^=" => self.shiftwidth.saturating_mul(value),
                    _ => value,
                };
                if width == 0 {
                    return Err(format!("E487: Argument must be positive: {arg}"));
                }
                if width > MAX_SHIFTWIDTH {
                    return Err(format!("E474: Invalid argument: {arg}"));
                }
                self.shiftwidth = width;
                continue;
            }
            let (name, value) = if let Some(name) = arg.strip_suffix('!') {
                (name, None)
            } else if let Some(name) = arg.strip_prefix("inv").filter(|n| lookup(n).is_some()) {
                (name, None)
            } else if let Some(name) = arg.strip_prefix("no").filter(|n| lookup(n).is_some()) {
                (name, Some(false))
            } else {
                (arg, Some(true))
            };
            let opt = lookup(name).ok_or_else(|| format!("E518: Unknown option: {arg}"))?;
            match (self.flag(opt), value) {
                (Some(on), Some(value)) => *on = value,
                (Some(on), None) => *on = !*on,
                // `:set sw` shows it; `:set nosw` is no use.
                (None, Some(true)) => said.push(self.show(opt)),
                (None, _) => return Err(format!("E474: Invalid argument: {arg}")),
            }
        }
        Ok((!said.is_empty()).then(|| said.join("  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINES: Lines = Lines {
        current: 5,
        last: 10,
        marks: Some((2, 4)),
    };

    fn range(command: &str) -> Result<(Option<LineRange>, &str), String> {
        parse_range(command, &LINES)
    }

    #[test]
    fn ranges_read_as_vim_reads_them() {
        assert_eq!(range("s/a/b/"), Ok((None, "s/a/b/")));
        assert_eq!(range("%s"), Ok((Some((1, 10)), "s")));
        assert_eq!(range("3"), Ok((Some((3, 3)), "")));
        assert_eq!(range("2,4s"), Ok((Some((2, 4)), "s")));
        assert_eq!(range(".,$"), Ok((Some((5, 10)), "")));
        assert_eq!(range(".+1,$-2"), Ok((Some((6, 8)), "")));
        assert_eq!(range("+2"), Ok((Some((7, 7)), "")));
        assert_eq!(range("-"), Ok((Some((4, 4)), "")));
        assert_eq!(range("'<,'>s"), Ok((Some((2, 4)), "s")));
        assert_eq!(range("3;+2"), Ok((Some((3, 5)), "")));
        assert_eq!(range("3,+2"), Ok((Some((3, 7)), "")));
        assert_eq!(range(",7"), Ok((Some((5, 7)), "")));
        assert_eq!(range("7,"), Ok((Some((5, 7)), "")));
        assert_eq!(range("8,3"), Ok((Some((3, 8)), "")), "swapped");
        assert_eq!(range("0"), Ok((Some((0, 0)), "")));
        assert_eq!(range("11").unwrap_err(), "E16: Invalid range");
        assert_eq!(range("1-5").unwrap_err(), "E16: Invalid range");
        assert_eq!(
            range("'a").unwrap_err(),
            "E: only the marks '< and '> are supported"
        );
        let unmarked = Lines {
            marks: None,
            ..LINES
        };
        assert_eq!(
            parse_range("'<,'>s", &unmarked).unwrap_err(),
            "E20: Mark not set"
        );
    }

    #[test]
    fn substitute_arguments_split() {
        let new = |pattern: &str, replacement: &str, flags: Flags| {
            Ok(Substitute::New {
                pattern: pattern.into(),
                replacement: replacement.into(),
                flags,
            })
        };
        let g = Flags {
            global: true,
            ..Flags::default()
        };
        assert_eq!(parse_substitute("/a/b/"), new("a", "b", Flags::default()));
        assert_eq!(parse_substitute("/a/b/g"), new("a", "b", g));
        assert_eq!(parse_substitute("/a/b"), new("a", "b", Flags::default()));
        assert_eq!(parse_substitute("/a"), new("a", "", Flags::default()));
        assert_eq!(parse_substitute("#a/b#c#g"), new("a/b", "c", g));
        assert_eq!(
            parse_substitute("/a\\/b/c\\/d/"),
            new("a/b", "c/d", Flags::default())
        );
        assert_eq!(
            parse_substitute("/a\\.b/\\1\\&/"),
            new("a\\.b", "\\1\\&", Flags::default())
        );
        assert_eq!(
            parse_substitute("/中/文/"),
            new("中", "文", Flags::default())
        );
        assert_eq!(
            parse_substitute("/a/b/gIne"),
            new(
                "a",
                "b",
                Flags {
                    global: true,
                    ignore_case: Some(false),
                    count_only: true,
                    quiet: true,
                }
            )
        );
        assert_eq!(parse_substitute("/a/b/gg"), new("a", "b", Flags::default()));
        assert_eq!(
            parse_substitute(""),
            Ok(Substitute::Repeat {
                flags: Flags::default(),
                keep: false
            })
        );
        assert_eq!(
            parse_substitute(" g"),
            Ok(Substitute::Repeat {
                flags: g,
                keep: false
            })
        );
        assert_eq!(
            parse_substitute("&"),
            Ok(Substitute::Repeat {
                flags: Flags::default(),
                keep: true
            })
        );
        assert_eq!(
            parse_substitute("/a/b/c").unwrap_err(),
            "E: :s with c (confirm each) is not supported"
        );
        assert_eq!(
            parse_substitute("/a/b/g 3").unwrap_err(),
            "E488: Trailing characters: 3"
        );
        assert_eq!(
            parse_substitute("xaxbx").unwrap_err(),
            "E146: Regular expressions can't be delimited by letters"
        );
        assert!(!merge(g, g).global, "`g` again toggles");
    }

    #[test]
    fn set_changes_and_shows_options() {
        let mut options = Options::default();
        assert_eq!(options.set("ic"), Ok(None));
        assert!(options.ignorecase);
        assert_eq!(options.set("ic?"), Ok(Some("ignorecase".into())));
        assert_eq!(options.set("noic scs"), Ok(None));
        assert!(!options.ignorecase && options.smartcase);
        assert_eq!(options.set("invscs ws!"), Ok(None));
        assert!(!options.smartcase && !options.wrapscan);
        assert_eq!(options.set("ws&"), Ok(None));
        assert!(options.wrapscan);
        assert_eq!(options.set("sw=4"), Ok(None));
        assert_eq!(options.set("sw"), Ok(Some("shiftwidth=4".into())));
        assert_eq!(
            options.set("shiftwidth+=2 sw?"),
            Ok(Some("shiftwidth=6".into()))
        );
        assert_eq!(options.set("sw-=5"), Ok(None));
        assert_eq!(options.shiftwidth, 1);
        assert_eq!(options.set("sw:3"), Ok(None));
        assert_eq!(options.shiftwidth, 3);
        assert_eq!(options.set("nu nowrap hls?"), Ok(Some("hlsearch".into())));
        assert!(options.number && !options.wrap);
        assert_eq!(
            options.set(""),
            Ok(Some("--- Options --- shiftwidth=3  number  nowrap".into()))
        );
        assert_eq!(options.set("sw& nonu wrap"), Ok(None));
        assert_eq!(options, Options::default());
        assert_eq!(options.set(""), Ok(Some("--- Options ---".into())));
        assert!(
            options
                .set("all")
                .unwrap()
                .unwrap()
                .contains("hlsearch  noignorecase")
        );

        // Errors stop there; what came before stays set.
        assert_eq!(
            options.set("ic foo").unwrap_err(),
            "E518: Unknown option: foo"
        );
        assert!(options.ignorecase);
        assert_eq!(
            options.set("sw=x").unwrap_err(),
            "E521: Number required after =: sw=x"
        );
        assert_eq!(
            options.set("sw=0").unwrap_err(),
            "E487: Argument must be positive: sw=0"
        );
        assert_eq!(
            options.set("sw=100").unwrap_err(),
            "E474: Invalid argument: sw=100"
        );
        assert_eq!(
            options.set("ic=1").unwrap_err(),
            "E474: Invalid argument: ic=1"
        );
        assert_eq!(
            options.set("nosw").unwrap_err(),
            "E474: Invalid argument: nosw"
        );
        assert_eq!(
            options.set("nofoo").unwrap_err(),
            "E518: Unknown option: nofoo"
        );
        assert_eq!(options.shiftwidth, 2);
    }
}
