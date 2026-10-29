//! Provider-neutral JavaScript-compatible value primitives for individually
//! ported source functions. These are Rust implementations, not a JS engine.
#![allow(dead_code)]
use serde_json::{json, Value};

pub fn property(value: &Value, key: &str) -> Value {
    if key == "length" {
        return match value {
            Value::Array(a) => json!(a.len()),
            Value::String(s) => json!(s.encode_utf16().count()),
            Value::Object(map) => map.get(key).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        };
    }
    value.get(key).cloned().unwrap_or(Value::Null)
}
pub fn index(value: &Value, key: &Value) -> Value {
    match value {
        Value::Object(map) => map.get(&to_string(key)).cloned().unwrap_or(Value::Null),
        Value::Array(a) => key
            .as_u64()
            .and_then(|i| a.get(i as usize))
            .cloned()
            .unwrap_or(Value::Null),
        Value::String(s) => key
            .as_u64()
            .and_then(|i| s.chars().nth(i as usize))
            .map(|c| json!(c.to_string()))
            .unwrap_or(Value::Null),
        _ => Value::Null,
    }
}
pub fn to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    to_string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0. && !x.is_nan()),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}
pub fn to_number(v: &Value) -> Value {
    match v {
        Value::Number(_) => v.clone(),
        Value::Null => json!(0),
        Value::Bool(b) => json!(if *b { 1 } else { 0 }),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                json!(0)
            } else {
                s.parse::<f64>()
                    .ok()
                    .filter(|x| x.is_finite())
                    .map_or(Value::Null, |x| json!(x))
            }
        }
        Value::Array(_) => to_number(&json!(to_string(v))),
        _ => Value::Null,
    }
}
pub fn negate(v: &Value) -> Value {
    to_number(v).as_f64().map_or(Value::Null, |x| json!(-x))
}
pub fn add(a: &Value, b: &Value) -> Value {
    // JavaScript ToPrimitive converts arrays and plain objects to strings
    // before deciding whether + means concatenation.
    if a.is_string()
        || b.is_string()
        || a.is_array()
        || b.is_array()
        || a.is_object()
        || b.is_object()
    {
        json!(format!("{}{}", to_string(a), to_string(b)))
    } else {
        numeric2(a, b, |a, b| a + b)
    }
}
pub fn subtract(a: &Value, b: &Value) -> Value {
    numeric2(a, b, |a, b| a - b)
}
pub fn multiply(a: &Value, b: &Value) -> Value {
    numeric2(a, b, |a, b| a * b)
}
pub fn divide(a: &Value, b: &Value) -> Value {
    numeric2(a, b, |a, b| a / b)
}
fn numeric2(a: &Value, b: &Value, fun: impl FnOnce(f64, f64) -> f64) -> Value {
    match (to_number(a).as_f64(), to_number(b).as_f64()) {
        (Some(a), Some(b)) => {
            let n = fun(a, b);
            if n.is_finite() {
                json!(n)
            } else {
                Value::Null
            }
        }
        _ => Value::Null,
    }
}
pub fn strict_equal(a: &Value, b: &Value) -> Value {
    json!(a == b)
}
pub fn strict_unequal(a: &Value, b: &Value) -> Value {
    json!(a != b)
}
pub fn gt(a: &Value, b: &Value) -> Value {
    rel(a, b, |a, b| a > b, |a, b| a > b)
}
pub fn lt(a: &Value, b: &Value) -> Value {
    rel(a, b, |a, b| a < b, |a, b| a < b)
}
pub fn gte(a: &Value, b: &Value) -> Value {
    rel(a, b, |a, b| a >= b, |a, b| a >= b)
}
pub fn lte(a: &Value, b: &Value) -> Value {
    rel(a, b, |a, b| a <= b, |a, b| a <= b)
}
fn rel(
    a: &Value,
    b: &Value,
    number_cmp: impl Fn(f64, f64) -> bool,
    string_cmp: impl Fn(&[u16], &[u16]) -> bool,
) -> Value {
    // Both string primitives compare lexicographically by UTF-16 code units,
    // not by numeric conversion (unlike arithmetic operators).
    if let (Some(a), Some(b)) = (a.as_str(), b.as_str()) {
        let aa = a.encode_utf16().collect::<Vec<_>>();
        let bb = b.encode_utf16().collect::<Vec<_>>();
        return json!(string_cmp(&aa, &bb));
    }
    let a = to_number(a);
    let b = to_number(b);
    match (a.as_f64(), b.as_f64()) {
        (Some(a), Some(b)) => json!(number_cmp(a, b)),
        _ => json!(false),
    }
}
pub fn and(a: &Value, b: &Value) -> Value {
    if truthy(a) {
        b.clone()
    } else {
        a.clone()
    }
}
pub fn or(a: &Value, b: &Value) -> Value {
    if truthy(a) {
        a.clone()
    } else {
        b.clone()
    }
}
pub fn nullish(a: &Value, b: &Value) -> Value {
    if a.is_null() {
        b.clone()
    } else {
        a.clone()
    }
}
pub fn concat(values: &[Value]) -> Value {
    json!(values.iter().map(to_string).collect::<String>())
}
pub fn call_method(v: &Value, method: &str, args: &[Value]) -> Value {
    let a = args.first().unwrap_or(&Value::Null);
    match method {
        "startsWith" => json!(v.as_str().unwrap_or("").starts_with(&to_string(a))),
        "endsWith" => json!(v.as_str().unwrap_or("").ends_with(&to_string(a))),
        "includes" => match v {
            Value::Array(xs) => json!(xs.contains(a)),
            Value::String(s) => json!(s.contains(&to_string(a))),
            _ => json!(false),
        },
        "indexOf" => match v {
            Value::Array(xs) => json!(xs
                .iter()
                .position(|x| x == a)
                .map(|i| i as i64)
                .unwrap_or(-1)),
            Value::String(s) => json!(s
                .find(&to_string(a))
                .map(|i| s[..i].encode_utf16().count() as i64)
                .unwrap_or(-1)),
            _ => json!(-1),
        },
        "trim" => json!(v.as_str().unwrap_or("").trim()),
        "trimStart" => json!(v.as_str().unwrap_or("").trim_start()),
        "trimEnd" => json!(v.as_str().unwrap_or("").trim_end()),
        "toLowerCase" => json!(v.as_str().unwrap_or("").to_lowercase()),
        "toUpperCase" => json!(v.as_str().unwrap_or("").to_uppercase()),
        "slice" | "substring" => {
            let n = match v {
                Value::Array(xs) if method == "slice" => xs.len() as i64,
                Value::String(s) => s.chars().count() as i64,
                _ => return Value::Null,
            };
            let to_index = |v: &Value, default: i64| {
                let i = to_number(v).as_i64().unwrap_or(default);
                if method == "slice" && i < 0 {
                    (n + i).max(0) as usize
                } else {
                    i.max(0).min(n) as usize
                }
            };
            let mut left = to_index(a, 0);
            let mut right = if args.len() > 1 {
                to_index(&args[1], n)
            } else {
                n as usize
            };
            if method == "substring" && left > right {
                std::mem::swap(&mut left, &mut right)
            }
            if left > right {
                left = right
            }
            match v {
                Value::Array(xs) => json!(xs[left..right].to_vec()),
                Value::String(s) => {
                    json!(s.chars().skip(left).take(right - left).collect::<String>())
                }
                _ => Value::Null,
            }
        }
        "split" => {
            let s = v.as_str().unwrap_or("");
            let sep = to_string(a);
            if args.is_empty() {
                json!([s])
            } else if sep.is_empty() {
                json!(s.chars().map(|c| c.to_string()).collect::<Vec<_>>())
            } else {
                json!(s.split(&sep).collect::<Vec<_>>())
            }
        }
        "join" => {
            let sep = if args.is_empty() {
                ",".into()
            } else {
                to_string(a)
            };
            match v {
                Value::Array(xs) => json!(xs
                    .iter()
                    .map(|x| if x.is_null() {
                        String::new()
                    } else {
                        to_string(x)
                    })
                    .collect::<Vec<_>>()
                    .join(&sep)),
                _ => Value::Null,
            }
        }
        "replace" => {
            let s = v.as_str().unwrap_or("");
            json!(s.replacen(
                &to_string(a),
                &to_string(args.get(1).unwrap_or(&Value::Null)),
                1
            ))
        }
        "replaceAll" => {
            let s = v.as_str().unwrap_or("");
            json!(s.replace(
                &to_string(a),
                &to_string(args.get(1).unwrap_or(&Value::Null))
            ))
        }
        "at" | "charAt" => {
            let s = v.as_str().unwrap_or("");
            let mut n = a.as_i64().unwrap_or(0);
            if n < 0 && method == "at" {
                n += s.chars().count() as i64
            }
            s.chars()
                .nth(n.max(0) as usize)
                .map(|c| json!(c.to_string()))
                .unwrap_or(if method == "charAt" {
                    json!("")
                } else {
                    Value::Null
                })
        }
        _ => Value::Null,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn useful_javascript_value_operators() {
        assert_eq!(add(&json!("a"), &json!(1)), json!("a1"));
        assert_eq!(and(&json!(""), &json!(true)), json!(""));
        assert_eq!(
            call_method(&json!("abc"), "slice", &[json!(1)]),
            json!("bc")
        );
        assert_eq!(property(&json!([1, 2]), "length"), json!(2));
        assert_eq!(property(&json!({"length":7}), "length"), json!(7));
        assert_eq!(to_number(&json!("")), json!(0));
        assert_eq!(to_number(&json!(["7"])), json!(7.0));
        assert_eq!(add(&json!(null), &json!(4)), json!(4.0));
        assert_eq!(negate(&json!("2")), json!(-2.0));
        assert_eq!(add(&json!([]), &json!(1)), json!("1"));
        assert_eq!(add(&json!([1, 2]), &json!(3)), json!("1,23"));
        assert_eq!(lt(&json!("apple"), &json!("banana")), json!(true));
        assert_eq!(lt(&json!("10"), &json!("2")), json!(true));
        assert_eq!(gt(&json!(true), &json!(0)), json!(true));
        assert_eq!(gte(&json!(null), &json!(0)), json!(true));
        assert_eq!(
            call_method(&json!([1, 2, 3]), "slice", &[json!(-2)]),
            json!([2, 3])
        );
        assert_eq!(
            call_method(&json!("😀abc"), "indexOf", &[json!("a")]),
            json!(2)
        );
    }
}
