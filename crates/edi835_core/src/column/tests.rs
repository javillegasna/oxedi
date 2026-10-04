use super::*;
use proptest::prelude::*;

/// An `R` value written the way X12 writes it, with exactly `scale` decimals.
fn format_r(value: i128, scale: u8) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let digits = value.unsigned_abs().to_string();
    let scale = usize::from(scale);
    if scale == 0 {
        return format!("{sign}{digits}");
    }
    let padded = format!("{digits:0>width$}", width = scale + 1);
    let (whole, fraction) = padded.split_at(padded.len() - scale);
    format!("{sign}{whole}.{fraction}")
}

#[test]
fn column_types_follow_the_element_types() {
    assert_eq!(ColumnType::of(None), ColumnType::Binary);
    assert_eq!(ColumnType::of(Some(ElementType::An)), ColumnType::Binary);
    assert_eq!(ColumnType::of(Some(ElementType::Id)), ColumnType::Binary);
    assert_eq!(
        ColumnType::of(Some(ElementType::N(2))),
        ColumnType::Int64 { scale: 2 }
    );
    assert_eq!(
        ColumnType::of(Some(ElementType::R { scale: 4 })),
        ColumnType::Decimal128 {
            precision: 38,
            scale: 4
        }
    );
    assert_eq!(ColumnType::of(Some(ElementType::Dt)), ColumnType::Date32);
    assert_eq!(ColumnType::of(Some(ElementType::Tm)), ColumnType::Time32);
}

#[test]
fn column_types_display_their_arrow_names() {
    assert_eq!(ColumnType::Binary.to_string(), "binary");
    assert_eq!(ColumnType::Int64 { scale: 0 }.to_string(), "int64");
    assert_eq!(
        ColumnType::Int64 { scale: 2 }.to_string(),
        "int64 (scale 2)"
    );
    assert_eq!(
        ColumnType::Decimal128 {
            precision: 38,
            scale: 2
        }
        .to_string(),
        "decimal128(38, 2)"
    );
    assert_eq!(ColumnType::Date32.to_string(), "date32");
    assert_eq!(ColumnType::Time32.to_string(), "time32 (seconds)");
}

#[test]
fn a_bitmap_packs_bits_least_significant_first() {
    let mut bitmap = Bitmap::new();
    for valid in [true, false, true, true, false, false, false, false, true] {
        bitmap.push(valid);
    }
    assert_eq!(bitmap.len(), 9);
    assert_eq!(bitmap.as_bytes(), &[0b0000_1101, 0b0000_0001]);
    assert_eq!(bitmap.get(0), Some(true));
    assert_eq!(bitmap.get(1), Some(false));
    assert_eq!(bitmap.get(8), Some(true));
    assert_eq!(bitmap.get(9), None);
    assert_eq!(bitmap.unset_count(), 5);
    assert!(Bitmap::new().is_empty());
}

#[test]
fn a_binary_column_keeps_arrow_offsets_and_repeats_them_for_nulls() {
    let mut column = ColumnData::new(ColumnType::Binary);
    column.push(Cell::Binary(b"AB")).unwrap();
    column.push_null();
    column.push(Cell::Binary(b"")).unwrap();
    column.push(Cell::Binary(b"CDE")).unwrap();
    assert_eq!(
        column.column(),
        &Column::Binary {
            offsets: vec![0, 2, 2, 2, 5],
            data: b"ABCDE".to_vec()
        }
    );
    assert_eq!(column.validity().as_bytes(), &[0b1101]);
    assert_eq!(column.get(0), Some(Cell::Binary(b"AB")));
    assert_eq!(column.get(1), Some(Cell::Null));
    assert_eq!(column.get(2), Some(Cell::Binary(b"")));
    assert_eq!(column.get(3), Some(Cell::Binary(b"CDE")));
    assert_eq!(column.get(4), None);
    assert_eq!((column.len(), column.null_count()), (4, 1));
}

