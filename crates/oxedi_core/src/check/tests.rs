use super::*;
use crate::{Delimiters, LoopEngine, Tokenizer};

const ISA: &str = "ISA*00*          *00*          *ZZ*SENDER         *ZZ*RECEIVER       *240101*1200*^*00501*000000001*0*P*>~";

/// Runs the engine and the checker over `input` (with `*`, `:` and `~`)
/// and returns every diagnostic, `finish` included.
fn check(spec: &Spec, input: &str) -> Vec<Diagnostic> {
    let mut engine = LoopEngine::new(spec);
    let delims = Delimiters::new(b'*', b':', b'~');
    let mut checker = EnvelopeChecker::new(spec, &delims);
    let mut out = Vec::new();
    for segment in Tokenizer::with_delimiters(input.as_bytes(), delims) {
        let events = engine.feed(&segment);
        out.extend_from_slice(checker.on(&segment, events));
    }
    engine.finish();
    out.extend_from_slice(checker.finish());
    out
}

fn rendered(spec: &Spec, input: &str) -> Vec<String> {
    check(spec, input).iter().map(ToString::to_string).collect()
}

/// A complete interchange around `body`, which sits between `ST*835*0001~`
/// and the `SE`; `se01` is written as given.
fn interchange(body: &str, se01: &str) -> String {
    format!(
        "{ISA}GS*HP*SENDER*RECEIVER*20240101*1200*7*X*005010X221A1~ST*835*0001~{body}SE*{se01}*0001~GE*1*7~IEA*1*000000001~"
    )
}

#[test]
fn a_byte_order_mark_is_reported_once_and_changes_nothing_else() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~TRN*1*1~", "4");
    assert_eq!(
        rendered(&spec, &format!("\u{feff}{input}")),
        vec![
            "SNIP 1 · the input starts with a UTF-8 byte order mark, kept as leading trivia of the first segment · segment #0 · at the root · datum \"\\u{feff}\""
        ]
    );
    let broken = interchange("BPR*I*1*C*CHK~", "9");
    let mut marked = check(&spec, &format!("\u{feff}\r\n{broken}"));
    let mark = marked.remove(0);
    assert_eq!(mark.rule, Rule::ByteOrderMark);
    assert_eq!(marked, check(&spec, &format!("\r\n{broken}")));
    assert!(!marked.is_empty());
}

#[test]
fn a_well_formed_interchange_yields_nothing() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~TRN*1*1~", "4");
    assert_eq!(check(&spec, &input), Vec::new());
}

#[test]
fn an_unknown_segment_names_its_id_index_and_path() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~ZZZ*1~", "4");
    let diagnostics = check(&spec, &input);
    assert_eq!(
        diagnostics,
        vec![Diagnostic::new(
            Rule::UnknownSegment {
                id: b"ZZZ".to_vec()
            },
            Some(4),
            None,
            None,
            vec![
                LoopRef {
                    name: "interchange".into(),
                    ordinal: 1
                },
                LoopRef {
                    name: "group".into(),
                    ordinal: 1
                },
                LoopRef {
                    name: "transaction".into(),
                    ordinal: 1
                },
            ],
            b"ZZZ".to_vec(),
        )]
    );
    assert_eq!(
        diagnostics[0].to_string(),
        "SNIP 1 · segment \"ZZZ\" is not part of the structure: no open loop holds it and it opens no loop · segment #4 · at interchange#1/group#1/transaction#1 · datum \"ZZZ\""
    );
}

#[test]
fn implicit_loops_name_the_segment_that_needed_them_and_never_their_missing_end() {
    let spec = Spec::builtin_835();
    assert_eq!(
        rendered(&spec, "ST*835*0001~BPR*I*1*C*CHK~SE*3*0001~"),
        vec![
            "SNIP 1 · loop \"interchange\" opened without its own trigger (\"ISA\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1 · datum \"ST\"",
            "SNIP 1 · loop \"group\" opened without its own trigger (\"GS\" with no conditions) to hold segment \"ST\" · segment #0 · at interchange#1/group#1 · datum \"ST\"",
        ]
    );
}

#[test]
fn a_wrong_segment_count_names_the_count_element() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~", "5");
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · SE01 declares \"5\" but the count is 3 · segment #4, element 1 · at interchange#1/group#1/transaction#1 · datum \"5\""
        ]
    );
}

