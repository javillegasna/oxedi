//! The built-in conformance kits: each manifest in `tests/kits/` runs against
//! the stack it names, and names only synthetic fixtures and their goldens.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use oxedi_core::conformance::{KitReport, run_kit};
use oxedi_core::spec_load::{Builtin, Kit, PackageId};
use oxedi_core::validate::{Valid, Validation, validate};

const MANIFEST_SUFFIX: &str = ".kit.json";

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn kits_dir() -> PathBuf {
    crate_root().join("tests/kits")
}

/// The manifests in `tests/kits/`, by file name. A file that is not a
/// manifest fails, so a new file cannot sit there unnoticed.
fn manifests() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(kits_dir())
        .unwrap_or_else(|e| panic!("cannot list tests/kits: {e}"))
        .map(|entry| {
            let entry = entry.unwrap_or_else(|e| panic!("cannot list tests/kits: {e}"));
            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();
    names.sort();
    let stray: Vec<&String> = names
        .iter()
        .filter(|name| !name.ends_with(MANIFEST_SUFFIX))
        .collect();
    assert!(
        stray.is_empty() && !names.is_empty(),
        "tests/kits holds only *{MANIFEST_SUFFIX} manifests, and at least one; found {names:?}"
    );
    names
}

fn read_kit(manifest: &str) -> Kit {
    let text = std::fs::read_to_string(kits_dir().join(manifest)).expect(manifest);
    Kit::from_json(&text).expect(manifest)
}

/// The file a kit path names, from the manifest's directory with `..` applied.
/// It must land under `tests/fixtures/` or `tests/golden/`.
fn resolve(path: &str) -> PathBuf {
    let mut resolved = kits_dir();
    for part in path.split('/') {
        match part {
            ".." => {
                resolved.pop();
            }
            part => resolved.push(part),
        }
    }
    let allowed = ["tests/fixtures", "tests/golden"]
        .iter()
        .any(|dir| resolved.starts_with(crate_root().join(dir)));
    assert!(allowed, "{path} resolves to {}", resolved.display());
    resolved
}

/// The compat patch package, which sits on the shipped stack and is not part of it.
const COMPAT: &str = include_str!("../../oxedi_py/python/oxedi/edi_835_parser/edi_835_parser.json");

/// The validated stack whose top package is `id`: a shipped package, or the
/// compat patch, over the shipped texts.
fn stack_of(id: &PackageId) -> Valid {
    let packages = Builtin::packages();
    packages
        .iter()
        .copied()
        .chain([COMPAT])
        .find_map(|top| match validate(top, packages) {
            Validation::Yes(valid) if valid.id() == id => Some(*valid),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no shipped stack has {id} on top"))
}

/// Runs a manifest on the stack its header names, handing the runner the bytes
/// of every file the kit lists.
fn run(manifest: &str) -> KitReport {
    let kit = read_kit(manifest);
    let stack = stack_of(&kit.header().package);
    let bytes: BTreeMap<&str, Vec<u8>> = kit
        .files()
        .into_iter()
        .map(|path| (path, std::fs::read(resolve(path)).expect(path)))
        .collect();
    let files: BTreeMap<&str, &[u8]> = bytes
        .iter()
        .map(|(path, bytes)| (*path, bytes.as_slice()))
        .collect();
    run_kit(&kit, &stack, &files).expect(manifest)
}

#[test]
fn every_builtin_kit_passes() {
    for manifest in manifests() {
        let report = run(&manifest);
        assert!(report.passed(), "{manifest}\n{report}");
    }
}

#[test]
fn builtin_kits_name_only_fixtures_and_goldens() {
    for manifest in manifests() {
        for path in read_kit(&manifest).files() {
            let resolved = resolve(path);
            assert!(
                !resolved
                    .components()
                    .any(|part| part.as_os_str() == "samples"),
                "{manifest}: {}",
                resolved.display()
            );
            assert!(resolved.is_file(), "{manifest}: {path}");
        }
    }
}

#[test]
fn every_fixture_with_an_isa_is_a_case_of_its_version() {
    let (five, four) = common::builtins();
    let mut inputs = Vec::new();
    for manifest in manifests() {
        let kit = read_kit(&manifest);
        let Some(named) = Builtin::get(&kit.header().package) else {
            continue;
        };
        let expected = if std::ptr::eq(named, Builtin::fallback()) {
            &five
        } else {
            &four
        };
        for case in kit.cases().values() {
            let name = case.input.as_str().rsplit('/').next().unwrap().to_owned();
            let bytes = common::load_fixture(&name);
            let delims = oxedi_core::delimiters::Delimiters::from_isa(&bytes).expect(&name);
            let selected = common::select(&five, &four, &bytes, delims);
            assert!(std::ptr::eq(selected, expected), "{manifest}: {name}");
            inputs.push(name);
        }
    }
    let named: Vec<&str> = inputs.iter().map(String::as_str).collect();
    common::assert_same_names("kit inputs and FIXTURES", &named, common::FIXTURES);
}

#[test]
fn the_compat_kit_names_every_isa_fixture() {
    let kit = read_kit("edi-835-parser-835-005010-X221A1.kit.json");
    let inputs: Vec<&str> = kit
        .cases()
        .values()
        .map(|case| case.input.as_str().rsplit('/').next().unwrap())
        .collect();
    common::assert_same_names("compat kit inputs and FIXTURES", &inputs, common::FIXTURES);
}
