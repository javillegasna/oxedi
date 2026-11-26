use crate::api::mask_diagnostics;
use crate::diagnostic::{Diagnostic, Rule, SnipLevel};
use crate::document::Document;
use crate::mask::Mask;
use crate::process::Processor;
use crate::project::Projector;
use crate::spec_load::Builtin;
use crate::vocabulary::DateMode;

const FILE: &[u8] = include_bytes!("../../../tests/fixtures/masked_findings_5010_sample.txt");

fn mask() -> Mask {
    Mask::new(&[7u8; 32], DateMode::Year).unwrap()
}

#[test]
fn without_own_text_it_masks_as_the_projector() {
    let document = Document::parse(FILE).unwrap();
    let spec = &Builtin::fallback().spec;
    let (_, found) = Processor::run(spec, &document, None);
    let mask = mask();
    let mut expected = found.clone();
    Projector::new(spec, document.delimiters())
        .with_mask(&mask)
        .mask_found(&mut expected, &document);
    let mut masked = found;
    mask_diagnostics(&mut masked, &document, spec, &mask, &[]);
    assert_eq!(masked, expected);
}

#[test]
fn an_own_message_is_kept_and_its_datum_masked() {
    let document = Document::parse(FILE).unwrap();
    let spec = &Builtin::fallback().spec;
    let at = |raw: &[u8]| raw.windows(17).any(|w| w == b"NM1*QC*1*DOE*JANE");
    let segment = (0..document.len())
        .find(|&i| document.raw(i).is_some_and(at))
        .unwrap();
    let message = "the first name JANE is not accepted".to_string();
    let rule = Rule::External {
        origin: "tool".into(),
        code: None,
        message: message.clone(),
        level: SnipLevel::L1,
    };
    let found = Diagnostic::new(rule, Some(segment), Some(4), None, vec![], b"JANE".to_vec());
    let message_of = |d: &Diagnostic| match &d.rule {
        Rule::External { message, .. } => message.clone(),
        _ => String::new(),
    };
    let mut own = vec![found.clone()];
    mask_diagnostics(&mut own, &document, spec, &mask(), &[true]);
    assert_eq!(message_of(&own[0]), message);
    assert_ne!(own[0].datum, b"JANE");
    let mut foreign = vec![found];
    mask_diagnostics(&mut foreign, &document, spec, &mask(), &[false]);
    assert_ne!(message_of(&foreign[0]), message);
}
