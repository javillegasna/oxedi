// Each `use` names a public item by its module path; the build fails if one disappears.
#![allow(unused_imports)]

mod by_module {
    use oxedi_core::check::EnvelopeChecker;
    use oxedi_core::column::Bitmap;
    use oxedi_core::column::Cell;
    use oxedi_core::column::CellError;
    use oxedi_core::column::Column;
    use oxedi_core::column::ColumnData;
    use oxedi_core::column::ColumnType;
    use oxedi_core::column::DECIMAL_PRECISION;
    use oxedi_core::column::RowError;
    use oxedi_core::column::Table;
    use oxedi_core::column::Tables;
    use oxedi_core::column::parse_dt;
    use oxedi_core::column::parse_n;
    use oxedi_core::column::parse_r;
    use oxedi_core::column::parse_tm;
    use oxedi_core::delimiters::Delimiters;
    use oxedi_core::delimiters::IsaError;
    use oxedi_core::diagnostic::Diagnostic;
    use oxedi_core::diagnostic::LoopRef;
    use oxedi_core::diagnostic::Rule;
    use oxedi_core::diagnostic::SnipLevel;
    use oxedi_core::document::Document;
    use oxedi_core::document::DocumentError;
    use oxedi_core::document::Segments;
    use oxedi_core::document::SizeError;
    use oxedi_core::document::Span;
    use oxedi_core::document::Spans;
    use oxedi_core::element::Element;
    use oxedi_core::element::Value;
    use oxedi_core::element::split_raw;
    use oxedi_core::element::unescape;
    use oxedi_core::engine::Event;
    use oxedi_core::engine::LoopEngine;
    use oxedi_core::frame::BYTE_ORDER_MARK;
    use oxedi_core::frame::Frame;
    use oxedi_core::frame::find_unescaped;
    use oxedi_core::frame::first_frame;
    use oxedi_core::frame::is_trivia;
    use oxedi_core::frame::leading_trivia;
    use oxedi_core::frame::next_frame;
    use oxedi_core::process::Output;
    use oxedi_core::process::Processor;
    use oxedi_core::project::Projector;
    use oxedi_core::segment::Segment;
    use oxedi_core::segment::WriteError;
    use oxedi_core::spec::AnchorChains;
    use oxedi_core::spec::ColumnSource;
    use oxedi_core::spec::Control;
    use oxedi_core::spec::ControlCount;
    use oxedi_core::spec::ControlError;
    use oxedi_core::spec::DeclaredVersion;
    use oxedi_core::spec::ElementDef;
    use oxedi_core::spec::ElementDefError;
    use oxedi_core::spec::ElementType;
    use oxedi_core::spec::LoopDef;
    use oxedi_core::spec::LoopId;
    use oxedi_core::spec::LoopSegments;
    use oxedi_core::spec::Pick;
    use oxedi_core::spec::ROW_COLUMN;
    use oxedi_core::spec::Repeat;
    use oxedi_core::spec::SEGMENT_COLUMN;
    use oxedi_core::spec::SegmentDef;
    use oxedi_core::spec::Spec;
    use oxedi_core::spec::SpecError;
    use oxedi_core::spec::TableDef;
    use oxedi_core::spec::TableDefError;
    use oxedi_core::spec::Trigger;
    use oxedi_core::spec::VersionError;
    use oxedi_core::spec::merge_patch;
    use oxedi_core::tokenizer::Tokenizer;
    use oxedi_core::tree::LoopTree;
    use oxedi_core::tree::Node;
    use oxedi_core::tree::NodeId;
    use oxedi_core::tree::TreeBuilder;
}

// Root re-exports from lib.rs
use oxedi_core::EnvelopeChecker;
use oxedi_core::Projector;
use oxedi_core::Tokenizer;
use oxedi_core::{
    AnchorChains, ColumnSource, Control, ControlCount, ControlError, DeclaredVersion, ElementDef,
    ElementDefError, ElementType, LoopDef, LoopId, LoopSegments, Pick, Repeat, SegmentDef, Spec,
    SpecError, TableDef, TableDefError, Trigger, VersionError, merge_patch,
};
use oxedi_core::{
    Bitmap, Cell, CellError, Column, ColumnData, ColumnType, RowError, Table, Tables, parse_dt,
    parse_n, parse_r, parse_tm,
};
use oxedi_core::{Delimiters, IsaError};
use oxedi_core::{Diagnostic, LoopRef, Rule, SnipLevel};
use oxedi_core::{Document, DocumentError, Segments, SizeError, Span, Spans};
use oxedi_core::{Element, Value};
use oxedi_core::{Event, LoopEngine};
use oxedi_core::{Frame, next_frame};
use oxedi_core::{LoopTree, Node, NodeId, TreeBuilder};
use oxedi_core::{Output, Processor};
use oxedi_core::{Segment, WriteError};

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
