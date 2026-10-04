//! Findings about a file's data, each one readable on its own.
//!
//! A diagnostic names the rule that failed, where it failed (segment index,
//! element and component position, and the open loops with the ordinal of
//! each instance) and the offending value as it appears in the file. It holds
//! owned values only, so it can be printed, stored or sent elsewhere without
//! the spec or the document that produced it.

use std::fmt;

use crate::document::{Document, Span};
use crate::spec::{ElementType, render_key};

/// The SNIP validation level a rule belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SnipLevel {
    /// Integrity: envelopes, control numbers and counts, segment structure.
    L1,
    /// Requirements: required elements, types and lengths.
    L2,
    /// Balancing: amounts that must add up.
    L3,
}

impl fmt::Display for SnipLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self {
            SnipLevel::L1 => 1,
            SnipLevel::L2 => 2,
            SnipLevel::L3 => 3,
        };
        write!(f, "SNIP {level}")
    }
}

/// One open loop instance: the loop's name and the 1-based ordinal of the
/// instance among every instance of that loop in the stream.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopRef {
    /// Loop name as the spec writes it, e.g. `2100`.
    pub name: String,
    /// 1 for the first instance of the loop, 2 for the second, and so on.
    pub ordinal: usize,
}

impl fmt::Display for LoopRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", render_key(&self.name), self.ordinal)
    }
}

/// The rule a diagnostic reports, with the values its message needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule {
    /// The input starts with a UTF-8 byte order mark, read as leading trivia
    /// of the first segment. Informational: nothing else changes.
    ByteOrderMark,
    /// No open loop holds the segment and it opens no loop.
    UnknownSegment {
        /// The segment id.
        id: Vec<u8>,
    },
    /// A loop was opened without its own trigger, to hold a descendant.
    ImplicitLoop {
        /// The loop that was opened.
        loop_name: String,
        /// The loop's own trigger as the spec writes it, e.g.
        /// `"GS" with no conditions` or `"N1" where {1: "PR"}`.
        expected_trigger: String,
        /// Id of the segment whose loop needed it.
        caused_by: Vec<u8>,
    },
    /// A loop that declares an end segment closed without capturing it.
    UnterminatedLoop {
        /// The loop.
        loop_name: String,
        /// The end segment the spec declares for it.
        expected_end: Vec<u8>,
        /// Index of the segment that opened the instance; `None` for an
        /// instance opened implicitly.
        opened_at: Option<usize>,
    },
    /// A closing segment's count element does not match what it counts.
    ControlCountMismatch {
        /// The closing segment id, e.g. `SE`.
        segment_id: Vec<u8>,
        /// 1-based position of the count element.
        element: usize,
        /// The count observed in the stream.
        expected: usize,
        /// The count element as written.
        found: Vec<u8>,
    },
    /// A control or count element the spec names is absent from its segment.
    ControlElementMissing {
        /// The segment id, e.g. `SE`.
        segment_id: Vec<u8>,
        /// 1-based position of the absent element.
        element: usize,
    },
    /// A closing segment's control number differs from its opener's.
    ControlNumberMismatch {
        /// The opening segment id, e.g. `ST`.
        opener: Vec<u8>,
        /// 1-based position of the control number in the opener.
        opener_element: usize,
        /// The closing segment id, e.g. `SE`.
        closer: Vec<u8>,
        /// 1-based position of the control number in the closer.
        closer_element: usize,
        /// The opener's control number as written.
        opener_value: Vec<u8>,
        /// The closer's control number as written.
        closer_value: Vec<u8>,
        /// Index of the opening segment; `None` for an instance opened implicitly.
        opened_at: Option<usize>,
    },
    /// A required element (or component) is absent or empty.
    RequiredElementMissing {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, when the definition is a component.
        component: Option<usize>,
        /// The element's name in the spec.
        name: String,
    },
    /// A value does not parse as its declared type.
    TypeMismatch {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, when the definition is a component.
        component: Option<usize>,
        /// The element's name in the spec.
        name: String,
        /// The declared type.
        expected: ElementType,
    },
    /// A value is shorter or longer than its definition allows.
    LengthOutOfRange {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// 1-based component position, when the definition is a component.
        component: Option<usize>,
        /// The element's name in the spec.
        name: String,
        /// Declared minimum length.
        min: Option<usize>,
        /// Declared maximum length.
        max: Option<usize>,
        /// The value's length.
        length: usize,
    },
    /// A text value was not stored: its column cannot address more bytes.
    ValueDropped {
        /// The table.
        table: String,
        /// The column.
        column: String,
        /// The column's byte length the value would have produced.
        bytes: usize,
    },
    /// A composite element has more components than its definition declares.
    CompositeShape {
        /// The segment id.
        segment_id: Vec<u8>,
        /// 1-based element position.
        element: usize,
        /// The element's name in the spec.
        name: String,
        /// Highest component position the definition declares.
        declared: usize,
        /// Components found in the file.
        found: usize,
    },
}

