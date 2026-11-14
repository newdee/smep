//! Which headings the outline lists: all of them down to a depth, less the
//! ones inside a section folded away.
//!
//! A section is known by its level, its title and which of the headings
//! with that level and title it is ([`SectionKey`]), not by its index or
//! line: those move with every edit above it, and a fold should stay put
//! while the text is written.

use std::collections::HashSet;

use crate::highlight::Heading;

/// The deepest heading level, and the depth that lists every heading.
pub const ALL_LEVELS: u8 = 6;

/// A section across edits: its heading's level and title, and how many
/// headings with the same level and title come before it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SectionKey {
    level: u8,
    title: String,
    nth: usize,
}

/// The key of every heading, in order.
pub fn keys(headings: &[Heading]) -> Vec<SectionKey> {
    let mut seen: Vec<(u8, &str)> = Vec::with_capacity(headings.len());
    headings
        .iter()
        .map(|heading| {
            let nth = seen
                .iter()
                .filter(|&&(level, title)| level == heading.level && title == heading.title)
                .count();
            seen.push((heading.level, &heading.title));
            SectionKey {
                level: heading.level,
                title: heading.title.clone(),
                nth,
            }
        })
        .collect()
}

/// One line of the outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// The heading's index in the document's headings.
    pub ix: usize,
    /// Whether the section holds headings the depth would list, so it can
    /// fold.
    pub foldable: bool,
    /// Whether it is folded (only ever true when foldable).
    pub folded: bool,
}

/// The lines of the outline: every heading at `depth` or above, less those
/// inside a section in `folded`. A section ends at the next heading of its
/// level or above.
pub fn rows(headings: &[Heading], folded: &HashSet<SectionKey>, depth: u8) -> Vec<Row> {
    let keys = keys(headings);
    let mut rows = Vec::new();
    // The level of the folded section being skipped, if any.
    let mut hiding: Option<u8> = None;
    for (ix, heading) in headings.iter().enumerate() {
        if hiding.is_some_and(|level| heading.level > level) {
            continue;
        }
        hiding = None;
        if heading.level > depth {
            continue;
        }
        let foldable = headings[ix + 1..]
            .iter()
            .take_while(|next| next.level > heading.level)
            .any(|next| next.level <= depth);
        let folded = foldable && folded.contains(&keys[ix]);
        if folded {
            hiding = Some(heading.level);
        }
        rows.push(Row {
            ix,
            foldable,
            folded,
        });
    }
    rows
}

