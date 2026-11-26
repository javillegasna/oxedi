//! Finds the text a problem is written with outside the problem module: an `impl Display` for a
//! type and a function named `render_*`. Only the listed notation types and renderers may stay.

use super::blank::{blank_comments_and_escapes, is_ident};
use super::files::{collect_rs, is_test_file};
use super::table::Listed;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub file: String,
    pub line: usize,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum List {
    Notation,
    Renderers,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TextViolation {
    Outside(List, Found),
    Unused(List, Found),
}

const TABLE_FILE: &str = "tests/architecture/table.rs";

impl fmt::Display for TextViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Self::Outside(kind, _) | Self::Unused(kind, _)) = self;
        let (what, list) = match kind {
            List::Notation => ("impl Display for", "NOTATION"),
            List::Renderers => ("fn", "RENDERERS"),
        };
        match self {
            Self::Outside(_, Found { file, line, name }) => write!(
                f,
                "{file}:{line} · {what} {name} outside problem/ · write a problem's text in problem/, or add {name} of {file} to {list} in {TABLE_FILE} with the reason"
            ),
            Self::Unused(_, Found { file, name, .. }) => write!(
                f,
                "{list} lists {name} in {file}, and that file does not hold it outside problem/ · remove it from {list} in {TABLE_FILE}"
            ),
        }
    }
}

fn skipped(rel: &Path) -> bool {
    rel.starts_with("problem") || is_test_file(rel)
}

fn is_word_at(b: &[u8], at: usize, word: &str) -> bool {
    let end = at + word.len();
    b[at..].starts_with(word.as_bytes())
        && (at == 0 || !is_ident(b[at - 1]))
        && b.get(end).is_none_or(|&c| !is_ident(c))
}

fn skip_space(b: &[u8], mut at: usize) -> usize {
    while b.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

fn skip_generics(b: &[u8], at: usize) -> usize {
    if b.get(at) != Some(&b'<') {
        return at;
    }
    let mut depth = 0usize;
    for (i, &c) in b.iter().enumerate().skip(at) {
        match c {
            b'<' => depth += 1,
            b'>' if i > 0 && b[i - 1] == b'-' => {}
            b'>' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    panic!("the generics at byte {at} never close · fix the scanner in tests/architecture/text.rs")
}

fn path_at(b: &[u8], at: usize) -> (&str, usize) {
    let mut end = at;
    while b.get(end).is_some_and(|&c| is_ident(c) || c == b':') {
        end += 1;
    }
    (std::str::from_utf8(&b[at..end]).unwrap_or_default(), end)
}

fn last_segment(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn line_of(b: &[u8], at: usize) -> usize {
    1 + b[..at].iter().filter(|&&c| c == b'\n').count()
}

/// The type an `impl` at `at` writes `Display` for, when it is a `Display` impl.
fn display_target(b: &[u8], at: usize) -> Option<&str> {
    let after_generics = skip_generics(b, skip_space(b, at + "impl".len()));
    let (tr, end) = path_at(b, skip_space(b, after_generics));
    if last_segment(tr) != "Display" {
        return None;
    }
    let at_for = skip_space(b, end);
    if !is_word_at(b, at_for, "for") {
        return None;
    }
    let mut at_type = skip_space(b, at_for + "for".len());
    while b.get(at_type) == Some(&b'&') {
        at_type = skip_space(b, at_type + 1);
    }
    let (ty, _) = path_at(b, at_type);
    Some(last_segment(ty))
}

/// Every `impl Display for T` in a blanked source outside `problem/` and the test files.
pub fn displays_in(rel: &Path, blanked: &[u8]) -> Vec<Found> {
    if skipped(rel) {
        return Vec::new();
    }
    (0..blanked.len())
        .filter(|&at| is_word_at(blanked, at, "impl"))
        .filter_map(|at| {
            let name = display_target(blanked, at)?.to_owned();
            Some(Found {
                file: rel.display().to_string(),
                line: line_of(blanked, at),
                name,
            })
        })
        .collect()
}

pub fn renderers_in(rel: &Path, blanked: &[u8]) -> Vec<Found> {
    if skipped(rel) {
        return Vec::new();
    }
    (0..blanked.len())
        .filter(|&at| is_word_at(blanked, at, "fn"))
        .filter_map(|at| {
            let (name, _) = path_at(blanked, skip_space(blanked, at + "fn".len()));
            name.starts_with("render_").then(|| Found {
                file: rel.display().to_string(),
                line: line_of(blanked, at),
                name: name.to_owned(),
            })
        })
        .collect()
}

pub fn text_finds(src: &Path) -> (Vec<Found>, Vec<Found>) {
    let mut files = Vec::new();
    collect_rs(src, src, &mut files);
    files.sort();
    let (mut displays, mut renderers) = (Vec::new(), Vec::new());
    for rel in &files {
        let path = src.join(rel);
        let text = fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "cannot read the source file {}: {e} · it must be UTF-8 and readable",
                path.display()
            )
        });
        let blanked = blank_comments_and_escapes(&text);
        displays.extend(displays_in(rel, &blanked));
        renderers.extend(renderers_in(rel, &blanked));
    }
    (displays, renderers)
}

fn is_listed(list: &[Listed], found: &Found) -> bool {
    list.iter()
        .any(|l| l.file == found.file && l.name == found.name)
}

/// Everything wrong between what the files hold and the two lists, each name keyed by its file.
pub fn text_violations(
    displays: Vec<Found>,
    renderers: Vec<Found>,
    notation: &[Listed],
    allowed: &[Listed],
) -> Vec<TextViolation> {
    let mut out = Vec::new();
    for (kind, found, list) in [
        (List::Notation, displays, notation),
        (List::Renderers, renderers, allowed),
    ] {
        out.extend(
            found
                .iter()
                .filter(|f| !is_listed(list, f))
                .map(|f| TextViolation::Outside(kind, f.clone())),
        );
        out.extend(
            list.iter()
                .filter(|l| !found.iter().any(|f| f.file == l.file && f.name == l.name))
                .map(|l| {
                    TextViolation::Unused(
                        kind,
                        Found {
                            file: l.file.to_owned(),
                            line: 0,
                            name: l.name.to_owned(),
                        },
                    )
                }),
        );
    }
    out
}
