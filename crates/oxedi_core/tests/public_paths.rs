// Each `use` names a public item by its module path; the build fails if one disappears.
#![allow(unused_imports)]

mod by_module {
    use oxedi_core::check::StructureChecker;
    use oxedi_core::column::Bitmap;
    use oxedi_core::column::Cell;
    use oxedi_core::column::CellError;
    use oxedi_core::column::Column;
    use oxedi_core::column::ColumnData;
    use oxedi_core::column::RowError;
    use oxedi_core::column::Table;
    use oxedi_core::column::Tables;
    use oxedi_core::conformance::CaseFault;
    use oxedi_core::conformance::CaseReport;
    use oxedi_core::conformance::Difference;
    use oxedi_core::conformance::KitReport;
    use oxedi_core::conformance::Line;
    use oxedi_core::conformance::LineDifference;
    use oxedi_core::conformance::Outcome;
    use oxedi_core::conformance::WrongStack;
    use oxedi_core::conformance::first_difference;
    use oxedi_core::conformance::render;
    use oxedi_core::conformance::run_kit;
    use oxedi_core::delimiters::Delimiters;
    use oxedi_core::delimiters::IsaError;
    use oxedi_core::diagnostic::Diagnostic;
    use oxedi_core::diagnostic::LoopPath;
    use oxedi_core::diagnostic::LoopRef;
    use oxedi_core::diagnostic::PARSER_ORIGIN;
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
    use oxedi_core::engine::Instance;
    use oxedi_core::engine::InstanceId;
    use oxedi_core::engine::LoopEngine;
    use oxedi_core::engine::lineage;
    use oxedi_core::engine::loop_paths;
    use oxedi_core::engine::named_lineage;
    use oxedi_core::frame::Frame;
    use oxedi_core::frame::SegmentEnd;
    use oxedi_core::frame::find_unescaped;
    use oxedi_core::frame::first_frame;
    use oxedi_core::frame::next_frame;
    use oxedi_core::mask::Mask;
    use oxedi_core::mask::MaskError;
    use oxedi_core::message::ElementRef;
    use oxedi_core::problem::Detail;
    use oxedi_core::problem::Place;
    use oxedi_core::problem::Problem;
    use oxedi_core::problem::ProblemRecord;
    use oxedi_core::problem::SpecPlace;
    use oxedi_core::process::Output;
    use oxedi_core::process::Processor;
    use oxedi_core::project::Projector;
    use oxedi_core::segment::Segment;
    use oxedi_core::segment::SegmentWriteError;
    use oxedi_core::spec::BatchLoopError;
    use oxedi_core::spec::ColumnSchema;
    use oxedi_core::spec::ColumnSource;
    use oxedi_core::spec::Control;
    use oxedi_core::spec::ControlCount;
    use oxedi_core::spec::DeclaredVersion;
    use oxedi_core::spec::ElementDef;
    use oxedi_core::spec::ElementEntry;
    use oxedi_core::spec::EnvelopeField;
    use oxedi_core::spec::EnvelopeFields;
    use oxedi_core::spec::LoopDef;
    use oxedi_core::spec::LoopId;
    use oxedi_core::spec::NamedTerm;
    use oxedi_core::spec::OccurrenceId;
    use oxedi_core::spec::ROW_COLUMN;
    use oxedi_core::spec::Repeat;
    use oxedi_core::spec::SEGMENT_COLUMN;
    use oxedi_core::spec::SegmentDef;
    use oxedi_core::spec::Selector;
    use oxedi_core::spec::Setting;
    use oxedi_core::spec::Sign;
    use oxedi_core::spec::Spec;
    use oxedi_core::spec::Summary;
    use oxedi_core::spec::TableDef;
    use oxedi_core::spec::TableSchema;
    use oxedi_core::spec::Terms;
    use oxedi_core::spec::Trigger;
    use oxedi_core::spec::UnknownColumn;
    use oxedi_core::spec::UnknownTable;
    use oxedi_core::spec::require_column;
    use oxedi_core::spec::spec_schema;
    use oxedi_core::spec::table_columns;
    use oxedi_core::spec::table_schema;
    use oxedi_core::spec_load::AnchorChains;
    use oxedi_core::spec_load::AnchorPositions;
    use oxedi_core::spec_load::BalanceError;
    use oxedi_core::spec_load::BalanceModel;
    use oxedi_core::spec_load::Builtin;
    use oxedi_core::spec_load::Change;
    use oxedi_core::spec_load::ChangeKind;
    use oxedi_core::spec_load::ColumnModel;
    use oxedi_core::spec_load::Composed;
    use oxedi_core::spec_load::ControlError;
    use oxedi_core::spec_load::ControlModel;
    use oxedi_core::spec_load::ElementDefError;
    use oxedi_core::spec_load::ElementModel;
    use oxedi_core::spec_load::ElementPosition;
    use oxedi_core::spec_load::Guide;
    use oxedi_core::spec_load::HeaderError;
    use oxedi_core::spec_load::IdError;
    use oxedi_core::spec_load::IdPart;
    use oxedi_core::spec_load::Kit;
    use oxedi_core::spec_load::KitCase;
    use oxedi_core::spec_load::KitError;
    use oxedi_core::spec_load::KitHeader;
    use oxedi_core::spec_load::KitOutput;
    use oxedi_core::spec_load::KitPath;
    use oxedi_core::spec_load::Layer;
    use oxedi_core::spec_load::Layers;
    use oxedi_core::spec_load::LoadedSpec;
    use oxedi_core::spec_load::LoopModel;
    use oxedi_core::spec_load::LoopSegments;
    use oxedi_core::spec_load::OccurrenceError;
    use oxedi_core::spec_load::OccurrenceModel;
    use oxedi_core::spec_load::Package;
    use oxedi_core::spec_load::PackageHeader;
    use oxedi_core::spec_load::PackageId;
    use oxedi_core::spec_load::QualifierModel;
    use oxedi_core::spec_load::Release;
    use oxedi_core::spec_load::RepeatModel;
    use oxedi_core::spec_load::Role;
    use oxedi_core::spec_load::Scope;
    use oxedi_core::spec_load::SegmentModel;
    use oxedi_core::spec_load::SpecError;
    use oxedi_core::spec_load::SpecModel;
    use oxedi_core::spec_load::Stack;
    use oxedi_core::spec_load::StackError;
    use oxedi_core::spec_load::Step;
    use oxedi_core::spec_load::SummaryColumnModel;
    use oxedi_core::spec_load::SummaryDefError;
    use oxedi_core::spec_load::SummaryFieldModel;
    use oxedi_core::spec_load::SummaryModel;
    use oxedi_core::spec_load::SummaryRowModel;
    use oxedi_core::spec_load::SummaryTableModel;
    use oxedi_core::spec_load::TableDefError;
    use oxedi_core::spec_load::TableModel;
    use oxedi_core::spec_load::TermModel;
    use oxedi_core::spec_load::TransactionSet;
    use oxedi_core::spec_load::TriggerModel;
    use oxedi_core::spec_load::UnknownVersion;
    use oxedi_core::spec_load::Version;
    use oxedi_core::spec_load::VersionError;
    use oxedi_core::spec_load::VersionModel;
    use oxedi_core::spec_load::merge_patch;
    use oxedi_core::summary::SummaryError;
    use oxedi_core::summary::SummaryValue;
    use oxedi_core::summary::summarize;
    use oxedi_core::tokenizer::Tokenizer;
    use oxedi_core::tree::LoopTree;
    use oxedi_core::tree::Node;
    use oxedi_core::tree::NodeId;
    use oxedi_core::tree::TreeBuilder;
    use oxedi_core::trivia::BYTE_ORDER_MARK;
    use oxedi_core::trivia::LeadingTrivia;
    use oxedi_core::trivia::is_trivia;
    use oxedi_core::validate::CannotWrite;
    use oxedi_core::validate::Capability;
    use oxedi_core::validate::Invalid;
    use oxedi_core::validate::Member;
    use oxedi_core::validate::NoPatientData;
    use oxedi_core::validate::Reasons;
    use oxedi_core::validate::Text;
    use oxedi_core::validate::Valid;
    use oxedi_core::validate::Validation;
    use oxedi_core::validate::validate;
    use oxedi_core::value::ColumnType;
    use oxedi_core::value::ConvertError;
    use oxedi_core::value::DECIMAL_PRECISION;
    use oxedi_core::value::ElementTextError;
    use oxedi_core::value::ElementType;
    use oxedi_core::value::LengthRange;
    use oxedi_core::value::MAX_SCALE;
    use oxedi_core::value::Parsed;
    use oxedi_core::value::TimeUnit;
    use oxedi_core::value::TypeCodeError;
    use oxedi_core::value::date_display;
    use oxedi_core::value::date_element;
    use oxedi_core::value::date_from_midnight;
    use oxedi_core::value::date_from_millis;
    use oxedi_core::value::days_from_civil;
    use oxedi_core::value::decimal_display;
    use oxedi_core::value::decimal_element;
    use oxedi_core::value::integer_from_f64;
    use oxedi_core::value::integer_from_i128;
    use oxedi_core::value::integer_from_u128;
    use oxedi_core::value::parse_dt;
    use oxedi_core::value::parse_n;
    use oxedi_core::value::parse_r;
    use oxedi_core::value::parse_tm;
    use oxedi_core::value::rescale;
    use oxedi_core::value::seconds_from;
    use oxedi_core::value::seconds_of_day;
    use oxedi_core::value::time_display;
    use oxedi_core::value::time_element;
    use oxedi_core::value::time_of_day;
    use oxedi_core::vocabulary::DateMode;
    use oxedi_core::vocabulary::Phi;
    use oxedi_core::vocabulary::Pick;
    use oxedi_core::write::ElementPlan;
    use oxedi_core::write::Envelope;
    use oxedi_core::write::EnvelopeSource;
    use oxedi_core::write::Finding;
    use oxedi_core::write::Instances;
    use oxedi_core::write::LoopPlan;
    use oxedi_core::write::LoopWriter;
    use oxedi_core::write::PlanError;
    use oxedi_core::write::Refusal;
    use oxedi_core::write::SegmentPlan;
    use oxedi_core::write::SegmentSource;
    use oxedi_core::write::ValueOrigin;
    use oxedi_core::write::ValueSource;
    use oxedi_core::write::WriteError;
    use oxedi_core::write::WritePlan;
    use oxedi_core::write::write;
    use oxedi_core::write::write_with_findings;
}

