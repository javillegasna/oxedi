//! The envelope a caller gives: who sends and receives, when, the first
//! control number and the delimiters; and the values of the envelope
//! segments the writer derives from it and from the spec.
//!
//! The envelope loops are the spec's loops with a `control`, outermost
//! first: an interchange, a functional group and a transaction set, the X12
//! envelope. Each element of an envelope trigger takes its value from the
//! first of the sources `Header::source` lists that applies. The codes and
//! widths come from the element's rule at the occurrence the loop's trigger
//! opens on. An element whose rule allows a fixed width only is padded to it:
//! numbers with leading zeros, text with trailing spaces.

use std::borrow::Cow;

use crate::delimiters::{Delimiters, HeaderPositions};
use crate::spec::{
    ElementDef, ElementRule, EnvelopeField, EnvelopeFields, LoopDef, OccurrenceDef, Spec,
};
use crate::value::{ElementTextError, ElementType};

use super::checked::GroupCode;
use super::finding::WriteError;
use super::render::{envelope_date, envelope_time};

/// The caller's part of the envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// The sender id's qualifier, e.g. `ZZ`.
    pub sender_qualifier: String,
    /// The sender id; padded to the interchange's fixed width.
    pub sender_id: String,
    /// The receiver id's qualifier.
    pub receiver_qualifier: String,
    /// The receiver id; padded to the interchange's fixed width.
    pub receiver_id: String,
    /// The functional group's sender code.
    pub application_sender: GroupCode,
    /// The functional group's receiver code.
    pub application_receiver: GroupCode,
    /// The date, as days since 1970-01-01.
    pub date: i32,
    /// The time, as seconds since midnight.
    pub time: i32,
    /// Whether the interchange is production or test data, e.g. `P` or `T`.
    pub usage_indicator: String,
    /// The interchange's and the group's control number, and the first
    /// transaction set's; each later transaction set takes the next one.
    pub control_number: u64,
    /// The delimiters to write with. The repetition separator is written
    /// where the interchange version (ISA12) is 00402 or later, which the reader
    /// takes as carrying one, and is ignored below it. With a release byte, a
    /// value holding the element separator, the component separator, the
    /// segment terminator or the release byte is written escaped with it;
    /// without one, such a value is a finding. The interchange header is the
    /// exception: its elements are fixed width and the reader takes the
    /// delimiters from fixed offsets, so a header value holding a delimiter is
    /// a finding and written as it is. A value holding the repetition
    /// separator is always a finding: the spec does not say which elements
    /// repeat, so the writer cannot tell a repeat from data.
    pub delimiters: Delimiters,
    /// Whether a line break follows each segment terminator.
    pub line_break: bool,
}

impl Envelope {
    /// The qualifier of a sender or receiver id when the caller gives none.
    pub const DEFAULT_QUALIFIER: &'static str = "ZZ";
    /// The usage indicator when the caller gives none: production data.
    pub const DEFAULT_USAGE_INDICATOR: &'static str = "P";
    /// The first control number when the caller gives none.
    pub const DEFAULT_CONTROL_NUMBER: u64 = 1;

    /// An envelope with both qualifiers `ZZ`, otherwise as [`Envelope::new`].
    pub fn with_ids(
        sender_id: impl Into<String>,
        receiver_id: impl Into<String>,
        date: i32,
        time: i32,
    ) -> Envelope {
        Envelope::new(
            Self::DEFAULT_QUALIFIER,
            sender_id,
            Self::DEFAULT_QUALIFIER,
            receiver_id,
            date,
            time,
        )
    }

    /// An envelope for production data with control number 1, the
    /// delimiters `*`, `:`, `~` and `^`, and no line breaks.
    pub fn new(
        sender_qualifier: impl Into<String>,
        sender_id: impl Into<String>,
        receiver_qualifier: impl Into<String>,
        receiver_id: impl Into<String>,
        date: i32,
        time: i32,
    ) -> Envelope {
        Envelope {
            sender_qualifier: sender_qualifier.into(),
            sender_id: sender_id.into(),
            receiver_qualifier: receiver_qualifier.into(),
            receiver_id: receiver_id.into(),
            application_sender: GroupCode::InterchangeId,
            application_receiver: GroupCode::InterchangeId,
            date,
            time,
            usage_indicator: Self::DEFAULT_USAGE_INDICATOR.to_string(),
            control_number: Self::DEFAULT_CONTROL_NUMBER,
            delimiters: Delimiters::new(b'*', b':', b'~').with_repetition(b'^'),
            line_break: false,
        }
    }
}