#[test]
fn cells_render_as_text_by_column_type() {
    let rendered = |kind: ColumnType, cells: &[Cell<'_>]| -> Vec<String> {
        let mut column = ColumnData::new(kind);
        for &cell in cells {
            column.push(cell).unwrap();
        }
        (0..column.len())
            .map(|row| column.render(row).unwrap())
            .collect()
    };
    assert_eq!(
        rendered(ColumnType::Binary, &[Cell::Binary(b"HC:99213"), Cell::Null]),
        vec!["HC:99213", "∅"]
    );
    assert_eq!(
        rendered(ColumnType::Int64 { scale: 2 }, &[Cell::Int64(-42)]),
        vec!["-42"]
    );
    assert_eq!(
        rendered(
            ColumnType::Decimal128 {
                precision: 38,
                scale: 2
            },
            &[
                Cell::Decimal128(12345),
                Cell::Decimal128(-5),
                Cell::Decimal128(0)
            ]
        ),
        vec!["123.45", "-0.05", "0.00"]
    );
    assert_eq!(
        rendered(
            ColumnType::Decimal128 {
                precision: 38,
                scale: 0
            },
            &[Cell::Decimal128(7)]
        ),
        vec!["7"]
    );
    assert_eq!(
        rendered(ColumnType::Date32, &[Cell::Date32(0), Cell::Date32(19_782)]),
        vec!["1970-01-01", "2024-02-29"]
    );
    assert_eq!(
        rendered(ColumnType::Time32, &[Cell::Time32(45_045)]),
        vec!["12:30:45"]
    );
    assert_eq!(ColumnData::new(ColumnType::Binary).render(0), None);
}

#[test]
fn fixed_width_columns_hold_zero_under_a_null() {
    let mut column = ColumnData::new(ColumnType::Decimal128 {
        precision: 38,
        scale: 2,
    });
    column.push(Cell::Decimal128(-1250)).unwrap();
    column.push(Cell::Null).unwrap();
    assert_eq!(
        column.column(),
        &Column::Decimal128 {
            values: vec![-1250, 0],
            precision: 38,
            scale: 2
        }
    );
    assert_eq!(column.get(1), Some(Cell::Null));
}

#[test]
fn dates_and_times_out_of_range_render_as_raw_numbers() {
    let mut dates = ColumnData::new(ColumnType::Date32);
    for days in [i32::MAX, i32::MAX - 719_468, i32::MAX - 719_469, 0] {
        dates.push(Cell::Date32(days)).unwrap();
    }
    assert_eq!(dates.render(0).as_deref(), Some("date32(2147483647)"));
    assert_eq!(dates.render(1).as_deref(), Some("5879610-09-09"));
    assert_eq!(dates.render(2).as_deref(), Some("5879610-09-08"));
    assert_eq!(dates.render(3).as_deref(), Some("1970-01-01"));
    let mut times = ColumnData::new(ColumnType::Time32);
    for seconds in [-1, i32::MIN, 0, 86_399, 86_400, 90_000, i32::MAX] {
        times.push(Cell::Time32(seconds)).unwrap();
    }
    assert_eq!(times.render(0).as_deref(), Some("time32(-1)"));
    assert_eq!(times.render(1).as_deref(), Some("time32(-2147483648)"));
    assert_eq!(times.render(2).as_deref(), Some("00:00:00"));
    assert_eq!(times.render(3).as_deref(), Some("23:59:59"));
    assert_eq!(times.render(4).as_deref(), Some("time32(86400)"));
    assert_eq!(times.render(5).as_deref(), Some("time32(90000)"));
    assert_eq!(times.render(6).as_deref(), Some("time32(2147483647)"));
}

#[test]
fn a_refused_text_value_is_quoted_and_cut_at_32_bytes() {
    let mut column = ColumnData::new(ColumnType::Date32);
    let long = [b'x'; 40];
    assert_eq!(
        column.push(Cell::Binary(&long)).unwrap_err().to_string(),
        format!(
            "a date32 column cannot hold a binary value (\"{}\"...)",
            "x".repeat(32)
        )
    );
}

