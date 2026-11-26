//! The caller's fields an envelope loop's trigger carries.

use std::fmt;

/// A caller's field an envelope loop's trigger carries, by the `Envelope`
/// field it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EnvelopeField {
    /// The sender's qualifier.
    SenderQualifier,
    /// The sender's id.
    SenderId,
    /// The receiver's qualifier.
    ReceiverQualifier,
    /// The receiver's id.
    ReceiverId,
    /// The sending application's code.
    ApplicationSender,
    /// The receiving application's code.
    ApplicationReceiver,
    /// The date the envelope is written.
    Date,
    /// The time the envelope is written.
    Time,
    /// The usage indicator (production or test).
    UsageIndicator,
}

impl EnvelopeField {
    /// Every field, in the order a spec lists them.
    pub const ALL: [EnvelopeField; 9] = [
        EnvelopeField::SenderQualifier,
        EnvelopeField::SenderId,
        EnvelopeField::ReceiverQualifier,
        EnvelopeField::ReceiverId,
        EnvelopeField::ApplicationSender,
        EnvelopeField::ApplicationReceiver,
        EnvelopeField::Date,
        EnvelopeField::Time,
        EnvelopeField::UsageIndicator,
    ];

    /// The word a spec writes for the field, also the name a trace or a
    /// finding prints.
    pub fn word(self) -> &'static str {
        match self {
            EnvelopeField::SenderQualifier => "sender_qualifier",
            EnvelopeField::SenderId => "sender_id",
            EnvelopeField::ReceiverQualifier => "receiver_qualifier",
            EnvelopeField::ReceiverId => "receiver_id",
            EnvelopeField::ApplicationSender => "application_sender",
            EnvelopeField::ApplicationReceiver => "application_receiver",
            EnvelopeField::Date => "date",
            EnvelopeField::Time => "time",
            EnvelopeField::UsageIndicator => "usage_indicator",
        }
    }
}

/// The element of a trigger each caller's field sits on, one slot per field
/// (indexed by the field), so the set stays `Copy`.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvelopeFields {
    /// 1-based element position of each field, by `EnvelopeField as usize`.
    elements: [Option<usize>; 9],
}

impl EnvelopeFields {
    /// The 1-based element the field sits on, when the loop places it.
    pub fn element(self, field: EnvelopeField) -> Option<usize> {
        self.elements.get(field as usize).copied().flatten()
    }

    /// The field that sits on the 1-based element, when one does.
    pub fn at(self, element: usize) -> Option<EnvelopeField> {
        EnvelopeField::ALL
            .into_iter()
            .find(|field| self.element(*field) == Some(element))
    }

    /// Every placed field with its 1-based element, by ascending element.
    pub fn by_element(self) -> Vec<(usize, EnvelopeField)> {
        let mut placed: Vec<(usize, EnvelopeField)> = EnvelopeField::ALL
            .into_iter()
            .filter_map(|field| self.element(field).map(|element| (element, field)))
            .collect();
        placed.sort_unstable();
        placed
    }

    /// The set with `field` on the 1-based `element`; `Err` holds the element
    /// the field already sits on.
    pub(crate) fn place(
        mut self,
        element: usize,
        field: EnvelopeField,
    ) -> Result<EnvelopeFields, usize> {
        // The slot index is the field's own discriminant, below the array length.
        match self.elements[field as usize] {
            Some(first) => Err(first),
            None => {
                self.elements[field as usize] = Some(element);
                Ok(self)
            }
        }
    }
}

// Prints the map from element to field, as the compiled spec lists it.
impl fmt::Debug for EnvelopeFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.by_element()).finish()
    }
}
