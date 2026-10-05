//! Python binding of the EDI 835 parser core.
//!
//! Every class delegates to the core. The only logic of its own is the bridge
//! from the core's columns to Arrow record batches (module `arrow`) and the
//! conversion of diagnostics to Python attributes (module `diagnostic`).
//! Parsing, streaming and exporting run with the GIL released. It also carries
//! the text of the core's `edi_835_parser.json` patch, so the Python layer that
//! reproduces edi-835-parser applies the same file the core's goldens test.
//! Every class, function and exception is also described for the stub
//! generator; the `stub_gen` binary writes the descriptions as
//! `python/oxedi835/_core.pyi`.

use pyo3::prelude::*;

/// Creates an exception of the `oxedi835` package and describes it for the
/// stub of `oxedi835._core`, the module that registers it.
macro_rules! native_exception {
    ($name:ident, $base:ty, $doc:literal) => {
        pyo3::create_exception!(oxedi835, $name, $base, $doc);

        pyo3_stub_gen::inventory::submit! {
            pyo3_stub_gen::type_info::PyClassInfo {
                pyclass_name: stringify!($name),
                struct_id: std::any::TypeId::of::<$name>,
                getters: &[],
                setters: &[],
                module: Some("oxedi835._core"),
                doc: $doc,
                bases: &[|| <$base as pyo3_stub_gen::PyStubType>::type_output()],
                has_eq: false,
                has_ord: false,
                has_hash: false,
                has_str: false,
                subclass: true,
            }
        }
    };
}

mod arrow;
mod diagnostic;
mod document;
mod native;
mod parse;
mod spec;
mod stream;
mod tables;

/// The name of the module attribute holding the text of `edi_835_parser.json`.
const EDI_835_PARSER_PATCH: &str = "EDI_835_PARSER_PATCH";

pyo3_stub_gen::module_variable!("oxedi835._core", EDI_835_PARSER_PATCH, String);

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
    m.add_function(wrap_pyfunction!(diagnostic::external_diagnostic, m)?)?;
    m.add_function(wrap_pyfunction!(parse::parse, m)?)?;
    m.add_function(wrap_pyfunction!(stream::stream, m)?)?;
    m.add(
        EDI_835_PARSER_PATCH,
        include_str!("../../edi835_core/specs/edi_835_parser.json"),
    )?;
    Ok(())
}

// The description of the module that the `stub_gen` binary writes as
// `python/oxedi835/_core.pyi`.
pyo3_stub_gen::define_stub_info_gatherer!(stub_info);