impl Rule {
    /// The SNIP level the rule belongs to.
    pub fn level(&self) -> SnipLevel {
        match self {
            Rule::ByteOrderMark
            | Rule::UnknownSegment { .. }
            | Rule::ImplicitLoop { .. }
            | Rule::UnterminatedLoop { .. }
            | Rule::ControlCountMismatch { .. }
            | Rule::ControlElementMissing { .. }
            | Rule::ControlNumberMismatch { .. } => SnipLevel::L1,
            Rule::RequiredElementMissing { .. }
            | Rule::TypeMismatch { .. }
            | Rule::LengthOutOfRange { .. }
            | Rule::CompositeShape { .. }
            | Rule::ValueDropped { .. } => SnipLevel::L2,
        }
    }

    /// The name of the rule's variant, e.g. `RequiredElementMissing`, so a
    /// caller can filter findings without parsing messages.
    pub fn kind(&self) -> &'static str {
        match self {
            Rule::ByteOrderMark => "ByteOrderMark",
            Rule::UnknownSegment { .. } => "UnknownSegment",
            Rule::ImplicitLoop { .. } => "ImplicitLoop",
            Rule::UnterminatedLoop { .. } => "UnterminatedLoop",
            Rule::ControlCountMismatch { .. } => "ControlCountMismatch",
            Rule::ControlElementMissing { .. } => "ControlElementMissing",
            Rule::ControlNumberMismatch { .. } => "ControlNumberMismatch",
            Rule::RequiredElementMissing { .. } => "RequiredElementMissing",
            Rule::TypeMismatch { .. } => "TypeMismatch",
            Rule::LengthOutOfRange { .. } => "LengthOutOfRange",
            Rule::ValueDropped { .. } => "ValueDropped",
            Rule::CompositeShape { .. } => "CompositeShape",
        }
    }
}

/// Bytes from the file, quoted on one line: valid text is escaped as a Rust
/// string literal is, and each invalid byte is written as `\xNN`.
pub(crate) struct Quoted<'a>(pub(crate) &'a [u8]);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for chunk in self.0.utf8_chunks() {
            let escaped = format!("{:?}", chunk.valid());
            let inner = escaped
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix('"'))
                .unwrap_or(&escaped);
            f.write_str(inner)?;
            for byte in chunk.invalid() {
                write!(f, "\\x{byte:02X}")?;
            }
        }
        f.write_str("\"")
    }
}

/// An element reference in X12 style: `CLP01`, or `SVC01-2` for a component.
struct ElementRef<'a> {
    segment_id: &'a [u8],
    element: usize,
    component: Option<usize>,
}