use oxedi_core::conformance::CaseFault;
// The door a binding uses.
mod api_door {
    use oxedi_core::api::Batch;
    use oxedi_core::api::BatchBy;
    use oxedi_core::api::Batches;
    use oxedi_core::api::ChosenSpec;
    use oxedi_core::api::DelimiterChoice;
    use oxedi_core::api::Identity;
    use oxedi_core::api::ReadOptions;
    use oxedi_core::api::Reading;
    use oxedi_core::api::SpecChoice;
    use oxedi_core::api::builtin;
    use oxedi_core::api::read;
    use oxedi_core::api::read_tables;
    use oxedi_core::api::read_tree;
}

use oxedi_core::spec_load::SpecModel;
use oxedi_core::write::{Finding, WriteError};
// Root re-exports from lib.rs
use oxedi_core::ConvertError;
use oxedi_core::ElementRef;
use oxedi_core::Place;
use oxedi_core::Problem;
use oxedi_core::ProblemRecord;
use oxedi_core::Projector;
use oxedi_core::StructureChecker;
use oxedi_core::Tokenizer;
use oxedi_core::{
    AnchorChains, AnchorPositions, Builtin, ColumnSource, Control, ControlCount, ControlError,
    DeclaredVersion, ElementDef, ElementDefError, ElementType, EnvelopeField, EnvelopeFields,
    LoadedSpec, LoopDef, LoopId, LoopSegments, OccurrenceId, Pick, Repeat, SegmentDef, Selector,
    Spec, SpecError, TableDef, TableDefError, Terms, Trigger, UnknownColumn, UnknownTable,
    UnknownVersion, Version, VersionError, merge_patch,
};
use oxedi_core::{
    Bitmap, Cell, CellError, Column, ColumnData, ColumnType, RowError, Table, Tables,
};
use oxedi_core::{
    CannotWrite, Capability, Invalid, Member, NoPatientData, Reasons, Text, Valid, Validation,
    validate,
};
use oxedi_core::{ColumnSchema, TableSchema, spec_schema};
use oxedi_core::{DateMode, Mask, MaskError, Phi};
use oxedi_core::{Delimiters, IsaError};
use oxedi_core::{Diagnostic, LoopRef, PARSER_ORIGIN, Rule, SnipLevel};
use oxedi_core::{Document, DocumentError, Segments, SizeError, Span, Spans};
use oxedi_core::{Element, Value};
use oxedi_core::{Event, Instance, InstanceId, LoopEngine, lineage, loop_paths, named_lineage};
use oxedi_core::{Frame, SegmentEnd, next_frame};
use oxedi_core::{LoopTree, Node, NodeId, TreeBuilder};
use oxedi_core::{Output, Processor};
use oxedi_core::{PlanError, Refusal, WritePlan};
use oxedi_core::{Segment, SegmentWriteError};
use oxedi_core::{Summary, SummaryDefError, SummaryError, SummaryValue, summarize};
use oxedi_core::{parse_dt, parse_n, parse_r, parse_tm};

