//! The caller's fields an envelope loop's trigger carries.

use crate::spec::{EnvelopeField, EnvelopeFields};
use crate::vocabulary::Vocabulary;

#[test]
fn envelope_field_words_are_the_envelope_field_names() {
    assert_eq!(
        EnvelopeField::ALL.map(EnvelopeField::word),
        [
            "sender_qualifier",
            "sender_id",
            "receiver_qualifier",
            "receiver_id",
            "application_sender",
            "application_receiver",
            "date",
            "time",
            "usage_indicator",
        ]
    );
    for field in EnvelopeField::ALL {
        assert_eq!(EnvelopeField::from_word(field.word()), Some(field));
    }
}

#[test]
fn envelope_fields_find_a_field_by_element_and_print_as_a_map() {
    let fields = EnvelopeFields::default()
        .place(9, EnvelopeField::Date)
        .and_then(|fields| fields.place(5, EnvelopeField::SenderId))
        .unwrap();
    assert_eq!(fields.at(9), Some(EnvelopeField::Date));
    assert_eq!(fields.at(6), None);
    assert_eq!(fields.element(EnvelopeField::SenderId), Some(5));
    assert_eq!(
        fields.by_element(),
        [(5, EnvelopeField::SenderId), (9, EnvelopeField::Date)]
    );
    assert_eq!(format!("{fields:?}"), "{5: SenderId, 9: Date}");
    assert_eq!(fields.place(10, EnvelopeField::Date), Err(9));
}
