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
