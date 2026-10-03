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
            Rule::UnknownSegment { .. }
            | Rule::ImplicitLoop { .. }
            | Rule::UnterminatedLoop { .. }
            | Rule::ControlCountMismatch { .. }
            | Rule::ControlElementMissing { .. }
            | Rule::ControlNumberMismatch { .. } => SnipLevel::L1,
            Rule::RequiredElementMissing { .. }
            | Rule::TypeMismatch { .. }
            | Rule::LengthOutOfRange { .. }
            | Rule::CompositeShape { .. } => SnipLevel::L2,
        }
    }
}

/// Bytes from the file, shown as text (invalid UTF-8 replaced) and quoted.
struct Quoted<'a>(&'a [u8]);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", String::from_utf8_lossy(self.0))
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
        self.segment
            .and_then(|index| document.spans().get(index))
            .cloned()
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
mod tests {
    use super::*;
    use crate::Delimiters;

    fn path(loops: &[(&str, usize)]) -> Vec<LoopRef> {
        loops
            .iter()
            .map(|&(name, ordinal)| LoopRef {
                name: name.to_string(),
                ordinal,
            })
            .collect()
    }

    const TRANSACTION: &[(&str, usize)] = &[("interchange", 1), ("group", 1), ("transaction", 1)];

    #[test]
    fn levels_display_as_snip_numbers() {
        assert_eq!(SnipLevel::L1.to_string(), "SNIP 1");
        assert_eq!(SnipLevel::L2.to_string(), "SNIP 2");
        assert_eq!(SnipLevel::L3.to_string(), "SNIP 3");
    }

    #[test]
    fn a_loop_ref_displays_name_and_ordinal() {
        let at = LoopRef {
            name: "2100".into(),
            ordinal: 3,
        };
        assert_eq!(at.to_string(), "2100#3");
    }

    #[test]
    fn a_loop_ref_quotes_a_name_that_holds_a_separator() {
        let at = LoopRef {
            name: "x/y".into(),
            ordinal: 2,
        };
        assert_eq!(at.to_string(), "\"x/y\"#2");
        let at = LoopRef {
            name: "x#2".into(),
            ordinal: 1,
        };
        assert_eq!(at.to_string(), "\"x#2\"#1");
    }

