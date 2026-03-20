// Fix for Bug 13: XML parser strip_quotes panics on single-character quote input
// File: lib/parsers/src/tool_calling/xml/parser.rs
// Severity: HIGH (listed as Medium in issue, but crash bug)
//
// Problem: strip_quotes("\"") matches both starts_with('"') and ends_with('"') on the
//          same single character, then slices &trimmed[1..0] which panics (begin > end).
// Fix: Add trimmed.len() >= 2 guard before the slice operation.

// === ORIGINAL (lines 19-28) ===
// fn strip_quotes(s: &str) -> &str {
//     let trimmed = s.trim();
//     if (trimmed.starts_with('"') && trimmed.ends_with('"'))
//         || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
//     {
//         &trimmed[1..trimmed.len() - 1]
//     } else {
//         trimmed
//     }
// }

// === FIXED ===
fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

// === TEST ===
#[test]
fn test_strip_quotes_single_quote_no_panic() {
    // Single double-quote: should return as-is, not panic
    assert_eq!(strip_quotes("\""), "\"");
    // Single single-quote: should return as-is, not panic
    assert_eq!(strip_quotes("'"), "'");
}

#[test]
fn test_strip_quotes_normal_cases_still_work() {
    assert_eq!(strip_quotes("\"hello\""), "hello");
    assert_eq!(strip_quotes("'world'"), "world");
    assert_eq!(strip_quotes("no_quotes"), "no_quotes");
    assert_eq!(strip_quotes("\"\""), "");  // empty quoted string
    assert_eq!(strip_quotes("''"), "");    // empty single-quoted string
}
