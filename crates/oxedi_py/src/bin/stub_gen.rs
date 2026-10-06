//! Writes `python/oxedi835/_core.pyi`, the stub of the native module, from
//! the descriptions the binding registers.
//!
//! The generator lays a mixed project's stub out as `_core/__init__.pyi`; the
//! package keeps a single `_core.pyi` beside the extension instead, so the
//! one module is rendered and written here. That relies on pyo3-stub-gen's
//! `StubInfo::modules` and `Module::format_with_config`, which are public but
//! undocumented: check them when bumping the crate.
//!
//! A default the generator could not write renders as `= ...`; the stub is
//! refused then, so the default is written at the source instead.

use std::error::Error;
use std::fs;
use std::process::ExitCode;

const MODULE: &str = "oxedi835._core";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let info = oxedi835_py::stub_info()?;
    let module = match info.modules.get(MODULE) {
        Some(module) if info.modules.len() == 1 => module,
        _ => {
            let names: Vec<&str> = info.modules.keys().map(String::as_str).collect();
            return Err(format!(
                "stub_gen: the binding must describe exactly the module {MODULE}; it describes {names:?}"
            )
            .into());
        }
    };
    let stub = module.format_with_config(info.config.use_type_statement);
    let unwritten: Vec<String> = stub
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("= ..."))
        .map(|(at, line)| format!("  line {}: {}", at + 1, line.trim()))
        .collect();
    if !unwritten.is_empty() {
        return Err(format!(
            "stub_gen: {} line(s) of the {MODULE} stub carry a default rendered as `...`; \
             describe the default at the source (override_type or a hand description):\n{}",
            unwritten.len(),
            unwritten.join("\n")
        )
        .into());
    }
    let dest = info.python_root.join("oxedi835").join("_core.pyi");
    fs::write(&dest, stub)?;
    println!("stub_gen: wrote {}", dest.display());
    Ok(())
}