#[test]
fn a_count_that_is_not_a_number_is_a_mismatch_with_the_text_as_datum() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~", "3X");
    let diagnostics = check(&spec, &input);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].rule,
        Rule::ControlCountMismatch {
            segment_id: b"SE".to_vec(),
            element: 1,
            expected: 3,
            found: b"3X".to_vec(),
        }
    );
    assert_eq!(diagnostics[0].datum, b"3X");
}

#[test]
fn a_composite_count_is_reported_as_the_whole_text() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~BPR*I*1*C*CHK~SE*1:2*0001~GE*1*7~IEA*1*000000001~"
    );
    let diagnostics = check(&spec, &input);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].to_string(),
        "SNIP 1 · SE01 declares \"1:2\" but the count is 3 · segment #4, element 1 · at interchange#1/group#1/transaction#1 · datum \"1:2\""
    );
    assert_eq!(diagnostics[0].datum, b"1:2");
}

#[test]
fn composite_control_numbers_are_compared_as_text() {
    let spec = Spec::builtin_835();
    let build = |closer: &str| {
        format!(
            "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*1:2~SE*2*{closer}~GE*1*7~IEA*1*000000001~"
        )
    };
    assert_eq!(check(&spec, &build("1:2")), Vec::new());
    assert_eq!(
        rendered(&spec, &build("1:3")),
        vec![
            "SNIP 1 · SE02 \"1:3\" does not match ST02 \"1:2\" of segment #2 · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum \"1:3\""
        ]
    );
}

#[test]
fn a_segment_without_its_count_and_number_reports_each_as_missing() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE~GE*1*7~IEA*1*000000001~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · control element SE01 is missing: the segment has no element 1 · segment #3, element 1 · at interchange#1/group#1/transaction#1 · datum \"\"",
            "SNIP 1 · control element SE02 is missing: the segment has no element 2 · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum \"\"",
        ]
    );
}

#[test]
fn a_missing_closer_number_is_reported_as_missing() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2~GE*1*7~IEA*1*000000001~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · control element SE02 is missing: the segment has no element 2 · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum \"\"",
        ]
    );
}

#[test]
fn a_missing_opener_number_is_reported_once_at_the_opener() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835~SE*2*0001~GE*1*7~IEA*1*000000001~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · control element ST02 is missing: the segment has no element 2 · segment #2, element 2 · at interchange#1/group#1/transaction#1 · datum \"\"",
        ]
    );
}

#[test]
fn an_end_segment_at_the_root_checks_nothing() {
    let spec = Spec::builtin_835();
    let found = rendered(&spec, "SE*9*0001~");
    assert_eq!(
        found,
        vec![
            "SNIP 1 · segment \"SE\" is not part of the structure: no open loop holds it and it opens no loop · segment #0 · at the root · datum \"SE\""
        ]
    );
}

#[test]
fn leading_zeros_in_a_count_are_accepted() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~", "0003");
    assert_eq!(check(&spec, &input), Vec::new());
}

#[test]
fn a_control_number_that_differs_from_the_opener_is_reported() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2*0002~GE*1*8~IEA*1*000000002~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · SE02 \"0002\" does not match ST02 \"0001\" of segment #2 · segment #3, element 2 · at interchange#1/group#1/transaction#1 · datum \"0002\"",
            "SNIP 1 · GE02 \"8\" does not match GS06 \"7\" of segment #1 · segment #4, element 2 · at interchange#1/group#1 · datum \"8\"",
            "SNIP 1 · IEA02 \"000000002\" does not match ISA13 \"000000001\" of segment #0 · segment #5, element 2 · at interchange#1 · datum \"000000002\"",
        ]
    );
}

#[test]
fn group_and_interchange_counts_count_their_children() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2*0001~ST*835*0002~SE*2*0002~GE*1*7~IEA*2*000000001~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · GE01 declares \"1\" but the count is 2 · segment #6, element 1 · at interchange#1/group#1 · datum \"1\"",
            "SNIP 1 · IEA01 declares \"2\" but the count is 1 · segment #7, element 1 · at interchange#1 · datum \"2\"",
        ]
    );
}

#[test]
fn instances_are_numbered_in_stream_order() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~SE*2*0001~ST*835*0002~ZZZ~SE*3*0002~GE*2*7~IEA*1*000000001~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · segment \"ZZZ\" is not part of the structure: no open loop holds it and it opens no loop · segment #5 · at interchange#1/group#1/transaction#2 · datum \"ZZZ\""
        ]
    );
}