    #[test]
    fn unknown_segment_displays_id_index_path_and_datum() {
        let diagnostic = Diagnostic::new(
            Rule::UnknownSegment { id: b"XX".to_vec() },
            Some(7),
            None,
            None,
            path(&[
                ("interchange", 1),
                ("group", 1),
                ("transaction", 1),
                ("1000A", 1),
            ]),
            b"XX".to_vec(),
        );
        assert_eq!(diagnostic.level, SnipLevel::L1);
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · segment \"XX\" is not part of the structure: no open loop holds it and it opens no loop · segment #7 · at interchange#1/group#1/transaction#1/1000A#1 · datum \"XX\""
        );
    }

    #[test]
    fn implicit_loop_displays_the_loop_and_the_segment_that_needed_it() {
        let diagnostic = Diagnostic::new(
            Rule::ImplicitLoop {
                loop_name: "group".into(),
                expected_trigger: "\"GS\" with no conditions".into(),
                caused_by: b"ST".to_vec(),
            },
            Some(0),
            None,
            None,
            path(&[("interchange", 1), ("group", 1)]),
            b"ST".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\""
        );
    }

    #[test]
    fn unterminated_loop_displays_the_opener_the_expected_end_and_the_closing_segment() {
        let diagnostic = Diagnostic::new(
            Rule::UnterminatedLoop {
                loop_name: "transaction".into(),
                expected_end: b"SE".to_vec(),
                opened_at: Some(2),
            },
            Some(4),
            None,
            None,
            path(TRANSACTION),
            b"GE".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
        );
        let implicit = Rule::UnterminatedLoop {
            loop_name: "transaction".into(),
            expected_end: b"SE".to_vec(),
            opened_at: None,
        };
        assert_eq!(
            implicit.to_string(),
            "loop \"transaction\" closed without its end segment \"SE\""
        );
    }

    #[test]
    fn a_finding_at_the_end_of_the_stream_says_so() {
        let diagnostic = Diagnostic::new(
            Rule::UnterminatedLoop {
                loop_name: "interchange".into(),
                expected_end: b"IEA".to_vec(),
                opened_at: Some(0),
            },
            None,
            None,
            None,
            path(&[("interchange", 1)]),
            Vec::new(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · loop \"interchange\" opened at segment #0 closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\""
        );
    }

    #[test]
    fn control_count_mismatch_displays_the_element_the_value_and_the_count() {
        let diagnostic = Diagnostic::new(
            Rule::ControlCountMismatch {
                segment_id: b"SE".to_vec(),
                element: 1,
                expected: 18,
                found: b"15".to_vec(),
            },
            Some(19),
            Some(1),
            None,
            path(TRANSACTION),
            b"15".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · SE01 declares \"15\" but the count is 18 · segment #19, element 1 · at interchange#1/group#1/transaction#1 · datum \"15\""
        );
    }

    #[test]
    fn control_element_missing_displays_the_segment_and_the_position() {
        let diagnostic = Diagnostic::new(
            Rule::ControlElementMissing {
                segment_id: b"SE".to_vec(),
                element: 2,
            },
            Some(4),
            Some(2),
            None,
            path(TRANSACTION),
            Vec::new(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · control element SE02 is missing: the segment has no element 2 · segment #4, element 2 · at interchange#1/group#1/transaction#1 · datum \"\""
        );
    }

    #[test]
    fn control_number_mismatch_displays_both_elements_values_and_the_opener() {
        let diagnostic = Diagnostic::new(
            Rule::ControlNumberMismatch {
                opener: b"ST".to_vec(),
                opener_element: 2,
                closer: b"SE".to_vec(),
                closer_element: 2,
                opener_value: b"0001".to_vec(),
                closer_value: b"0002".to_vec(),
                opened_at: Some(2),
            },
            Some(4),
            Some(2),
            None,
            path(TRANSACTION),
            b"0002".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" of segment #2 · segment #4, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\""
        );
        let implicit = Rule::ControlNumberMismatch {
            opener: b"ST".to_vec(),
            opener_element: 2,
            closer: b"SE".to_vec(),
            closer_element: 2,
            opener_value: b"0001".to_vec(),
            closer_value: b"0002".to_vec(),
            opened_at: None,
        };
        assert_eq!(
            implicit.to_string(),
            "SE02 \"0002\" does not match ST02 \"0001\""
        );
    }

    #[test]
    fn required_element_missing_displays_the_element_and_its_name() {
        let diagnostic = Diagnostic::new(
            Rule::RequiredElementMissing {
                segment_id: b"CLP".to_vec(),
                element: 1,
                component: None,
                name: "claim_submitter_identifier".into(),
            },
            Some(12),
            Some(1),
            None,
            path(&[("transaction", 1), ("2000", 1), ("2100", 1)]),
            Vec::new(),
        );
        assert_eq!(
            diagnostic.level.to_string(),
            "SNIP 2",
            "element rules are level 2"
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · required element CLP01 (claim_submitter_identifier) is missing or empty · segment #12, element 1 · at transaction#1/2000#1/2100#1 · datum \"\""
        );
    }

    #[test]
    fn type_mismatch_displays_the_component_and_the_declared_type() {
        let diagnostic = Diagnostic::new(
            Rule::TypeMismatch {
                segment_id: b"CLP".to_vec(),
                element: 3,
                component: None,
                name: "total_claim_charge_amount".into(),
                expected: ElementType::R { scale: 2 },
            },
            Some(12),
            Some(3),
            None,
            path(&[("2100", 2)]),
            b"12A".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · element CLP03 (total_claim_charge_amount) is not a valid R (decimal, scale 2) · segment #12, element 3 · at 2100#2 · datum \"12A\""
        );
        let component = Rule::TypeMismatch {
            segment_id: b"SVC".to_vec(),
            element: 1,
            component: Some(1),
            name: "product_or_service_id_qualifier".into(),
            expected: ElementType::Id,
        };
        assert_eq!(
            component.to_string(),
            "element SVC01-1 (product_or_service_id_qualifier) is not a valid ID (code)"
        );
    }

    #[test]
    fn length_out_of_range_displays_the_length_and_the_bounds() {
        let rule = |min, max| Rule::LengthOutOfRange {
            segment_id: b"CLP".to_vec(),
            element: 1,
            component: None,
            name: "claim_submitter_identifier".into(),
            min,
            max,
            length: 40,
        };
        let diagnostic = Diagnostic::new(
            rule(Some(1), Some(38)),
            Some(12),
            Some(1),
            None,
            Vec::new(),
            b"0123456789012345678901234567890123456789".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · element CLP01 (claim_submitter_identifier) has length 40; the spec allows 1 to 38 · segment #12, element 1 · at the root · datum \"0123456789012345678901234567890123456789\""
        );
        assert!(
            rule(Some(41), None)
                .to_string()
                .ends_with("allows at least 41")
        );
        assert!(
            rule(None, Some(38))
                .to_string()
                .ends_with("allows at most 38")
        );
        assert!(rule(None, None).to_string().ends_with("allows any length"));
    }

    #[test]
    fn composite_shape_displays_found_and_declared_components() {
        let diagnostic = Diagnostic::new(
            Rule::CompositeShape {
                segment_id: b"SVC".to_vec(),
                element: 1,
                name: "composite_medical_procedure".into(),
                declared: 8,
                found: 9,
            },
            Some(17),
            Some(1),
            Some(9),
            path(&[("2110", 1)]),
            b"X".to_vec(),
        );
        assert_eq!(
            diagnostic.to_string(),
            "SNIP 2 · element SVC01 (composite_medical_procedure) has 9 components; the spec declares 8 · segment #17, element 1, component 9 · at 2110#1 · datum \"X\""
        );
    }

    #[test]
    fn invalid_utf8_in_a_datum_is_shown_with_replacement_characters() {
        let diagnostic = Diagnostic::new(
            Rule::UnknownSegment {
                id: vec![b'Z', 0xFF],
            },
            Some(1),
            None,
            None,
            Vec::new(),
            vec![b'Z', 0xFF],
        );
        assert!(
            diagnostic
                .to_string()
                .ends_with("at the root · datum \"Z\u{FFFD}\""),
            "{diagnostic}"
        );
    }

    #[test]
    fn span_resolves_the_segment_bytes_from_the_document() {
        let document =
            Document::with_delimiters(&b"AA*1~BB*2~"[..], Delimiters::new(b'*', b':', b'~'));
        let at = |segment| {
            Diagnostic::new(
                Rule::UnknownSegment { id: b"BB".to_vec() },
                segment,
                None,
                None,
                Vec::new(),
                b"BB".to_vec(),
            )
        };
        let span = at(Some(1)).span(&document).unwrap();
        assert_eq!(&document.as_bytes()[span.raw], b"BB*2~");
        assert_eq!(at(Some(9)).span(&document), None);
        assert_eq!(at(None).span(&document), None);
    }
}
