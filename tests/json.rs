// Hand-authored cases for RFC 8259 and the profile in design/06-agent-interface.md.
use raipixel::Work;
use raipixel::json::{self, ErrorKind, Limits, Value};

fn parse(text: &str) -> Value {
    json::parse(text.as_bytes(), Limits::default(), &mut Work::default()).unwrap()
}

#[test]
fn parses_all_value_types_and_preserves_array_order() {
    let Value::Object(object) = parse(r#"{"name":"hero","ops":[null,true,false,12,{},[]]}"#) else {
        panic!("object expected")
    };
    assert_eq!(object["name"], Value::String("hero".into()));
    let Value::Array(values) = &object["ops"] else {
        panic!("array expected")
    };
    assert_eq!(
        &values[..3],
        &[Value::Null, Value::Bool(true), Value::Bool(false)]
    );
    assert!(matches!(&values[3], Value::Number(n) if n.as_str() == "12"));
    assert_eq!(values.len(), 6);
}

#[test]
fn permits_scalar_roots_and_only_json_whitespace() {
    for text in ["null", "true", "false", "0", "\"pixel\"", " \r\n\t [ ] \n"] {
        parse(text);
    }
    for text in ["\u{a0}null", "null\u{b}", "null\u{c}"] {
        assert_eq!(
            json::parse(text.as_bytes(), Limits::default(), &mut Work::default())
                .unwrap_err()
                .code(),
            "E_JSON"
        );
    }
}