#[test]
fn a_loop_closed_by_an_outer_end_segment_is_unterminated() {
    let spec = Spec::builtin_835();
    let input = format!(
        "{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~BPR*I*1*C*CHK~GE*1*7~IEA*1*000000001~"
    );
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · segment #4 · at interchange#1/group#1/transaction#1 · datum \"GE\""
        ]
    );
}

#[test]
fn loops_still_open_at_the_end_of_the_stream_are_unterminated() {
    let spec = Spec::builtin_835();
    let input = format!("{ISA}GS*HP*S*R*20240101*1200*7*X*005010X221A1~ST*835*0001~");
    assert_eq!(
        rendered(&spec, &input),
        vec![
            "SNIP 1 · loop \"transaction\" opened at segment #2 closed without its end segment \"SE\" · end of stream · at interchange#1/group#1/transaction#1 · datum \"\"",
            "SNIP 1 · loop \"group\" opened at segment #1 closed without its end segment \"GE\" · end of stream · at interchange#1/group#1 · datum \"\"",
            "SNIP 1 · loop \"interchange\" opened at segment #0 closed without its end segment \"IEA\" · end of stream · at interchange#1 · datum \"\"",
        ]
    );
}

#[test]
fn envelope_rules_come_from_the_spec() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{
            "batch":{"trigger":{"segment":"HDR"},"segments":["LN"],"end":"TRL",
                "control":{"opener_element":1,"closer_element":2,"count_element":1,"count":"segments"}}
        }}"#,
    )
    .unwrap();
    assert_eq!(check(&spec, "HDR*A1~LN*x~LN*y~TRL*4*A1~"), Vec::new());
    assert_eq!(
        rendered(&spec, "HDR*A1~LN*x~TRL*9*B2~"),
        vec![
            "SNIP 1 · TRL01 declares \"9\" but the count is 3 · segment #2, element 1 · at batch#1 · datum \"9\"",
            "SNIP 1 · TRL02 \"B2\" does not match HDR01 \"A1\" of segment #0 · segment #2, element 2 · at batch#1 · datum \"B2\"",
        ]
    );
}

#[test]
fn a_loop_without_control_only_checks_that_its_end_arrives() {
    let spec = Spec::from_json(
        r#"{"name":"t","loops":{"batch":{"trigger":{"segment":"HDR"},"end":"TRL"}}}"#,
    )
    .unwrap();
    assert_eq!(check(&spec, "HDR*1~TRL*whatever~"), Vec::new());
    assert_eq!(
        rendered(&spec, "HDR*1~"),
        vec![
            "SNIP 1 · loop \"batch\" opened at segment #0 closed without its end segment \"TRL\" · end of stream · at batch#1 · datum \"\""
        ]
    );
}

#[test]
fn empty_segments_are_not_counted() {
    let spec = Spec::builtin_835();
    let input = interchange("BPR*I*1*C*CHK~~", "3");
    assert_eq!(check(&spec, &input), Vec::new());
}

#[test]
fn finishing_resets_the_checker() {
    let spec = Spec::builtin_835();
    let input = interchange("ZZZ~", "3");
    let first = check(&spec, &input);
    let mut engine = LoopEngine::new(&spec);
    let delims = Delimiters::new(b'*', b':', b'~');
    let mut checker = EnvelopeChecker::new(&spec, &delims);
    let run = |engine: &mut LoopEngine<'_>, checker: &mut EnvelopeChecker<'_>| {
        let mut out = Vec::new();
        for segment in Tokenizer::with_delimiters(input.as_bytes(), delims) {
            let events = engine.feed(&segment);
            out.extend_from_slice(checker.on(&segment, events));
        }
        engine.finish();
        out.extend_from_slice(checker.finish());
        out
    };
    assert_eq!(run(&mut engine, &mut checker), first);
    assert_eq!(
        run(&mut engine, &mut checker),
        first,
        "ordinals restart at 1"
    );
}

#[test]
fn counts_parse_digits_only() {
    assert_eq!(parse_count(b"0042"), Some(42));
    assert_eq!(parse_count(b""), None);
    assert_eq!(parse_count(b"4 "), None);
    assert_eq!(parse_count(b"-4"), None);
    assert_eq!(parse_count(b"99999999999999999999999"), None);
}
