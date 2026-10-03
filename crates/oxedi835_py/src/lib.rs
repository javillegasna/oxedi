//! Python binding of the EDI 835 parser core.
//!
//! Every class delegates to the core. The only logic of its own is the bridge
//! from the core's columns to Arrow record batches (module `arrow`) and the
//! conversion of diagnostics to Python attributes (module `diagnostic`).
//! Parsing, streaming and exporting run with the GIL released.

use pyo3::prelude::*;

mod spec;

/// The `oxedi835._core` extension module.
#[pymodule]
#[pyo3(name = "_core")]
fn core_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("SpecError", py.get_type::<spec::SpecError>())?;
    m.add_class::<spec::PySpec>()?;
    Ok(())
}
