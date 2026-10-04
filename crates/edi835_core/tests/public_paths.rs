// Each `use` names a public item by its module path; the build fails if one disappears.
#![allow(unused_imports)]

mod by_module {
    use edi835_core::check::EnvelopeChecker;
    use edi835_core::column::Bitmap;
    use edi835_core::column::Cell;
    use edi835_core::column::CellError;
    use edi835_core::column::Column;
    use edi835_core::column::ColumnData;
    use edi835_core::column::ColumnType;
    use edi835_core::column::DECIMAL_PRECISION;
    use edi835_core::column::RowError;
    use edi835_core::column::Table;
    use edi835_core::column::Tables;
    use edi835_core::column::parse_dt;
    use edi835_core::column::parse_n;
    use edi835_core::column::parse_r;
    use edi835_core::column::parse_tm;
    use edi835_core::delimiters::Delimiters;
    use edi835_core::delimiters::IsaError;
    use edi835_core::diagnostic::Diagnostic;
    use edi835_core::diagnostic::LoopRef;
    use edi835_core::diagnostic::Rule;
    use edi835_core::diagnostic::SnipLevel;
    use edi835_core::document::Document;
    use edi835_core::document::DocumentError;
    use edi835_core::document::Segments;
    use edi835_core::document::SizeError;
    use edi835_core::document::Span;
    use edi835_core::document::Spans;
    use edi835_core::element::Element;
    use edi835_core::element::Value;
    use edi835_core::element::split_raw;
    use edi835_core::element::unescape;
    use edi835_core::engine::Event;
    use edi835_core::engine::LoopEngine;
    use edi835_core::frame::BYTE_ORDER_MARK;
    use edi835_core::frame::Frame;
    use edi835_core::frame::find_unescaped;
    use edi835_core::frame::first_frame;
    use edi835_core::frame::is_trivia;
    use edi835_core::frame::leading_trivia;
    use edi835_core::frame::next_frame;
    use edi835_core::process::Output;
    use edi835_core::process::Processor;
    use edi835_core::project::Projector;
    use edi835_core::segment::Segment;
    use edi835_core::segment::WriteError;
    use edi835_core::spec::AnchorChains;
    use edi835_core::spec::ColumnSource;
    use edi835_core::spec::Control;
    use edi835_core::spec::ControlCount;
    use edi835_core::spec::ControlError;
    use edi835_core::spec::DeclaredVersion;
    use edi835_core::spec::ElementDef;
    use edi835_core::spec::ElementDefError;
    use edi835_core::spec::ElementType;
    use edi835_core::spec::LoopDef;
    use edi835_core::spec::LoopId;
    use edi835_core::spec::ROW_COLUMN;
    use edi835_core::spec::Repeat;
    use edi835_core::spec::SEGMENT_COLUMN;
    use edi835_core::spec::SegmentDef;
    use edi835_core::spec::Spec;
    use edi835_core::spec::SpecError;
    use edi835_core::spec::TableDef;
    use edi835_core::spec::TableDefError;
    use edi835_core::spec::Trigger;
    use edi835_core::spec::VersionError;
    use edi835_core::spec::merge_patch;
    use edi835_core::tokenizer::Tokenizer;
    use edi835_core::tree::LoopTree;
    use edi835_core::tree::Node;
    use edi835_core::tree::NodeId;
    use edi835_core::tree::TreeBuilder;
}

// Root re-exports from lib.rs
use edi835_core::EnvelopeChecker;
use edi835_core::Projector;
use edi835_core::Tokenizer;
use edi835_core::{
    AnchorChains, ColumnSource, Control, ControlCount, ControlError, DeclaredVersion, ElementDef,
    ElementDefError, ElementType, LoopDef, LoopId, Repeat, SegmentDef, Spec, SpecError, TableDef,
    TableDefError, Trigger, VersionError, merge_patch,
};
use edi835_core::{
    Bitmap, Cell, CellError, Column, ColumnData, ColumnType, RowError, Table, Tables, parse_dt,
    parse_n, parse_r, parse_tm,
};
use edi835_core::{Delimiters, IsaError};
use edi835_core::{Diagnostic, LoopRef, Rule, SnipLevel};
use edi835_core::{Document, DocumentError, Segments, SizeError, Span, Spans};
use edi835_core::{Element, Value};
use edi835_core::{Event, LoopEngine};
use edi835_core::{Frame, next_frame};
use edi835_core::{LoopTree, Node, NodeId, TreeBuilder};
use edi835_core::{Output, Processor};
use edi835_core::{Segment, WriteError};

#[test]
fn every_public_path_resolves() {}

// Associated items added to `Spec`, named so the build fails if one disappears.
#[allow(dead_code)]
fn spec_items(spec: &Spec) {
    let _: &str = Spec::BUILTIN_835_4010_PATCH;
    let _: fn() -> Spec = Spec::builtin_835_4010;
    let _: for<'a> fn(&'a Spec) -> Option<&'a DeclaredVersion> = Spec::version;
    let _: &Spec = Spec::select(&[spec], spec, Vec::<Segment<'static>>::new());
}