/// What an envelope element's value comes from, by the name a finding gives it
/// (`write::EnvelopeSource` outside the module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// A caller's field the loop's `control.fields` puts on the element.
    Caller(EnvelopeField),
    /// The interchange control number.
    ControlNumber,
    /// The repetition separator.
    Repetition,
    /// The component separator.
    Component,
    /// The interchange version.
    Version,
}

impl Field {
    pub fn name(self) -> &'static str {
        match self {
            Field::Caller(field) => field.word(),
            Field::ControlNumber => "control_number",
            Field::Repetition => "delimiters.repetition",
            Field::Component => "delimiters.component",
            Field::Version => "version",
        }
    }
}

/// The delimiter places of the interchange header, which the syntax layer owns
/// and the top envelope loop's trigger holds; a caller's field never takes
/// them.
const HEADER_DELIMITERS: [(usize, Field); 2] = [
    (HeaderPositions::ISA.repetition, Field::Repetition),
    (HeaderPositions::ISA.component, Field::Component),
];

/// The role of each delimiter, as messages name it.
pub(super) const ELEMENT: &str = "element separator";
pub(super) const COMPONENT: &str = "component separator";
pub(super) const SEGMENT: &str = "segment terminator";
pub(super) const REPETITION: &str = "repetition separator";
pub(super) const RELEASE: &str = "release character";

/// One value of an envelope trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EnvelopeValue {
    pub(super) element: usize,
    pub(super) bytes: Vec<u8>,
    /// The caller's field it comes from, when one does.
    pub(super) field: Option<Field>,
    /// `false` for a delimiter written as the value itself.
    pub(super) checked: bool,
    /// Why the field has no text the element can hold, when it has none.
    pub(super) refused: Option<ElementTextError>,
}

/// The delimiters the written file uses: those of the envelope, with the
/// repetition separator only when the header carries one, which `carrying`
/// says by the interchange version that makes it so.
pub(super) fn delimiters(
    envelope: &Envelope,
    carrying: Option<&[u8]>,
) -> Result<Vec<(u8, &'static str)>, WriteError> {
    let given = &envelope.delimiters;
    let mut all = vec![
        (given.element, ELEMENT),
        (given.component, COMPONENT),
        (given.segment, SEGMENT),
    ];
    if let Some(version) = carrying {
        let Some(byte) = given.repetition else {
            return Err(WriteError::NoRepetition {
                version: version.to_vec(),
            });
        };
        all.push((byte, REPETITION));
    }
    if let Some(byte) = given.release {
        all.push((byte, RELEASE));
    }
    for (i, &(byte, role)) in all.iter().enumerate() {
        if byte.is_ascii_alphanumeric() || byte.is_ascii_whitespace() {
            return Err(WriteError::DelimiterNotAllowed { role, byte });
        }
        if let Some(&(_, first)) = all.iter().take(i).find(|(other, _)| *other == byte) {
            return Err(WriteError::SameDelimiter {
                first,
                second: role,
                byte,
            });
        }
    }
    Ok(all)
}

/// The bytes the writer puts in the interchange header's version element
/// (ISA12), found the way the header writes every element, or nothing when
/// the spec has no envelope loop or its header has no such element.
pub(super) fn interchange_version(spec: &Spec, envelope: &Envelope) -> Vec<u8> {
    let Some((root, control)) = spec
        .roots()
        .iter()
        .map(|&root| spec.get(root))
        .find_map(|def| def.control.as_ref().map(|control| (def, control)))
    else {
        return Vec::new();
    };
    let number = control_text(
        envelope.control_number,
        trigger_rule(spec, root, control.opener_element),
    );
    let header = Header::new(spec, root, &number, envelope);
    spec.element_def(&root.trigger.segment, HeaderPositions::ISA.version, None)
        .and_then(|def| header.value(HeaderPositions::ISA.version, def))
        .map(|value| value.bytes)
        .unwrap_or_default()
}

/// The envelope the writer emits with: the caller's, with no repetition
/// separator when the header carries none, so a delimiter the file cannot
/// hold is never written.
pub(super) fn for_spec(envelope: &Envelope, carries_repetition: bool) -> Cow<'_, Envelope> {
    if carries_repetition || envelope.delimiters.repetition.is_none() {
        return Cow::Borrowed(envelope);
    }
    let mut owned = envelope.clone();
    owned.delimiters.repetition = None;
    Cow::Owned(owned)
}