#[test]
fn a_refused_text_value_escapes_invalid_bytes_and_cuts_on_a_character() {
    let mut column = ColumnData::new(ColumnType::Date32);
    assert_eq!(
        column
            .push(Cell::Binary(b"a\xE9\"b"))
            .unwrap_err()
            .to_string(),
        "a date32 column cannot hold a binary value (\"a\\xE9\\\"b\")"
    );
    let mut text = vec![b'x'; 31];
    text.extend_from_slice("\u{e9}tail".as_bytes());
    assert_eq!(
        column.push(Cell::Binary(&text)).unwrap_err().to_string(),
        format!(
            "a date32 column cannot hold a binary value (\"{}\"...)",
            "x".repeat(31)
        )
    );
    let mut exact = vec![b'x'; 30];
    exact.extend_from_slice("\u{e9}tail".as_bytes());
    assert_eq!(
        column.push(Cell::Binary(&exact)).unwrap_err().to_string(),
        format!(
            "a date32 column cannot hold a binary value (\"{}\u{e9}\"...)",
            "x".repeat(30)
        )
    );
}

#[test]
fn a_refused_run_of_stray_continuation_bytes_keeps_its_cut_at_32_bytes() {
    let mut column = ColumnData::new(ColumnType::Date32);
    assert_eq!(
        column
            .push(Cell::Binary(&[0x80; 40]))
            .unwrap_err()
            .to_string(),
        format!(
            "a date32 column cannot hold a binary value (\"{}\"...)",
            "\\x80".repeat(32)
        )
    );
}

#[test]
fn a_cell_of_another_type_is_refused_and_nothing_is_appended() {
    let mut column = ColumnData::new(ColumnType::Date32);
    assert_eq!(
        column.push(Cell::Int64(3)),
        Err(CellError::TypeMismatch {
            column: ColumnType::Date32,
            cell: "int64",
            value: "3".into()
        })
    );
    assert!(column.is_empty());
}

#[test]
fn cell_errors_display_the_column_and_the_cell() {
    assert_eq!(
        CellError::TypeMismatch {
            column: ColumnType::Date32,
            cell: "int64",
            value: "3".into()
        }
        .to_string(),
        "a date32 column cannot hold an int64 value (3)"
    );
    assert_eq!(
        CellError::TypeMismatch {
            column: ColumnType::Int64 { scale: 0 },
            cell: "date32",
            value: "-1".into()
        }
        .to_string(),
        "an int64 column cannot hold a date32 value (-1)"
    );
    assert_eq!(
        CellError::BinaryOverflow { bytes: 2147483650 }.to_string(),
        "a binary column holds at most 2147483647 bytes; this value would bring it to 2147483650"
    );
}

#[test]
fn a_row_is_appended_whole_or_not_at_all() {
    let mut table = Table::new(
        "t",
        [
            ("id".to_string(), ColumnType::Binary),
            ("amount".to_string(), ColumnType::Int64 { scale: 0 }),
        ],
    );
    table
        .push_row(&[Cell::Binary(b"A"), Cell::Int64(1)])
        .unwrap();
    let err = table
        .push_row(&[Cell::Binary(b"B"), Cell::Binary(b"x")])
        .unwrap_err();
    assert_eq!(
        err,
        RowError::Cell {
            table: "t".into(),
            column: "amount".into(),
            source: CellError::TypeMismatch {
                column: ColumnType::Int64 { scale: 0 },
                cell: "binary",
                value: "\"x\"".into()
            }
        }
    );
    assert_eq!(
        table.push_row(&[Cell::Null]),
        Err(RowError::Arity {
            table: "t".into(),
            expected: 2,
            found: 1
        })
    );
    assert_eq!(table.len(), 1);
    for (_, column) in table.columns() {
        assert_eq!(column.len(), 1);
    }
    assert_eq!(table.column("id").unwrap().get(0), Some(Cell::Binary(b"A")));
    assert_eq!(table.column("missing"), None);
}

