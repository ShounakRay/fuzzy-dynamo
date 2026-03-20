// Fix for Bug 14: XML parser try_literal_eval corrupts values containing Python keywords
// File: lib/parsers/src/tool_calling/xml/parser.rs
// Severity: HIGH
//
// Problem: try_literal_eval uses global `.replace("True", "true")` etc., which
// corrupts argument values containing these substrings. For example,
// "TrueNorth" becomes "trueNorth" and "NoneAvailable" becomes "nullAvailable".
//
// Fix: Use word-boundary-aware regex replacements so only standalone Python
// keywords (True, False, None) are replaced, not substrings within larger words.

// === ORIGINAL (try_literal_eval, lines ~490-497) ===
// fn try_literal_eval(s: &str) -> Result<Value, ()> {
//     if let Ok(val) = serde_json::from_str::<Value>(s) {
//         return Ok(val);
//     }
//     let normalized = s
//         .replace('\'', "\"")
//         .replace("True", "true")
//         .replace("False", "false")
//         .replace("None", "null");
//     serde_json::from_str::<Value>(&normalized).map_err(|_| ())
// }

// === FIXED ===
use regex::Regex;
use std::sync::OnceLock;

static TRUE_RE: OnceLock<Regex> = OnceLock::new();
static FALSE_RE: OnceLock<Regex> = OnceLock::new();
static NONE_RE: OnceLock<Regex> = OnceLock::new();

fn try_literal_eval(s: &str) -> Result<Value, ()> {
    // First try standard JSON
    if let Ok(val) = serde_json::from_str::<Value>(s) {
        return Ok(val);
    }

    // Try to handle Python-style literals (single quotes, True/False/None)
    // FIX: Use word-boundary-aware regex so "TrueNorth" is NOT corrupted to "trueNorth".
    let normalized = s.replace('\'', "\"");

    let true_re = TRUE_RE.get_or_init(|| Regex::new(r"\bTrue\b").unwrap());
    let false_re = FALSE_RE.get_or_init(|| Regex::new(r"\bFalse\b").unwrap());
    let none_re = NONE_RE.get_or_init(|| Regex::new(r"\bNone\b").unwrap());

    let normalized = true_re.replace_all(&normalized, "true");
    let normalized = false_re.replace_all(&normalized, "false");
    let normalized = none_re.replace_all(&normalized, "null");

    serde_json::from_str::<Value>(&normalized).map_err(|_| ())
}

// === TEST ===
#[test]
fn test_try_literal_eval_preserves_true_in_strings() {
    // Regression test for Bug 14: "TrueNorth" must NOT become "trueNorth"
    let input = r#"{'destination': 'TrueNorth'}"#;
    let result = try_literal_eval(input).unwrap();
    assert_eq!(
        result["destination"].as_str().unwrap(),
        "TrueNorth",
        "try_literal_eval corrupted 'TrueNorth' to '{}'",
        result["destination"]
    );
}

#[test]
fn test_try_literal_eval_preserves_false_in_strings() {
    let input = r#"{'claim': 'Falsehood'}"#;
    let result = try_literal_eval(input).unwrap();
    assert_eq!(
        result["claim"].as_str().unwrap(),
        "Falsehood",
        "try_literal_eval corrupted 'Falsehood'"
    );
}

#[test]
fn test_try_literal_eval_preserves_none_in_strings() {
    let input = r#"{'status': 'NoneAvailable'}"#;
    let result = try_literal_eval(input).unwrap();
    assert_eq!(
        result["status"].as_str().unwrap(),
        "NoneAvailable",
        "try_literal_eval corrupted 'NoneAvailable'"
    );
}

#[test]
fn test_try_literal_eval_still_converts_standalone_keywords() {
    // Standalone True/False/None should still be converted
    let input = r#"{'flag': True, 'other': False, 'empty': None}"#;
    let result = try_literal_eval(input).unwrap();
    assert_eq!(result["flag"], true);
    assert_eq!(result["other"], false);
    assert!(result["empty"].is_null());
}

#[test]
fn test_try_literal_eval_valid_json_passthrough() {
    // Already-valid JSON should pass through without any replacements
    let input = r#"{"destination": "TrueNorth"}"#;
    let result = try_literal_eval(input).unwrap();
    assert_eq!(result["destination"].as_str().unwrap(), "TrueNorth");
}