/// The rule of `element` of `def`'s trigger segment at the occurrence the
/// trigger opens on; `None` when the spec does not define the element.
pub(super) fn trigger_rule<'s>(
    spec: &'s Spec,
    def: &'s LoopDef,
    element: usize,
) -> Option<ElementRule<'s>> {
    let element_def = spec.element_def(&def.trigger.segment, element, None)?;
    Some(ElementRule::at(
        element_def,
        def.trigger_entry(),
        element,
        None,
    ))
}

/// The values of an envelope loop's trigger segment, with `control` the
/// instance's control number.
pub(super) fn trigger_values(
    spec: &Spec,
    def: &LoopDef,
    control: &[u8],
    envelope: &Envelope,
) -> Vec<EnvelopeValue> {
    let Some(segment) = spec.segment(&def.trigger.segment) else {
        return Vec::new();
    };
    let header = Header::new(spec, def, control, envelope);
    segment
        .elements
        .iter()
        .filter_map(|(&position, def)| header.value(position, def))
        .collect()
}

/// Where the value of one element of an envelope trigger comes from.
enum Source<'a> {
    /// The control number the loop's `control` places on the opener element.
    Control,
    /// The caller's field the loop's `control.fields`, or the header's
    /// delimiter places, put on that element.
    Field(Field),
    /// The spec's declared version, when it sits on that segment and element.
    Version(&'a [u8]),
    /// The first code the element's rule at the trigger occurrence allows.
    Code(&'a str),
    /// Nothing: the element is written empty.
    Nothing,
}

/// One envelope trigger being written: what decides the value of each of its
/// elements.
struct Header<'a> {
    spec: &'a Spec,
    trigger: &'a [u8],
    /// The occurrence the trigger opens on, whose entries narrow the rules.
    occurrence: Option<&'a OccurrenceDef>,
    /// The element that holds the control number, when the loop has a `control`.
    opener: Option<usize>,
    /// The caller's fields the loop's `control` places.
    fields: EnvelopeFields,
    /// Whether the trigger sits on a top loop and so holds the delimiter places.
    holds_delimiters: bool,
    control: &'a [u8],
    envelope: &'a Envelope,
}

impl<'a> Header<'a> {
    /// The header of `def`'s trigger.
    fn new(spec: &'a Spec, def: &'a LoopDef, control: &'a [u8], envelope: &'a Envelope) -> Self {
        Header {
            spec,
            trigger: &def.trigger.segment,
            occurrence: def.trigger_entry(),
            opener: def.control.map(|control| control.opener_element),
            fields: def
                .control
                .map(|control| control.fields)
                .unwrap_or_default(),
            holds_delimiters: def.parent.is_none(),
            control,
            envelope,
        }
    }

    /// The rule of `position` at the trigger occurrence.
    fn rule(&self, position: usize, def: &'a ElementDef) -> ElementRule<'a> {
        ElementRule::at(def, self.occurrence, position, None)
    }

    /// The source of the value of `position`, in order: the control number,
    /// the caller's field, the declared version, the first code its rule
    /// allows, nothing.
    fn source(&self, position: usize, rule: &ElementRule<'a>) -> Source<'a> {
        if self.opener == Some(position) {
            return Source::Control;
        }
        if let Some(field) = self.field(position) {
            return Source::Field(field);
        }
        if let Some(version) = self.declared_version(position) {
            return Source::Version(version);
        }
        rule.codes
            .first()
            .map_or(Source::Nothing, |code| Source::Code(code))
    }

    /// What the loop puts on `position`: a delimiter place of the header, else
    /// the caller's field its `control.fields` names.
    fn field(&self, position: usize) -> Option<Field> {
        let delimiter = HEADER_DELIMITERS
            .iter()
            .find(|(at, _)| self.holds_delimiters && *at == position);
        delimiter
            .map(|(_, field)| *field)
            .or_else(|| self.fields.at(position).map(Field::Caller))
    }

    /// The spec's declared version, when it sits on this segment and
    /// `position`.
    fn declared_version(&self, position: usize) -> Option<&'a [u8]> {
        let version = self.spec.version()?;
        let sits_here = version.segment == self.trigger && version.element == position;
        sits_here.then(|| version.values.first().map_or(&[][..], Vec::as_slice))
    }

    /// The value of `position`, padded to the width its rule allows, or
    /// `None` when its caller's field has nothing to write there.
    fn value(&self, position: usize, def: &'a ElementDef) -> Option<EnvelopeValue> {
        let rule = self.rule(position, def);
        let mut value = match self.source(position, &rule) {
            Source::Control => {
                EnvelopeValue::plain(position, self.control, Some(Field::ControlNumber))
            }
            Source::Field(field) => EnvelopeValue::of_field(position, field, &rule, self.envelope)?,
            Source::Version(bytes) => EnvelopeValue::plain(position, bytes, Some(Field::Version)),
            Source::Code(code) => EnvelopeValue::plain(position, code.as_bytes(), None),
            Source::Nothing => EnvelopeValue::plain(position, &[], None),
        };
        if value.refused.is_none() {
            value.bytes = pad(value.bytes, &rule);
        }
        Some(value)
    }
}

impl EnvelopeValue {
    /// A checked value with no refusal.
    fn plain(element: usize, bytes: &[u8], field: Option<Field>) -> EnvelopeValue {
        EnvelopeValue {
            element,
            bytes: bytes.to_vec(),
            field,
            checked: true,
            refused: None,
        }
    }

