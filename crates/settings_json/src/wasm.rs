use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::ops::Range;

pub fn update_value_in_json_text<'a>(
    text: &mut String,
    _key_path: &mut Vec<&'a str>,
    _tab_size: usize,
    old_value: &'a Value,
    new_value: &'a Value,
    edits: &mut Vec<(Range<usize>, String)>,
) {
    if old_value == new_value {
        return;
    }

    let replacement = serde_json::to_string_pretty(new_value).unwrap_or_else(|_| text.clone());
    let range = 0..text.len();
    *text = replacement.clone();
    edits.push((range, replacement));
}

pub fn replace_value_in_json_text<T: AsRef<str>>(
    text: &str,
    _key_path: &[T],
    _tab_size: usize,
    new_value: Option<&Value>,
    _replace_key: Option<&str>,
) -> (Range<usize>, String) {
    let replacement = new_value
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| text.to_string());
    (0..text.len(), replacement)
}

pub fn replace_top_level_array_value_in_json_text(
    text: &str,
    _key_path: &[impl AsRef<str>],
    new_value: Option<&Value>,
    _replace_key: Option<&str>,
    _array_index: usize,
    _tab_size: usize,
) -> (Range<usize>, String) {
    let replacement = new_value
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| text.to_string());
    (0..text.len(), replacement)
}

pub fn append_top_level_array_value_in_json_text(
    text: &str,
    new_value: &Value,
    _tab_size: usize,
) -> (Range<usize>, String) {
    let replacement = serde_json::to_string_pretty(new_value).unwrap_or_else(|_| text.to_string());
    (0..text.len(), replacement)
}

pub fn infer_json_indent_size(_text: &str) -> usize {
    2
}

pub fn to_pretty_json(
    value: &impl Serialize,
    _indent_size: usize,
    _indent_prefix_len: usize,
) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

pub fn parse_json_with_comments<T: DeserializeOwned>(content: &str) -> Result<T> {
    Ok(serde_json_lenient::from_str(content)?)
}

pub fn to_comment_aware_json<T: Serialize>(value: &T, _tab_size: usize) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}
