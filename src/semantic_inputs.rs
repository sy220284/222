//! Schema-hidden normalization of model-generated JSON inputs.
//! Mirrors source semanticBoolean.ts and semanticNumber.ts; an invalid value is
//! passed through and is rejected by the tool's declared type (not JS truthiness).
use anyhow::{bail, Result};
use serde_json::{Number, Value};

pub fn boolean(value: Value) -> Value {
    match value {
        Value::String(v) if v == "true" => Value::Bool(true),
        Value::String(v) if v == "false" => Value::Bool(false),
        other => other,
    }
}

/// Original decimal grammar: ^-?\d+(\.\d+)?$
/// Exponent notation, leading plus, whitespace, NaN and Infinity are not coerced.
/// Reconstruct a JSON number from an IEEE-754 value, matching the Number()
/// conversion performed by the original TypeScript/Zod preprocessor. Parsing
/// directly as i64/u64 first would silently retain precision that JS loses.
fn js_number_value(n: f64) -> Value {
    if n == 0.0 && n.is_sign_negative() {
        return Value::Number(Number::from_f64(n).expect("finite negative zero"));
    }
    if n.fract() == 0.0 {
        // The upper bound is exclusive: 2^64 cannot fit in u64.
        if n >= 0.0 && n < 18_446_744_073_709_551_616.0 {
            return Value::Number((n as u64).into());
        }
        if n < 0.0 && n >= -9_223_372_036_854_775_808.0 {
            return Value::Number((n as i64).into());
        }
    }
    Value::Number(Number::from_f64(n).expect("finite JS number"))
}

pub fn number(value: Value) -> Value {
    let Value::String(v) = value else {
        return value;
    };
    let body = v.strip_prefix('-').unwrap_or(&v);
    let mut parts = body.split('.');
    let whole = parts.next().unwrap_or("");
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return Value::String(v);
    }
    if let Some(fraction) = parts.next() {
        if fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return Value::String(v);
        }
    }
    if parts.next().is_some() {
        return Value::String(v);
    }
    let Some(n) = v.parse::<f64>().ok().filter(|n| n.is_finite()) else {
        return Value::String(v);
    };
    js_number_value(n)
}