#[test]
fn decodes_escapes_utf8_and_surrogate_pairs() {
    assert_eq!(
        parse(r#""\"\\\/\b\f\n\r\t\u0000""#),
        Value::String("\"\\/\u{8}\u{c}\n\r\t\0".into())
    );
    assert_eq!(
        parse(r#""açã🗡\u0062\uD83D\uDE00""#),
        Value::String("açã🗡b😀".into())
    );
    assert_eq!(
        parse(r#""\udbff\udfff""#),
        Value::String("\u{10ffff}".into())
    );
}

#[test]
fn rejects_invalid_strings_and_unpaired_surrogates() {
    for text in [
        r#""\x""#,
        r#""\u123""#,
        r#""\uZZZZ""#,
        r#""\uD800""#,
        r#""\uDC00""#,
        r#""\uD800\u0061""#,
        "\"unterminated",
        "\"line\nbreak\"",
    ] {
        assert!(
            json::parse(text.as_bytes(), Limits::default(), &mut Work::default()).is_err(),
            "{text}"
        );
    }
    let err = json::parse(b"\"\xff\"", Limits::default(), &mut Work::default()).unwrap_err();
    assert_eq!(err.offset, 1);
    assert_eq!(err.code(), "E_JSON");
    assert!(
        json::parse(
            "\u{feff}{}".as_bytes(),
            Limits::default(),
            &mut Work::default()
        )
        .is_err()
    );
}

#[test]
fn rejects_duplicate_decoded_keys_at_the_second_key() {
    for text in [r#"{"a":1,"a":2}"#, r#"{"a":1,"\u0061":2}"#] {
        let err =
            json::parse(text.as_bytes(), Limits::default(), &mut Work::default()).unwrap_err();
        assert_eq!(err.kind, ErrorKind::DuplicateKey);
        assert_eq!(err.offset, 7);
        assert_eq!(err.code(), "E_JSON");
    }
    parse(r#"{"a":1,"nested":{"a":2}}"#);
}

#[test]
fn accepts_complete_number_grammar_without_float_rounding() {
    for text in [
        "0",
        "-0",
        "123",
        "-456",
        "0.25",
        "1e9",
        "1E+9",
        "1e-9",
        "-3.00e+12",
        "18446744073709551615",
        "1e99999999999999999999999",
    ] {
        let Value::Number(n) = parse(text) else {
            panic!("number expected")
        };
        assert_eq!(n.as_str(), text);
    }
}

#[test]
fn rejects_malformed_numbers_and_non_json_numeric_constants() {
    for text in [
        "+1",
        "01",
        "-01",
        ".1",
        "1.",
        "1e",
        "1e+",
        "--1",
        "NaN",
        "Infinity",
        "-Infinity",
        "0x10",
        "1_000",
    ] {
        assert!(
            json::parse(text.as_bytes(), Limits::default(), &mut Work::default()).is_err(),
            "{text}"
        );
    }
}

#[test]
fn rejects_missing_delimiters_trailing_commas_and_extra_documents() {
    for text in [
        "",
        "[",
        "{",
        "[1,]",
        r#"{"a":1,}"#,
        r#"{a:1}"#,
        r#"{"a" 1}"#,
        "[1 2]",
        "null true",
        "[true",
        "True",
        "/*comment*/[]",
    ] {
        assert!(
            json::parse(text.as_bytes(), Limits::default(), &mut Work::default()).is_err(),
            "{text}"
        );
    }
    let err = json::parse(b"[0,]", Limits::default(), &mut Work::default()).unwrap_err();
    assert_eq!(err.offset, 3);
    assert!(!err.to_string().is_empty());
}

#[test]
fn input_nesting_token_and_value_limits_are_inclusive() {
    let base = Limits::default();
    for (text, limits) in [
        ("null", Limits { bytes: 3, ..base }),
        ("[[]]", Limits { depth: 1, ..base }),
        (
            r#""abc""#,
            Limits {
                token_bytes: 2,
                ..base
            },
        ),
        (
            "123",
            Limits {
                token_bytes: 2,
                ..base
            },
        ),
        ("[null]", Limits { values: 1, ..base }),
        (
            r#"{"abc":null}"#,
            Limits {
                token_bytes: 2,
                ..base
            },
        ),
    ] {
        assert_eq!(
            json::parse(text.as_bytes(), limits, &mut Work::default())
                .unwrap_err()
                .code(),
            "E_LIMIT"
        );
    }
    assert_eq!(
        json::parse(
            b"null",
            Limits {
                bytes: 4,
                values: 1,
                depth: 0,
                ..base
            },
            &mut Work::default()
        )
        .unwrap(),
        Value::Null
    );
    assert!(json::parse(b"[[]]", Limits { depth: 2, ..base }, &mut Work::default()).is_ok());
    assert!(
        json::parse(
            br#""\u0061\u0062""#,
            Limits {
                token_bytes: 2,
                ..base
            },
            &mut Work::default()
        )
        .is_ok()
    );
    assert!(
        json::parse(
            "\"é\"".as_bytes(),
            Limits {
                token_bytes: 1,
                ..base
            },
            &mut Work::default()
        )
        .is_err()
    );
}

#[test]
fn rejects_raising_any_default_parser_ceiling() {
    let base = Limits::default();
    for limits in [
        Limits {
            bytes: base.bytes + 1,
            ..base
        },
        Limits {
            depth: base.depth + 1,
            ..base
        },
        Limits {
            token_bytes: base.token_bytes + 1,
            ..base
        },
        Limits {
            values: base.values + 1,
            ..base
        },
    ] {
        assert_eq!(
            json::parse(b"null", limits, &mut Work::default())
                .unwrap_err()
                .code(),
            "E_LIMIT"
        );
    }
}

#[test]
fn parsing_charges_input_bytes_and_value_visits() {
    let mut work = Work::new(5).unwrap();
    assert_eq!(
        json::parse(b"null", Limits::default(), &mut work).unwrap(),
        Value::Null
    );
    assert_eq!(work.remaining(), 0);
    assert_eq!(
        json::parse(b"null", Limits::default(), &mut work)
            .unwrap_err()
            .code(),
        "E_LIMIT"
    );
}

#[test]
fn parses_the_real_proposed_command_schema() {
    assert!(matches!(
        parse(include_str!("../docs/design/command-schema.json")),
        Value::Object(_)
    ));
}

#[test]
fn converts_decimal_and_exponent_numbers_to_exact_integers() {
    for (text, expected) in [
        ("0", Some(0)),
        ("-0", Some(0)),
        ("1.000", Some(1)),
        ("10e-1", Some(1)),
        ("0.1e1", Some(1)),
        ("12.34e2", Some(1234)),
        ("-3.0e2", Some(-300)),
        ("1.2", None),
        ("1e-1", None),
        ("1000e-4", None),
        ("1.00000000000000000001", None),
    ] {
        let Value::Number(n) = parse(text) else {
            panic!("number expected")
        };
        assert_eq!(n.as_i64(), expected, "{text}");
    }
    let text = format!("1{}e-50", "0".repeat(50));
    let Value::Number(n) = parse(&text) else {
        panic!("number expected")
    };
    assert_eq!(n.as_i64(), Some(1));
}

#[test]
fn integer_conversion_checks_signed_and_unsigned_ranges() {
    for (text, signed, unsigned) in [
        ("9223372036854775807", Some(i64::MAX), Some(i64::MAX as u64)),
        ("-9223372036854775808", Some(i64::MIN), None),
        ("9223372036854775808", None, Some(1u64 << 63)),
        ("-9223372036854775809", None, None),
        ("18446744073709551615", None, Some(u64::MAX)),
        ("18446744073709551616", None, None),
        ("1e99999999999999999999999", None, None),
        ("1e-99999999999999999999999", None, None),
        ("-0e99999999999999999999999", Some(0), Some(0)),
    ] {
        let Value::Number(n) = parse(text) else {
            panic!("number expected")
        };
        assert_eq!(n.as_i64(), signed, "{text}");
        assert_eq!(n.as_u64(), unsigned, "{text}");
    }
}

fn emit(value: &Value) -> Vec<u8> {
    json::emit(value, Limits::default(), &mut Work::default()).unwrap()
}

#[test]
fn emits_sorted_keys_ordered_arrays_and_terminal_newline() {
    let value = parse(r#"{"z":[],"a":[null,true,false,1.20e+2]}"#);
    assert_eq!(
        emit(&value),
        b"{\"a\":[null,true,false,1.20e+2],\"z\":[]}\n"
    );
}

#[test]
fn emission_escapes_controls_and_preserves_unicode() {
    let value = Value::String("\"\\\u{8}\u{c}\n\r\t\0\u{1f}/ação🗡".into());
    assert_eq!(
        String::from_utf8(emit(&value)).unwrap(),
        "\"\\\"\\\\\\b\\f\\n\\r\\t\\u0000\\u001F/ação🗡\"\n"
    );
    assert_eq!(
        json::parse(&emit(&value), Limits::default(), &mut Work::default()).unwrap(),
        value
    );
}

#[test]
fn parse_emit_preserves_the_real_schema_and_numbers() {
    let schema = parse(include_str!("../docs/design/command-schema.json"));
    assert_eq!(
        json::parse(&emit(&schema), Limits::default(), &mut Work::default()).unwrap(),
        schema
    );
    for text in [
        "-0",
        "18446744073709551615",
        "0.123456789012345678901",
        "1e99999999999999999999999",
    ] {
        let value = parse(text);
        assert_eq!(
            String::from_utf8(emit(&value)).unwrap(),
            format!("{text}\n")
        );
    }
}

#[test]
fn emission_enforces_the_same_limits_for_caller_built_values() {
    let base = Limits::default();
    let value = Value::Array(vec![Value::String("abc".into())]);
    for limits in [
        Limits { bytes: 3, ..base },
        Limits { depth: 0, ..base },
        Limits {
            token_bytes: 2,
            ..base
        },
        Limits { values: 1, ..base },
    ] {
        assert_eq!(
            json::emit(&value, limits, &mut Work::default())
                .unwrap_err()
                .code(),
            "E_LIMIT"
        );
    }
    assert!(
        json::emit(
            &Value::Null,
            Limits {
                bytes: 5,
                depth: 0,
                values: 1,
                ..base
            },
            &mut Work::default()
        )
        .is_ok()
    );
    assert!(
        json::emit(
            &Value::Null,
            Limits { bytes: 4, ..base },
            &mut Work::default()
        )
        .is_err()
    );
}

#[test]
fn emission_charges_value_visits_and_actual_output_bytes() {
    let mut work = Work::new(6).unwrap();
    assert_eq!(
        json::emit(&Value::Null, Limits::default(), &mut work).unwrap(),
        b"null\n"
    );
    assert_eq!(work.remaining(), 0);
    assert_eq!(
        json::emit(&Value::Null, Limits::default(), &mut work)
            .unwrap_err()
            .code(),
        "E_LIMIT"
    );
}