impl fmt::Display for ElementRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{:02}",
            String::from_utf8_lossy(self.segment_id),
            self.element
        )?;
        match self.component {
            Some(component) => write!(f, "-{component}"),
            None => Ok(()),
        }
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Rule::ByteOrderMark => write!(
                f,
                "the input starts with a UTF-8 byte order mark, kept as leading trivia of the first segment"
            ),
            Rule::UnknownSegment { id } => write!(
                f,
                "segment {} is not part of the structure: no open loop holds it and it opens no loop",
                Quoted(id)
            ),
            Rule::ImplicitLoop {
                loop_name,
                expected_trigger,
                caused_by,
            } => write!(
                f,
                "loop {loop_name:?} opened without its own trigger ({expected_trigger}) to hold segment {}",
                Quoted(caused_by)
            ),
            Rule::UnterminatedLoop {
                loop_name,
                expected_end,
                opened_at,
            } => {
                write!(f, "loop {loop_name:?} ")?;
                if let Some(opened_at) = opened_at {
                    write!(f, "opened at segment #{opened_at} ")?;
                }
                write!(f, "closed without its end segment {}", Quoted(expected_end))
            }
            Rule::ControlCountMismatch {
                segment_id,
                element,
                expected,
                found,
            } => write!(
                f,
                "{} declares {} but the count is {expected}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                },
                Quoted(found)
            ),
            Rule::ControlElementMissing {
                segment_id,
                element,
            } => write!(
                f,
                "control element {} is missing: the segment has no element {element}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                }
            ),
            Rule::ControlNumberMismatch {
                opener,
                opener_element,
                closer,
                closer_element,
                opener_value,
                closer_value,
                opened_at,
            } => {
                write!(
                    f,
                    "{} {} does not match {} {}",
                    ElementRef {
                        segment_id: closer,
                        element: *closer_element,
                        component: None
                    },
                    Quoted(closer_value),
                    ElementRef {
                        segment_id: opener,
                        element: *opener_element,
                        component: None
                    },
                    Quoted(opener_value)
                )?;
                match opened_at {
                    Some(opened_at) => write!(f, " of segment #{opened_at}"),
                    None => Ok(()),
                }
            }
            Rule::RequiredElementMissing {
                segment_id,
                element,
                component,
                name,
            } => write!(
                f,
                "required element {} ({name}) is missing or empty",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: *component
                }
            ),
            Rule::TypeMismatch {
                segment_id,
                element,
                component,
                name,
                expected,
            } => write!(
                f,
                "element {} ({name}) is not a valid {expected}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: *component
                }
            ),
            Rule::LengthOutOfRange {
                segment_id,
                element,
                component,
                name,
                min,
                max,
                length,
            } => {
                write!(
                    f,
                    "element {} ({name}) has length {length}; the spec allows ",
                    ElementRef {
                        segment_id,
                        element: *element,
                        component: *component
                    }
                )?;
                match (min, max) {
                    (Some(min), Some(max)) => write!(f, "{min} to {max}"),
                    (Some(min), None) => write!(f, "at least {min}"),
                    (None, Some(max)) => write!(f, "at most {max}"),
                    (None, None) => write!(f, "any length"),
                }
            }
            Rule::ValueDropped {
                table,
                column,
                bytes,
            } => write!(
                f,
                "text for column {column:?} of table {table:?} was not stored: it would bring the column to {bytes} bytes and a column holds at most {}",
                i32::MAX
            ),
            Rule::CompositeShape {
                segment_id,
                element,
                name,
                declared,
                found,
            } => write!(
                f,
                "element {} ({name}) has {found} components; the spec declares {declared}",
                ElementRef {
                    segment_id,
                    element: *element,
                    component: None
                }
            ),
        }
    }
}

/// One finding about the data of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What failed.
    pub rule: Rule,
    /// The SNIP level of `rule`.
    pub level: SnipLevel,
    /// Index of the segment at fault; `None` when the finding is about the
    /// end of the stream.
    pub segment: Option<usize>,
    /// 1-based element position inside the segment, when the finding is.
    pub element: Option<usize>,
    /// 1-based component position inside the element, when the finding is.
    pub component: Option<usize>,
    /// Open loops at the time, outermost first.
    pub path: Vec<LoopRef>,
    /// The offending value as it appears in the file.
    pub datum: Vec<u8>,
}

impl Diagnostic {
    /// A diagnostic whose level is the rule's own.
    pub fn new(
        rule: Rule,
        segment: Option<usize>,
        element: Option<usize>,
        component: Option<usize>,
        path: Vec<LoopRef>,
        datum: Vec<u8>,
    ) -> Diagnostic {
        Diagnostic {
            level: rule.level(),
            rule,
            segment,
            element,
            component,
            path,
            datum,
        }
    }

    /// Where the segment at fault lives in `document`; `None` when the
    /// diagnostic names no segment or the document has no such segment.
    pub fn span(&self, document: &Document<'_>) -> Option<Span> {
        self.segment.and_then(|index| document.span(index))
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} · {} · ", self.level, self.rule)?;
        match self.segment {
            Some(segment) => write!(f, "segment #{segment}")?,
            None => write!(f, "end of stream")?,
        }
        if let Some(element) = self.element {
            write!(f, ", element {element}")?;
            if let Some(component) = self.component {
                write!(f, ", component {component}")?;
            }
        }
        write!(f, " · at ")?;
        if self.path.is_empty() {
            write!(f, "the root")?;
        }
        for (i, open) in self.path.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{open}")?;
        }
        write!(f, " · datum {}", Quoted(&self.datum))
    }
}

#[cfg(test)]
mod tests;