#[test]
fn every_public_path_resolves() {}

// Associated items added to `Spec`, named so the build fails if one disappears.
#[allow(dead_code)]
fn spec_items(spec: &Spec) {
    let _: fn() -> Spec = Spec::builtin_835_4010;
    let _: for<'a> fn(&'a Spec) -> Option<&'a DeclaredVersion> = Spec::version;
    let _: &Spec = Spec::select(&[spec], spec, Vec::<Segment<'static>>::new());
    let _: &[(String, Summary)] = spec.summaries();
    let _: Option<&Summary> = spec.summary("count_claims");
    let _: Option<&str> = spec.default_table();
}

// Associated items of `Builtin`, named so the build fails if one disappears.
#[allow(dead_code)]
fn builtin_items() {
    let _: Version = Builtin::DEFAULT_VERSION;
    let _: fn() -> &'static [Builtin] = Builtin::all;
    let _: fn() -> &'static [&'static str] = Builtin::packages;
    let _: fn(oxedi_core::spec_load::Role) -> &'static str = oxedi_core::spec_load::Role::word;
    let _: fn(&str, &[&str]) -> Validation = validate;
    let _: fn(&str, &str) -> Option<oxedi_core::conformance::LineDifference> =
        oxedi_core::conformance::first_difference;
    let _: fn(
        oxedi_core::spec_load::KitOutput,
        &oxedi_core::column::Tables,
        &[oxedi_core::diagnostic::Diagnostic],
    ) -> String = oxedi_core::conformance::render;
    let _: fn() -> &'static Builtin = Builtin::fallback;
    let _: fn(&str, Version) -> Result<&'static Builtin, UnknownVersion> = Builtin::find;
    let _: fn(&str, &str) -> Result<&'static Builtin, UnknownVersion> = Builtin::find_text;
    let _: fn(&str) -> Vec<Version> = Builtin::versions;
    let _: fn(&oxedi_core::spec_load::PackageId) -> Option<&'static Builtin> = Builtin::get;
    let _: fn(Version) -> &'static str = Version::release;
    let _: fn(&oxedi_core::spec_load::Release) -> Option<Version> = Version::of_release;
    let _: [Version; 2] = Version::ALL;
    let _: fn(Version) -> &'static str = Version::text;
    let _: fn(&str) -> Option<Version> = Version::parse;
    let _: &Builtin = Builtin::select(Vec::<Segment<'static>>::new());
}

