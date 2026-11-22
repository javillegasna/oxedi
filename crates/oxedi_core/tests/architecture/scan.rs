//! Reads which module of the core names which other module, from the text of its files.
//!
//! The reader works on text, not on a parse: it blanks the comments (keeping line breaks, so
//! line numbers hold) and then looks for `crate::` and `super::` paths. A path inside a string
//! or a macro counts, because it names a module all the same. It also finds the calls to the
//! shortcuts of `table.rs`.

use super::blank::{is_ident, strip_comments};
use super::table::Shortcut;
use std::collections::BTreeSet;
use std::path::Path;

/// What a path of a file names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Another module of the core, by its folder name.
    Module(String),
    /// An item the crate root re-exports, named without its module.
    Root(String),
    /// A `super::` chain that climbs out of the file's module.
    Outside,
}

/// One path of one file that names a module other than its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Use {
    /// The module the file belongs to: the first folder of its path.
    pub from: String,
    pub to: Target,
    /// The file, relative to `src`, with `/` as separator.
    pub file: String,
    /// 1-based line of the path.
    pub line: usize,
    /// The path as written, from `crate` or `super` to its end.
    pub path: String,
}

/// The uses in one file. `rel` is its path relative to `src`; `modules` the core's modules.
pub fn uses_in(rel: &Path, text: &str, modules: &BTreeSet<String>) -> Vec<Use> {
    let clean = strip_comments(text);
    let from = module_of(rel);
    let reader = Reader {
        lines: line_starts(&clean),
        from,
        file: rel.to_string_lossy().replace('\\', "/"),
        depth: module_depth(rel),
        modules,
        clean,
    };
    reader.scan()
}

/// The module a file belongs to: the first folder of its path relative to `src`.
fn module_of(rel: &Path) -> String {
    rel.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The calls in one file to the shortcuts another module writes, each a use of the module
/// that writes it. `rel` is the file's path relative to `src`. A call counts outside comments
/// only, and only as a whole path: no name character right before or after it.
pub fn shortcut_uses(rel: &Path, text: &str, shortcuts: &[Shortcut]) -> Vec<Use> {
    let clean = strip_comments(text);
    let lines = line_starts(&clean);
    let from = module_of(rel);
    let file = rel.to_string_lossy().replace('\\', "/");
    let mut found = Vec::new();
    for shortcut in shortcuts.iter().filter(|s| s.module != from) {
        found.extend(calls_at(&clean, shortcut.call).map(|at| Use {
            from: from.clone(),
            to: Target::Module(shortcut.module.to_owned()),
            file: file.clone(),
            line: lines.partition_point(|&start| start <= at),
            path: shortcut.call.to_owned(),
        }));
    }
    found
}

/// Where `call` starts in `b` as a whole path: no name character right before or after it.
fn calls_at<'a>(b: &'a [u8], call: &'a str) -> impl Iterator<Item = usize> + 'a {
    let needle = call.as_bytes();
    let free = |next: Option<&u8>| !next.is_some_and(|&byte| is_ident(byte));
    (0..b.len()).filter(move |&at| {
        b[at..].starts_with(needle)
            && free(at.checked_sub(1).and_then(|before| b.get(before)))
            && free(b.get(at + needle.len()))
    })
}

/// How deep the file's own module sits: `a/mod.rs` is 1, `a/b.rs` and `a/b/mod.rs` are 2.
fn module_depth(rel: &Path) -> usize {
    let parts = rel.components().count();
    if rel.file_name().is_some_and(|n| n == "mod.rs") {
        parts - 1
    } else {
        parts
    }
}

struct Reader<'a> {
    clean: Vec<u8>,
    lines: Vec<usize>,
    from: String,
    file: String,
    depth: usize,
    modules: &'a BTreeSet<String>,
}

const CRATE: &[u8] = b"crate::";
const SUPER: &[u8] = b"super::";