    /// The value of a caller's field, or `None` when it has nothing to write
    /// on the element; the field's refusal when it has no text the element
    /// can hold.
    fn of_field(
        element: usize,
        field: Field,
        rule: &ElementRule<'_>,
        envelope: &Envelope,
    ) -> Option<EnvelopeValue> {
        let plain = EnvelopeValue::plain(element, &[], Some(field));
        Some(match field_value(field, rule, envelope)? {
            Ok((bytes, checked)) => EnvelopeValue {
                bytes,
                checked,
                ..plain
            },
            Err(reason) => EnvelopeValue {
                refused: Some(reason),
                ..plain
            },
        })
    }
}

/// The value of a caller's field for an element, and whether it is checked
/// for delimiters; `None` when the field has nothing to write there, the
/// reason when the field has no text the element can hold.
fn field_value(
    field: Field,
    rule: &ElementRule<'_>,
    envelope: &Envelope,
) -> Option<Result<(Vec<u8>, bool), ElementTextError>> {
    let text = |value: &str| Some(Ok((value.as_bytes().to_vec(), true)));
    let delimiters = &envelope.delimiters;
    match field {
        Field::Caller(EnvelopeField::SenderQualifier) => text(&envelope.sender_qualifier),
        Field::Caller(EnvelopeField::SenderId) => text(&envelope.sender_id),
        Field::Caller(EnvelopeField::ReceiverQualifier) => text(&envelope.receiver_qualifier),
        Field::Caller(EnvelopeField::ReceiverId) => text(&envelope.receiver_id),
        Field::Caller(EnvelopeField::ApplicationSender) => {
            text(envelope.application_sender.text(&envelope.sender_id))
        }
        Field::Caller(EnvelopeField::ApplicationReceiver) => {
            text(envelope.application_receiver.text(&envelope.receiver_id))
        }
        Field::Caller(EnvelopeField::Date) => {
            Some(envelope_date(envelope.date, rule.length.max).map(|text| (text, true)))
        }
        Field::Caller(EnvelopeField::Time) => {
            Some(envelope_time(envelope.time, rule.length.max).map(|text| (text, true)))
        }
        Field::Caller(EnvelopeField::UsageIndicator) => text(&envelope.usage_indicator),
        Field::Repetition => match (delimiters.repetition, rule.codes.first()) {
            (Some(byte), _) => Some(Ok((vec![byte], false))),
            (None, Some(code)) => Some(Ok((code.as_bytes().to_vec(), true))),
            (None, None) => None,
        },
        Field::Component => Some(Ok((vec![delimiters.component], false))),
        Field::Version | Field::ControlNumber => None,
    }
}

/// Pads a value to the width its rule allows when the rule allows one width
/// only, or when the element is a number shorter than its minimum.
pub(super) fn pad(mut bytes: Vec<u8>, rule: &ElementRule<'_>) -> Vec<u8> {
    let Some(min) = rule.length.min else {
        return bytes;
    };
    let numeric = matches!(rule.def.kind, ElementType::N(_));
    let fixed = rule.length.max == Some(min);
    if bytes.len() >= min || !(numeric || fixed) {
        return bytes;
    }
    let missing = min - bytes.len();
    if numeric {
        let mut padded = vec![b'0'; missing];
        padded.append(&mut bytes);
        padded
    } else {
        bytes.resize(min, b' ');
        bytes
    }
}

/// A control number as its element writes it: digits padded with leading
/// zeros to the minimum its rule allows.
pub(super) fn control_text(number: u64, rule: Option<ElementRule<'_>>) -> Vec<u8> {
    let digits = number.to_string().into_bytes();
    let min = rule.and_then(|rule| rule.length.min).unwrap_or(0);
    if digits.len() >= min {
        return digits;
    }
    let mut padded = vec![b'0'; min - digits.len()];
    padded.extend_from_slice(&digits);
    padded
}