#[test]
fn row_errors_display_the_table_and_the_column() {
    let arity = RowError::Arity {
        table: "claims".into(),
        expected: 5,
        found: 4,
    };
    assert_eq!(
        arity.to_string(),
        "table \"claims\" has 5 columns; the row has 4 cells"
    );
    assert!(std::error::Error::source(&arity).is_none());
    let cell = RowError::Cell {
        table: "claims".into(),
        column: "charge".into(),
        source: CellError::TypeMismatch {
            column: ColumnType::Decimal128 {
                precision: 38,
                scale: 2,
            },
            cell: "binary",
            value: "\"ab\"".into(),
        },
    };
    assert_eq!(
        cell.to_string(),
        "table \"claims\" column \"charge\": a decimal128(38, 2) column cannot hold a binary value (\"ab\")"
    );
    assert!(std::error::Error::source(&cell).is_some());
}

#[test]
fn a_table_renders_its_title_header_and_rows() {
    let mut claims = Table::new(
        "claims",
        [
            ("id".to_string(), ColumnType::Binary),
            (
                "charge".to_string(),
                ColumnType::Decimal128 {
                    precision: 38,
                    scale: 2,
                },
            ),
        ],
    );
    claims
        .push_row(&[Cell::Binary(b"A1"), Cell::Decimal128(-1250)])
        .unwrap();
    claims.push_row(&[Cell::Null, Cell::Null]).unwrap();
    assert_eq!(claims.header(), "id: binary | charge: decimal128(38, 2)");
    assert_eq!(claims.render_row(0).as_deref(), Some("A1 | -12.50"));
    assert_eq!(claims.render_row(1).as_deref(), Some("∅ | ∅"));
    assert_eq!(claims.render_row(2), None);
    assert_eq!(
        claims.to_string(),
        "## claims (rows: 2)\nid: binary | charge: decimal128(38, 2)\nA1 | -12.50\n∅ | ∅\n\n"
    );
    let empty = Table::new("adjustments", [("n".to_string(), ColumnType::Date32)]);
    assert_eq!(empty.render_row(0), None);
    assert_eq!(
        Tables::new(vec![claims, empty]).to_string(),
        "## adjustments (rows: 0)\nn: date32\n\n\
         ## claims (rows: 2)\nid: binary | charge: decimal128(38, 2)\nA1 | -12.50\n∅ | ∅\n\n"
    );
    assert_eq!(Tables::default().to_string(), "");
}

#[test]
fn taking_rows_leaves_an_empty_table_with_the_same_columns() {
    let mut table = Table::new("t", [("n".to_string(), ColumnType::Int64 { scale: 2 })]);
    table.push_row(&[Cell::Int64(7)]).unwrap();
    let taken = table.take_rows();
    assert_eq!(taken.len(), 1);
    assert_eq!(taken.column("n").unwrap().get(0), Some(Cell::Int64(7)));
    assert!(table.is_empty());
    assert_eq!(
        table.column("n").unwrap().kind(),
        ColumnType::Int64 { scale: 2 }
    );
}

#[test]
fn tables_are_ordered_by_name() {
    let tables = Tables::new(vec![Table::new("services", []), Table::new("claims", [])]);
    let names: Vec<&str> = tables.iter().map(Table::name).collect();
    assert_eq!(names, vec!["claims", "services"]);
    assert_eq!(tables.get("services").map(Table::name), Some("services"));
    assert_eq!(tables.len(), 2);
}