// Associated items of `LoadedSpec` and the model's loader.
#[allow(dead_code)]
fn loaded_items() {
    let _: fn(&str) -> Result<LoadedSpec, SpecError> = LoadedSpec::from_json;
    let _: fn(&str) -> Result<oxedi_core::spec_load::Kit, SpecError> =
        oxedi_core::spec_load::Kit::from_json;
    let _: fn(SpecModel) -> Result<LoadedSpec, SpecError> = LoadedSpec::from_model;
    let _: fn(&LoadedSpec, &str) -> Result<LoadedSpec, SpecError> = LoadedSpec::merge_patch;
    let _: fn(&LoadedSpec) -> Result<String, serde_json::Error> = LoadedSpec::to_json;
    let _: fn(&str) -> Result<SpecModel, SpecError> = SpecModel::from_json;
    let _: fn(&SpecModel) -> Result<Spec, SpecError> = SpecModel::compile;
    let _: fn(&Builtin) -> Result<SpecModel, SpecError> = Builtin::model;
    let _: fn(&Builtin) -> Result<LoadedSpec, SpecError> = Builtin::loaded;
}

// Associated items the bindings read their defaults, names and units from,
// named so the build fails if one disappears.
#[allow(dead_code)]
fn binding_items() {
    use oxedi_core::DateMode;
    use oxedi_core::value::TimeUnit;
    use oxedi_core::write::Envelope;
    let _: Envelope = Envelope::with_ids("SENDER", "RECEIVER", 0, 0);
    let _: &str = Envelope::DEFAULT_QUALIFIER;
    let _: &str = Envelope::DEFAULT_USAGE_INDICATOR;
    let _: u64 = Envelope::DEFAULT_CONTROL_NUMBER;
    let _: for<'a> fn(&'a Diagnostic) -> &'a str = Diagnostic::origin;
    let _: for<'a> fn(&'a Diagnostic) -> Option<&'a str> = Diagnostic::code;
    let _: fn(&Diagnostic) -> String = Diagnostic::path_text;
    let _: fn(SnipLevel) -> u8 = SnipLevel::number;
    let _: fn(DateMode) -> &'static str = DateMode::word;
    let _: [TimeUnit; 4] = [
        TimeUnit::Second,
        TimeUnit::Millisecond,
        TimeUnit::Microsecond,
        TimeUnit::Nanosecond,
    ];
    let _: fn(TimeUnit) -> i64 = TimeUnit::ticks_per_second;
    let _: fn(TimeUnit) -> i64 = TimeUnit::ticks_per_day;
    let _: usize = IsaError::ISA_LEN;
    let _: fn(&IsaError) -> usize = IsaError::skipped;
    let _: for<'a> fn(&'a IsaError, &'a [u8]) -> &'a [u8] = IsaError::datum;
    let _: for<'a> fn(&'a DocumentError, &'a [u8]) -> &'a [u8] = DocumentError::datum;
    let _: for<'a> fn(&'a WriteError) -> &'a [Finding] = WriteError::findings;
    let _: for<'a> fn(&'a CaseFault) -> &'a str = CaseFault::path;
    let _: for<'a> fn(&'a Segment<'static>, usize, usize) -> Option<&'a [u8]> = Segment::component;
}