/// Applies the two source preprocessors only to declared built-in tool fields.
/// Number fields here are integer-typed; fractional/negative values are rejected
/// by the field's actual schema instead of falling back to default values.
pub fn normalize_tool_fields(tool: &str, args: &mut Value) -> Result<()> {
    let bool_fields: &[&str] = match tool {
        "run_command" => &["run_in_background"],
        "write_file" => &["create_only"],
        "edit_file" => &["replace_all"],
        "task_output" => &["block"],
        "git_diff" => &["staged"],
        _ => &[],
    };
    let integer_fields: &[&str] = match tool {
        "read_file" => &["start_line", "line_count"],
        "list_files" => &["max_depth", "limit"],
        "search_text" => &["limit"],
        "git_log" => &["count"],
        "run_command" => &["timeout_seconds"],
        "task_output" => &["timeout"],
        _ => &[],
    };
    let Some(map) = args.as_object_mut() else {
        return Ok(());
    };
    for &key in bool_fields {
        if let Some(value) = map.get_mut(key) {
            *value = boolean(value.take());
            if !value.is_boolean() {
                bail!("{tool}.{key} must be a boolean")
            }
        }
    }
    for &key in integer_fields {
        if let Some(value) = map.get_mut(key) {
            let raw = value.take();
            *value = match raw {
                // JSON numeric literals are already IEEE-754 Numbers in the
                // original JS runtime, even when they are not quoted.
                Value::Number(n) => n.as_f64().map(js_number_value).unwrap_or(Value::Number(n)),
                other => number(other),
            };
            // Zod accepts -0 as an integer; convert it to the unsigned
            // representation required by the Rust tool schemas.
            if value.as_f64() == Some(0.0) {
                *value = Value::Number(0u64.into());
            }
            if value.as_u64().is_none() {
                bail!("{tool}.{key} must be a nonnegative integer")
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn semantic_boolean_accepts_only_exact_boolean_strings() {
        assert_eq!(boolean(json!("true")), json!(true));
        assert_eq!(boolean(json!("false")), json!(false));
        assert_eq!(boolean(json!("False")), json!("False"));
        assert_eq!(boolean(json!(" true")), json!(" true"));
        assert_eq!(boolean(json!(0)), json!(0));
        assert_eq!(boolean(Value::Null), Value::Null);
    }
    #[test]
    fn semantic_number_uses_original_decimal_grammar_and_finiteness() {
        for (input, expected) in [
            ("30", 30.0),
            ("-5", -5.0),
            ("3.14", 314.0 / 100.0),
            ("00", 0.0),
            ("-0", 0.0),
            ("0004.025", 4.025),
        ] {
            assert_eq!(number(json!(input)).as_f64(), Some(expected), "{input}");
        }
        for input in [
            "", "-", "+1", "1.", " .5", ".5", "1e3", "0x10", "Infinity", " 30", "30 ", "--3",
            "1.2.3",
        ] {
            assert_eq!(number(json!(input)), json!(input), "{input}");
        }
        let enormous = "9".repeat(1000);
        assert_eq!(number(json!(enormous)), json!(enormous));
        assert_eq!(number(json!(null)), Value::Null);
    }
    #[test]
    fn js_number_precision_and_negative_zero_match_original_preprocessor() {
        // Number("9007199254740993") rounds to 9007199254740992 in JS.
        assert_eq!(
            number(json!("9007199254740993")).as_u64(),
            Some(9_007_199_254_740_992)
        );
        assert_eq!(
            number(json!("-9007199254740993")).as_i64(),
            Some(-9_007_199_254_740_992)
        );
        for input in ["-0", "-0.0"] {
            let value = number(json!(input));
            assert_eq!(value.as_f64(), Some(0.0));
            assert!(value.as_f64().unwrap().is_sign_negative(), "{input}");
            let mut args = json!({"timeout_seconds":input});
            normalize_tool_fields("run_command", &mut args).unwrap();
            assert_eq!(args["timeout_seconds"], json!(0));
        }
        // Direct JSON numeric literals undergo the same JS precision rounding.
        let mut args = json!({"timeout_seconds":9_007_199_254_740_993u64});
        normalize_tool_fields("run_command", &mut args).unwrap();
        assert_eq!(
            args["timeout_seconds"].as_u64(),
            Some(9_007_199_254_740_992)
        );
        let mut args = json!({"timeout_seconds":30.0});
        normalize_tool_fields("run_command", &mut args).unwrap();
        assert_eq!(args["timeout_seconds"], json!(30));
        // JS Number() rounds this to 2^64; it cannot be an unsigned Rust int.
        let mut args = json!({"timeout_seconds":"18446744073709551615"});
        assert!(normalize_tool_fields("run_command", &mut args).is_err());
    }
    #[test]
    fn rejects_invalid_fields_instead_of_using_default_and_does_not_coerce_unknowns() {
        let mut args =
            json!({"timeout_seconds":"30","run_in_background":"false","description":"30"});
        normalize_tool_fields("run_command", &mut args).unwrap();
        assert_eq!(args["timeout_seconds"].as_u64(), Some(30));
        assert_eq!(args["run_in_background"], false);
        assert_eq!(args["description"], "30");
        for bad in ["NaN", "-2", "2.5", "", "1e6", "true"] {
            let mut a = json!({"timeout_seconds":bad,"command":"echo x"});
            assert!(
                normalize_tool_fields("run_command", &mut a).is_err(),
                "{bad}"
            );
        }
        let mut a = json!({"replace_all":"false"});
        normalize_tool_fields("edit_file", &mut a).unwrap();
        assert_eq!(a["replace_all"], false);
        let mut a = json!({"replace_all":"FALSE"});
        assert!(normalize_tool_fields("edit_file", &mut a).is_err());
    }
}