#[test]
fn n_values_are_digits_with_an_optional_minus() {
    assert_eq!(parse_n(b"0"), Some(0));
    assert_eq!(parse_n(b"007"), Some(7));
    assert_eq!(parse_n(b"-5"), Some(-5));
    assert_eq!(parse_n(b"9223372036854775807"), Some(i64::MAX));
    assert_eq!(parse_n(b"-9223372036854775808"), Some(i64::MIN));
    for text in [
        &b""[..],
        b"-",
        b"+5",
        b"1.0",
        b" 1",
        b"1 ",
        b"1a",
        b"9223372036854775808",
    ] {
        assert_eq!(parse_n(text), None, "{:?}", String::from_utf8_lossy(text));
    }
}

#[test]
fn r_values_are_scaled_and_never_lose_a_decimal() {
    assert_eq!(parse_r(b"12.34", 2), Some(1234));
    assert_eq!(parse_r(b"12.3", 2), Some(1230));
    assert_eq!(parse_r(b"12", 2), Some(1200));
    assert_eq!(parse_r(b"-0.5", 2), Some(-50));
    assert_eq!(parse_r(b".5", 2), Some(50));
    assert_eq!(parse_r(b"5.", 2), Some(500));
    assert_eq!(parse_r(b"12", 0), Some(12));
    assert_eq!(
        parse_r(b"99999999999999999999.999999999999999999", 18),
        Some(99_999_999_999_999_999_999_999_999_999_999_999_999)
    );
    for (text, scale) in [
        (&b"12.345"[..], 2),
        (b"12.0", 0),
        (b"", 2),
        (b".", 2),
        (b"-", 2),
        (b"1.2.3", 2),
        (b"1e5", 2),
        (b" 1", 2),
        (b"1 ", 2),
        (b"+1", 2),
        (b"1,5", 2),
        (b"100000000000000000000", 18),
    ] {
        assert_eq!(
            parse_r(text, scale),
            None,
            "{:?} at scale {scale}",
            String::from_utf8_lossy(text)
        );
    }
}

#[test]
fn dt_values_are_real_calendar_dates() {
    assert_eq!(parse_dt(b"19700101"), Some(0));
    assert_eq!(parse_dt(b"700101"), Some(0));
    assert_eq!(parse_dt(b"19691231"), Some(-1));
    assert_eq!(parse_dt(b"20240229"), Some(19_782));
    assert_eq!(parse_dt(b"20000229"), Some(11_016));
    assert_eq!(parse_dt(b"491231"), parse_dt(b"20491231"));
    assert_eq!(parse_dt(b"500101"), parse_dt(b"19500101"));
    for text in [
        &b"20230229"[..],
        b"19000229",
        b"20241301",
        b"20240100",
        b"20240431",
        b"2024011",
        b"2024-01-01",
        b"",
        b"240229 ",
        b"00000101",
        b"00000000",
    ] {
        assert_eq!(parse_dt(text), None, "{:?}", String::from_utf8_lossy(text));
    }
}

#[test]
fn tm_values_are_seconds_since_midnight() {
    assert_eq!(parse_tm(b"0000"), Some(0));
    assert_eq!(parse_tm(b"1230"), Some(45_000));
    assert_eq!(parse_tm(b"123045"), Some(45_045));
    assert_eq!(parse_tm(b"1230459"), Some(45_045));
    assert_eq!(parse_tm(b"12304599"), Some(45_045));
    assert_eq!(parse_tm(b"235959"), Some(86_399));
    for text in [
        &b"2400"[..],
        b"1260",
        b"123060",
        b"123",
        b"12304",
        b"123045999",
        b"12a0",
        b"",
        b"12:30",
    ] {
        assert_eq!(parse_tm(text), None, "{:?}", String::from_utf8_lossy(text));
    }
}

