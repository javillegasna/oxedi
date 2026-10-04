//! Python binding of the EDI 835 parser core.
//!
//! Every class delegates to the core. The only logic of its own is the bridge
//! from the core's columns to Arrow record batches (module `arrow`) and the
//! conversion of diagnostics to Python attributes (module `diagnostic`).
//! Parsing, streaming and exporting run with the GIL released. It also carries
//! the text of the core's `edi_835_parser.json` patch, so the Python layer that
//! reproduces edi-835-parser applies the same file the core's goldens test.

use pyo3::prelude::*;

mod arrow;
mod diagnostic;
mod document;
mod parse;
mod spec;
mod stream;
mod tables;

/// The `oxedi835._core` extension module.
#[pymodule]
#[pyo3(name = "_core")]
fn core_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = m.py();
    m.add("ParseError", py.get_type::<document::ParseError>())?;
    m.add("SpecError", py.get_type::<spec::SpecError>())?;
    m.add_class::<diagnostic::PyDiagnostic>()?;
    m.add_class::<document::PyDelimiters>()?;
    m.add_class::<document::PyDocument>()?;
    m.add_class::<document::PySegment>()?;
    m.add_class::<parse::PyParseResult>()?;
    m.add_class::<spec::PySpec>()?;
    m.add_class::<stream::PyBatch>()?;
    m.add_class::<stream::PyStream>()?;
    m.add_class::<tables::PyTable>()?;
    m.add_class::<tables::PyTables>()?;
    m.add_function(wrap_pyfunction!(parse::parse, m)?)?;
    m.add_function(wrap_pyfunction!(stream::stream, m)?)?;
    m.add(
        "EDI_835_PARSER_PATCH",
        include_str!("../../edi835_core/specs/edi_835_parser.json"),
    )?;
    Ok(())
}
