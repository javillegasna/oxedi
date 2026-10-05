//! Writes `python/oxedi835/_core.pyi`, the stub of the native module, from
//! the descriptions the binding registers.
//!
//! The generator lays a mixed project's stub out as `_core/__init__.pyi`; the
//! package keeps a single `_core.pyi` beside the extension instead, so the
//! one module is rendered and written here.

use std::error::Error;
use std::fs;

const MODULE: &str = "oxedi835._core";

fn main() -> Result<(), Box<dyn Error>> {
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
    let dest = info.python_root.join("oxedi835").join("_core.pyi");
    fs::write(
        &dest,
        module.format_with_config(info.config.use_type_statement),
    )?;
    println!("stub_gen: wrote {}", dest.display());
    Ok(())
}
