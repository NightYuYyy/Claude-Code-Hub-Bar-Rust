//! Lenient JSON coercion helpers, ported from the free functions at the bottom
//! of `APIService.swift` (`stringValue`, `intValue`, `doubleValue`, `boolValue`,
//! `itemRows`, etc.). The CCH backend is inconsistent about numeric vs string
//! encodings and array vs object wrapping, so every read goes through these.

use serde_json::{Map, Value};

/// Coerce a JSON value to a string. Numbers become their string form; anything
/// else falls back to `fallback`.
pub fn string_value(value: Option<&Value>, fallback: &str) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => fallback.to_string(),
    }
}

/// First non-empty trimmed string among `keys`, else `fallback`.
pub fn first_string_value(row: &Map<String, Value>, keys: &[&str], fallback: &str) -> String {
    for key in keys {
        let value = string_value(row.get(*key), "");
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    fallback.to_string()
}

/// Coerce a JSON value to an `i64`. Strings are parsed as floats then truncated
/// (matching `Int(Double(s) ?? Double(fallback))`).
pub fn int_value(value: Option<&Value>, fallback: i64) -> i64 {
    match value {
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i
            } else if let Some(f) = n.as_f64() {
                f as i64
            } else {
                fallback
            }
        }
        Some(Value::String(s)) => s.parse::<f64>().map(|f| f as i64).unwrap_or(fallback),
        Some(Value::Bool(b)) => i64::from(*b),
        _ => fallback,
    }
}

/// `None` for missing/null, else coerced `i64`.
pub fn optional_int(value: Option<&Value>) -> Option<i64> {
    match value {
        None | Some(Value::Null) => None,
        v => Some(int_value(v, 0)),
    }
}

pub fn first_optional_int(row: &Map<String, Value>, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|k| optional_int(row.get(*k)))
}

/// Coerce a JSON value to an `f64`.
pub fn double_value(value: Option<&Value>, fallback: f64) -> f64 {
    match value {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(fallback),
        Some(Value::String(s)) => s.parse::<f64>().unwrap_or(fallback),
        Some(Value::Bool(b)) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => fallback,
    }
}

pub fn optional_double(value: Option<&Value>) -> Option<f64> {
    match value {
        None | Some(Value::Null) => None,
        v => Some(double_value(v, 0.0)),
    }
}

pub fn first_optional_double(row: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| optional_double(row.get(*k)))
}

/// Coerce a JSON value to a bool. Accepts `"true"`, `"1"`, `"yes"` for strings.
pub fn bool_value(value: Option<&Value>, fallback: bool) -> bool {
    match value {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(fallback),
        Some(Value::String(s)) => {
            matches!(s.to_lowercase().as_str(), "true" | "1" | "yes")
        }
        _ => fallback,
    }
}

/// Extract a row array from a value that may be a bare array, or an object that
/// wraps the array under `items`/`logs`/`data`/`groups`, possibly nested under
/// `data`. Returns owned objects (only objects survive).
pub fn item_rows(value: &Value) -> Vec<Map<String, Value>> {
    fn objects_from_array(arr: &[Value]) -> Vec<Map<String, Value>> {
        arr.iter()
            .filter_map(|v| v.as_object().cloned())
            .collect()
    }

    if let Some(arr) = value.as_array() {
        return objects_from_array(arr);
    }
    let Some(dict) = value.as_object() else {
        return Vec::new();
    };
    for key in ["items", "logs", "data", "groups"] {
        if let Some(arr) = dict.get(key).and_then(Value::as_array) {
            return objects_from_array(arr);
        }
    }
    if let Some(data) = dict.get("data") {
        if data.is_object() {
            return item_rows(data);
        }
    }
    Vec::new()
}

/// Total pages from a usage-logs response (`pageInfo.totalPages`, min 1).
pub fn usage_log_total_pages(value: &Value) -> i64 {
    let Some(dict) = value.as_object() else {
        return 1;
    };
    let page_info = dict.get("pageInfo").and_then(Value::as_object);
    let total = page_info
        .and_then(|p| p.get("totalPages"))
        .map(Some)
        .map(|v| int_value(v, 1))
        .unwrap_or(1);
    total.max(1)
}

/// Extract a list of string rows from a value that may be a bare string array
/// or wrap one under `items`/`groups`/`data`.
pub fn provider_group_string_rows(value: &Value) -> Option<Vec<String>> {
    fn as_string_array(v: &Value) -> Option<Vec<String>> {
        let arr = v.as_array()?;
        if arr.iter().all(Value::is_string) {
            Some(
                arr.iter()
                    .map(|s| s.as_str().unwrap_or_default().to_string())
                    .collect(),
            )
        } else {
            None
        }
    }

    if let Some(rows) = as_string_array(value) {
        return Some(rows);
    }
    let dict = value.as_object()?;
    for key in ["items", "groups", "data"] {
        if let Some(rows) = dict.get(key).and_then(as_string_array) {
            return Some(rows);
        }
    }
    if let Some(data) = dict.get("data") {
        if data.is_object() {
            return provider_group_string_rows(data);
        }
    }
    None
}

/// Compact summary of an "array-ish" field for provider rows, matching
/// `compactArrayDescription`.
pub fn compact_array_description(value: Option<&Value>) -> String {
    match value {
        Some(Value::Array(arr)) if arr.iter().all(Value::is_string) => {
            if arr.is_empty() {
                "all".to_string()
            } else {
                arr.iter()
                    .take(4)
                    .map(|s| s.as_str().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        Some(Value::Array(arr)) if arr.iter().all(Value::is_object) => {
            if arr.is_empty() {
                "none".to_string()
            } else {
                format!("{} rules", arr.len())
            }
        }
        Some(Value::Array(arr)) => {
            if arr.is_empty() {
                "none".to_string()
            } else {
                format!("{} items", arr.len())
            }
        }
        _ => "none".to_string(),
    }
}
