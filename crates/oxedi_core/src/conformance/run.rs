//! Running a kit's cases over bytes the caller read.

use std::collections::BTreeMap;

use super::diff::first_difference;
use super::error::WrongStack;
use super::render::render;
use super::report::{CaseFault, CaseReport, Difference, KitReport, Outcome};
use crate::document::Document;
use crate::process::Processor;
use crate::spec::Spec;
use crate::spec_load::{Kit, KitCase, KitOutput};
use crate::validate::Valid;

/// Runs every case of `kit` with the spec of `stack`. `files` holds the bytes
/// of each path [`Kit::files`] lists, by the path as the kit writes it: the
/// core opens nothing. A case that cannot run is reported, never skipped, and
/// its first fault ends it: a case with a missing file reports that file only.
/// The report names the stack's top package.
///
/// Fails when the kit checks another package than the stack's top.
pub fn run_kit(
    kit: &Kit,
    stack: &Valid,
    files: &BTreeMap<&str, &[u8]>,
) -> Result<KitReport, WrongStack> {
    if &kit.header().package != stack.id() {
        return Err(WrongStack {
            kit: Box::new(kit.header().package.clone()),
            stack: Box::new(stack.id().clone()),
        });
    }
    let spec = &stack.loaded.spec;
    Ok(KitReport {
        package: stack.id().clone(),
        cases: kit
            .cases()
            .iter()
            .map(|(name, case)| CaseReport {
                name: name.clone(),
                outcome: run_case(case, spec, files),
            })
            .collect(),
    })
}

fn run_case(case: &KitCase, spec: &Spec, files: &BTreeMap<&str, &[u8]>) -> Outcome {
    match compare(case, spec, files) {
        Ok(differences) if differences.is_empty() => Outcome::Passed,
        Ok(differences) => Outcome::Differs(differences),
        Err(fault) => Outcome::Broken(fault),
    }
}

// Reads the input once, renders each expected output and compares it.
fn compare(
    case: &KitCase,
    spec: &Spec,
    files: &BTreeMap<&str, &[u8]>,
) -> Result<Vec<Difference>, CaseFault> {
    let input = file(files, case.input.as_str())?;
    let document = Document::parse(input).map_err(|reason| CaseFault::NotADocument {
        path: case.input.as_str().to_owned(),
        reason,
    })?;
    let (tables, diagnostics) = Processor::run(spec, &document, None);
    let render = |output| render(output, &tables, &diagnostics);
    case.expect
        .iter()
        .filter_map(|(&output, path)| difference(output, path.as_str(), files, &render).transpose())
        .collect()
}

// The first difference between one expected file and what the run rendered.
fn difference(
    output: KitOutput,
    path: &str,
    files: &BTreeMap<&str, &[u8]>,
    render: &impl Fn(KitOutput) -> String,
) -> Result<Option<Difference>, CaseFault> {
    let expected = std::str::from_utf8(file(files, path)?).map_err(|_| CaseFault::NotText {
        path: path.to_owned(),
    })?;
    Ok(
        first_difference(expected, &render(output)).map(|at| Difference {
            output,
            file: path.to_owned(),
            at,
        }),
    )
}

fn file<'f>(files: &BTreeMap<&str, &'f [u8]>, path: &str) -> Result<&'f [u8], CaseFault> {
    files
        .get(path)
        .copied()
        .ok_or_else(|| CaseFault::MissingFile {
            path: path.to_owned(),
        })
}