/// The line that stands for heading `ix` among `rows`: its own, or that of
/// the nearest section holding it that is listed.
pub fn shown_for(headings: &[Heading], rows: &[Row], ix: usize) -> Option<usize> {
    let mut at = ix;
    loop {
        if rows.iter().any(|row| row.ix == at) {
            return Some(at);
        }
        let level = headings.get(at)?.level;
        at = headings[..at].iter().rposition(|h| h.level < level)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(levels: &[(u8, &str)]) -> Vec<Heading> {
        levels
            .iter()
            .enumerate()
            .map(|(line, &(level, title))| Heading {
                level,
                title: title.to_string(),
                line,
                block: line,
            })
            .collect()
    }

    fn listed<'a>(
        headings: &'a [Heading],
        folded: &HashSet<SectionKey>,
        depth: u8,
    ) -> Vec<&'a str> {
        rows(headings, folded, depth)
            .iter()
            .map(|row| headings[row.ix].title.as_str())
            .collect()
    }

    fn fold(headings: &[Heading], ix: usize) -> HashSet<SectionKey> {
        HashSet::from([keys(headings)[ix].clone()])
    }

    #[test]
    fn a_folded_section_hides_what_it_holds_and_no_more() {
        let headings = doc(&[
            (1, "A"),
            (2, "A1"),
            (3, "A1a"),
            (2, "A2"),
            (1, "B"),
            (2, "B1"),
        ]);
        let none = HashSet::new();
        assert_eq!(
            listed(&headings, &none, ALL_LEVELS),
            ["A", "A1", "A1a", "A2", "B", "B1"]
        );
        // A1 holds A1a only; A holds up to B.
        assert_eq!(
            listed(&headings, &fold(&headings, 1), ALL_LEVELS),
            ["A", "A1", "A2", "B", "B1"]
        );
        assert_eq!(
            listed(&headings, &fold(&headings, 0), ALL_LEVELS),
            ["A", "B", "B1"]
        );
        let rows = rows(&headings, &fold(&headings, 0), ALL_LEVELS);
        assert!(rows[0].foldable && rows[0].folded);
        assert!(rows[1].foldable && !rows[1].folded, "B");
        assert!(!rows[2].foldable, "B1 holds nothing");
        // A fold on a heading with nothing under it changes nothing.
        assert_eq!(
            listed(&headings, &fold(&headings, 5), ALL_LEVELS),
            ["A", "A1", "A1a", "A2", "B", "B1"]
        );
        assert!(!rows.iter().any(|row| row.folded && !row.foldable));
    }

    #[test]
    fn the_depth_lists_the_levels_above_it() {
        let headings = doc(&[(1, "A"), (2, "A1"), (3, "A1a"), (2, "A2"), (4, "deep")]);
        let none = HashSet::new();
        assert_eq!(listed(&headings, &none, 1), ["A"]);
        assert_eq!(listed(&headings, &none, 2), ["A", "A1", "A2"]);
        assert_eq!(listed(&headings, &none, 3), ["A", "A1", "A1a", "A2"]);
        // Only what the depth lists makes a section foldable.
        let at_two = rows(&headings, &none, 2);
        assert!(at_two[0].foldable, "A holds A1 and A2");
        assert!(!at_two[1].foldable, "A1a is past the depth");
        assert!(!at_two[2].foldable, "so is deep");
        assert!(!rows(&headings, &none, 1)[0].foldable);
        // A document that starts below h1, or skips a level.
        let headings = doc(&[(2, "x"), (4, "y"), (3, "z"), (1, "w")]);
        assert_eq!(listed(&headings, &none, ALL_LEVELS), ["x", "y", "z", "w"]);
        assert_eq!(
            listed(&headings, &fold(&headings, 0), ALL_LEVELS),
            ["x", "w"]
        );
        assert_eq!(listed(&headings, &none, 3), ["x", "z", "w"]);
        assert!(listed(&[], &none, ALL_LEVELS).is_empty());
    }

    #[test]
    fn a_fold_stays_with_its_section_through_edits() {
        let before = doc(&[(1, "Intro"), (2, "Same"), (3, "x"), (2, "Same"), (3, "y")]);
        // The second "Same" is folded.
        let folded = fold(&before, 3);
        assert_eq!(
            listed(&before, &folded, ALL_LEVELS),
            ["Intro", "Same", "x", "Same"]
        );
        // A heading added above, another one renamed: the fold holds.
        let after = doc(&[
            (1, "New"),
            (1, "Introduction"),
            (2, "Same"),
            (3, "x"),
            (2, "Same"),
            (3, "y"),
        ]);
        assert_eq!(
            listed(&after, &folded, ALL_LEVELS),
            ["New", "Introduction", "Same", "x", "Same"]
        );
        // The section renamed: a new section, unfolded.
        let renamed = doc(&[(1, "Intro"), (2, "Same"), (3, "x"), (2, "Other"), (3, "y")]);
        assert_eq!(listed(&renamed, &folded, ALL_LEVELS).len(), 5);
    }

    #[test]
    fn a_hidden_heading_is_shown_by_the_section_holding_it() {
        let headings = doc(&[(1, "A"), (2, "A1"), (3, "A1a"), (1, "B")]);
        let none = HashSet::new();
        let all = rows(&headings, &none, ALL_LEVELS);
        assert_eq!(shown_for(&headings, &all, 2), Some(2));
        let folded = rows(&headings, &fold(&headings, 1), ALL_LEVELS);
        assert_eq!(shown_for(&headings, &folded, 2), Some(1));
        let folded = rows(&headings, &fold(&headings, 0), ALL_LEVELS);
        assert_eq!(shown_for(&headings, &folded, 2), Some(0));
        assert_eq!(shown_for(&headings, &folded, 3), Some(3));
        // Past the depth: the nearest listed section above.
        let shallow = rows(&headings, &none, 1);
        assert_eq!(shown_for(&headings, &shallow, 2), Some(0));
        // A heading above every listed one has nothing to stand for it.
        let headings = doc(&[(3, "c"), (1, "A")]);
        let shallow = rows(&headings, &none, 1);
        assert_eq!(shown_for(&headings, &shallow, 0), None);
        assert_eq!(shown_for(&headings, &shallow, 9), None);
    }
}
