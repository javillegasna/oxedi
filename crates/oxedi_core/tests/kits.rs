//! The built-in conformance kits: each manifest in `tests/kits/` runs against
//! the built-in it names, and names only synthetic fixtures and their goldens.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxedi_core::conformance::{KitReport, run_kit};
use oxedi_core::spec_load::{Builtin, Kit};

/// The manifests, relative to this crate: the 5010 kit, then the 4010 kit.
const KITS: &[&str] = &[
    "tests/kits/835-005010-X221A1.kit.json",
    "tests/kits/835-004010-X091A1.kit.json",
];

fn manifest_path(manifest: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(manifest)
}

fn read_kit(manifest: &str) -> Kit {
    let text = std::fs::read_to_string(manifest_path(manifest)).expect(manifest);
    Kit::from_json(&text).expect(manifest)
}

/// Runs a manifest with the built-in its header names, handing the runner the
/// bytes of every file the kit lists, resolved against the manifest's directory.
fn run(manifest: &str) -> KitReport {
    let kit = read_kit(manifest);
    let builtin = Builtin::get(&kit.header().package).expect("a built-in kit names a built-in");
    let dir = manifest_path(manifest).parent().unwrap().to_path_buf();
    let bytes: BTreeMap<&str, Vec<u8>> = kit
        .files()
        .into_iter()
        .map(|path| (path, std::fs::read(dir.join(path)).expect(path)))
        .collect();
    let files: BTreeMap<&str, &[u8]> = bytes
        .iter()
        .map(|(path, bytes)| (*path, bytes.as_slice()))
        .collect();
    run_kit(&kit, &builtin.spec, &files)
}

/// The path a kit file names, from this crate's root, with `..` applied.
fn from_crate_root(manifest: &str, path: &str) -> String {
    let mut parts: Vec<&str> = Path::new(manifest)
        .parent()
        .unwrap()
        .to_str()
        .unwrap()
        .split('/')
        .collect();
    for part in path.split('/') {
        match part {
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

#[test]
fn every_builtin_kit_passes() {
    for manifest in KITS {
        let report = run(manifest);
        assert!(report.passed(), "{manifest}\n{report}");
    }
}

#[test]
fn builtin_kits_name_only_fixtures_and_goldens() {
    for manifest in KITS {
        for path in read_kit(manifest).files() {
            let resolved = from_crate_root(manifest, path);
            assert!(
                resolved.starts_with("tests/fixtures/") || resolved.starts_with("tests/golden/"),
                "{manifest}: {path} resolves to {resolved}"
            );
            assert!(!resolved.contains("samples"), "{manifest}: {resolved}");
        }
    }
}

#[test]
fn every_fixture_with_an_isa_is_a_case_of_its_version() {
    let (five, four) = common::builtins();
    let mut inputs = Vec::new();
    for (manifest, expected) in KITS.iter().zip([&five, &four]) {
        for case in read_kit(manifest).cases().values() {
            let name = case.input.as_str().rsplit('/').next().unwrap().to_owned();
            let bytes = common::load_fixture(&name);
            let delims = oxedi_core::Delimiters::from_isa(&bytes).expect(&name);
            let selected = common::select(&five, &four, &bytes, delims);
            assert!(std::ptr::eq(selected, expected), "{manifest}: {name}");
            inputs.push(name);
        }
    }
    let named: Vec<&str> = inputs.iter().map(String::as_str).collect();
    common::assert_same_names("kit inputs and FIXTURES", &named, common::FIXTURES);
}