proptest! {
    #[test]
    fn valid_n_values_round_trip(value in any::<i64>()) {
        prop_assert_eq!(parse_n(value.to_string().as_bytes()), Some(value));
    }

    #[test]
    fn valid_r_values_round_trip(
        scale in 0u8..=18,
        value in -(10i128.pow(30))..10i128.pow(30),
    ) {
        prop_assert_eq!(parse_r(format_r(value, scale).as_bytes(), scale), Some(value));
    }

    #[test]
    fn valid_dates_round_trip(year in 1i32..=9999, month in 1i32..=12, day in 1i32..=31) {
        prop_assume!(day <= days_in_month(year, month));
        let text = format!("{year:04}{month:02}{day:02}");
        let days = parse_dt(text.as_bytes());
        prop_assert!(days.is_some(), "{}", text);
        prop_assert_eq!(days.map(civil_from_days), Some((year, month, day)));
    }

    #[test]
    fn valid_times_round_trip(seconds in 0i32..86_400) {
        let text = format!(
            "{:02}{:02}{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        );
        prop_assert_eq!(parse_tm(text.as_bytes()), Some(seconds));
    }

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..24), scale in 0u8..=40) {
        let _ = parse_n(&bytes);
        let _ = parse_r(&bytes, scale);
        let _ = parse_dt(&bytes);
        let _ = parse_tm(&bytes);
    }

    #[test]
    fn rows_read_back_with_their_validity(
        rows in proptest::collection::vec(
            (
                proptest::option::of(proptest::collection::vec(any::<u8>(), 0..8)),
                proptest::option::of(any::<i64>()),
                proptest::option::of(any::<i128>()),
                proptest::option::of(any::<i32>()),
            ),
            0..40,
        )
    ) {
        let mut table = Table::new(
            "t",
            [
                ("b".to_string(), ColumnType::Binary),
                ("n".to_string(), ColumnType::Int64 { scale: 0 }),
                ("r".to_string(), ColumnType::Decimal128 { precision: 38, scale: 2 }),
                ("d".to_string(), ColumnType::Date32),
                ("t".to_string(), ColumnType::Time32),
            ],
        );
        for (b, n, r, d) in &rows {
            let cells = [
                b.as_deref().map_or(Cell::Null, Cell::Binary),
                n.map_or(Cell::Null, Cell::Int64),
                r.map_or(Cell::Null, Cell::Decimal128),
                d.map_or(Cell::Null, Cell::Date32),
                d.map_or(Cell::Null, Cell::Time32),
            ];
            prop_assert!(table.push_row(&cells).is_ok());
        }
        prop_assert_eq!(table.len(), rows.len());
        for (i, (b, n, r, d)) in rows.iter().enumerate() {
            let cell = |name: &str| table.column(name).and_then(|column| column.get(i));
            prop_assert_eq!(cell("b"), Some(b.as_deref().map_or(Cell::Null, Cell::Binary)));
            prop_assert_eq!(cell("n"), Some(n.map_or(Cell::Null, Cell::Int64)));
            prop_assert_eq!(cell("r"), Some(r.map_or(Cell::Null, Cell::Decimal128)));
            prop_assert_eq!(cell("d"), Some(d.map_or(Cell::Null, Cell::Date32)));
            prop_assert_eq!(cell("t"), Some(d.map_or(Cell::Null, Cell::Time32)));
            prop_assert_eq!(
                table.column("b").and_then(|column| column.validity().get(i)),
                Some(b.is_some())
            );
        }
        for (_, column) in table.columns() {
            prop_assert_eq!(column.len(), rows.len());
            prop_assert_eq!(column.validity().as_bytes().len(), rows.len().div_ceil(8));
        }
    }
}

proptest! {
    #[test]
    fn is_dt_accepts_exactly_what_parse_dt_parses(
        text in proptest::collection::vec(proptest::sample::select(b"0123459-".to_vec()), 0..10),
    ) {
        prop_assert_eq!(is_dt(&text), parse_dt(&text).is_some());
    }

    #[test]
    fn is_dt_accepts_every_valid_date(year in 1i32..=9999, month in 1i32..=12, day in 1i32..=31) {
        let text = format!("{year:04}{month:02}{day:02}");
        prop_assert_eq!(is_dt(text.as_bytes()), parse_dt(text.as_bytes()).is_some());
    }
}