impl Reader<'_> {
    fn scan(&self) -> Vec<Use> {
        let mut found = Vec::new();
        let mut at = 0;
        while at < self.clean.len() {
            at = if !self.starts_path(at) {
                at + 1
            } else if self.clean[at..].starts_with(CRATE) {
                self.crate_at(at, &mut found)
            } else {
                self.super_at(at, &mut found)
            };
        }
        found
    }

    /// A `crate::` or `super::` that is not the tail of a longer name (`$crate::` is one).
    fn starts_path(&self, at: usize) -> bool {
        let rest = &self.clean[at..];
        let keyword = rest.starts_with(CRATE) || rest.starts_with(SUPER);
        keyword && (at == 0 || !is_ident(self.clean[at - 1]))
    }

    /// Reads the path (or group of paths) after `crate::` and returns where to go on.
    fn crate_at(&self, at: usize, found: &mut Vec<Use>) -> usize {
        let after = at + CRATE.len();
        if self.clean.get(after) != Some(&b'{') {
            self.item(after, found);
            return after;
        }
        let (starts, end) = group_item_starts(&self.clean, after);
        starts.iter().for_each(|&s| self.item(s, found));
        end
    }

    /// A `super::` chain is a use only when it climbs out of the file's own module.
    fn super_at(&self, at: usize, found: &mut Vec<Use>) -> usize {
        let mut end = at;
        let mut climbs = 0;
        while self.clean[end..].starts_with(SUPER) {
            end += SUPER.len();
            climbs += 1;
        }
        if climbs >= self.depth {
            found.push(self.make(at, Target::Outside, path_text(&self.clean, at)));
        }
        end
    }

    /// One path that starts at `start`, right after `crate::` or at an item of its group.
    fn item(&self, start: usize, found: &mut Vec<Use>) {
        let name = if self.clean.get(start) == Some(&b'*') {
            "*".to_owned()
        } else {
            text_at(&self.clean, start, is_ident)
        };
        let to = if self.modules.contains(&name) {
            Target::Module(name)
        } else {
            Target::Root(name)
        };
        let own = matches!(&to, Target::Module(m) if *m == self.from);
        if !own && !matches!(&to, Target::Root(n) if n.is_empty()) {
            let path = format!("crate::{}", path_text(&self.clean, start));
            found.push(self.make(start, to, path));
        }
    }

    fn make(&self, at: usize, to: Target, path: String) -> Use {
        Use {
            from: self.from.clone(),
            to,
            file: self.file.clone(),
            line: self.lines.partition_point(|&s| s <= at),
            path,
        }
    }
}

/// The bytes from `at` on that `keep` accepts, as text.
fn text_at(b: &[u8], at: usize, keep: fn(u8) -> bool) -> String {
    let len = b[at..].iter().take_while(|&&c| keep(c)).count();
    String::from_utf8_lossy(&b[at..at + len]).into_owned()
}

/// The path written at `from`, without any group that follows it.
fn path_text(b: &[u8], from: usize) -> String {
    let text = text_at(b, from, |c| is_ident(c) || b":$*".contains(&c));
    text.trim_end_matches(':').to_owned()
}

/// Offset of the first byte of each line.
fn line_starts(b: &[u8]) -> Vec<usize> {
    let breaks = b.iter().enumerate().filter(|&(_, &c)| c == b'\n');
    std::iter::once(0)
        .chain(breaks.map(|(i, _)| i + 1))
        .collect()
}

/// The start of each first-level item of the `{...}` group that opens at `open`, and the
/// offset after its closing brace. Nested groups belong to their item.
fn group_item_starts(b: &[u8], open: usize) -> (Vec<usize>, usize) {
    let mut starts = Vec::new();
    let mut depth = 0;
    let mut item_next = true;
    for (i, &c) in b.iter().enumerate().skip(open) {
        match c {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b',' if depth == 1 => item_next = true,
            _ if depth == 1 && item_next && !c.is_ascii_whitespace() => {
                starts.push(i);
                item_next = false;
            }
            _ => {}
        }
        if depth == 0 {
            return (starts, i + 1);
        }
    }
    (starts, b.len())
}
