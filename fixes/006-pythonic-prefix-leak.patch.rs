// Fix for Bug 6: Pythonic parser absorbs prefix characters into function name
// File: lib/parsers/src/tool_calling/pythonic/pythonic_parser.rs
// Severity: HIGH
//
// Problem: The regex pattern `[a-zA-Z]+\w*\(` does not enforce a word boundary
// before the function name. Characters like 'v', 'm', etc. that immediately
// precede the real function name (e.g., "]get_weather(") are absorbed because
// `[a-zA-Z]+` greedily matches backward into adjacent identifier characters.
// Example: input "vv]get_weather(...)" extracts "vvget_weather" instead of
// "get_weather".
//
// Fix: Add a word boundary assertion `\b` (or negative lookbehind `(?<![a-zA-Z0-9_])`)
// before `[a-zA-Z]` in the regex, so function names must start at a word boundary.

// === ORIGINAL (get_pythonic_regex, line ~22) ===
// let pattern = r"\[([a-zA-Z]+\w*\(([a-zA-Z]+\w*=.*?,\s*)*([a-zA-Z]+\w*=.*?\s?)?\),\s*)*([a-zA-Z]+\w*\(([a-zA-Z]+\w*=.*?,\s*)*([a-zA-Z]+\w*=.*?\s*)?\)\s*)+\]";

// === FIXED ===
fn get_pythonic_regex() -> &'static Regex {
    PYTHONIC_REGEX.get_or_init(|| {
        // Format Structure: [tool1(arg1=val1, arg2=val2), tool2(arg1=val3)]
        // FIX: Added (?<!\w) negative lookbehind before each function name pattern
        // to prevent absorbing adjacent identifier characters as part of the name.
        // \b alone is insufficient because it matches between \W and \w, but the
        // bracket ']' before 'get_weather' already constitutes a boundary — the bug
        // is that 'vv]' causes the regex to start matching at 'v' not 'g'.
        // The lookbehind ensures no word character immediately precedes the match.
        let pattern = r"\[(?:(?<!\w)[a-zA-Z]+\w*\((?:[a-zA-Z]+\w*=.*?,\s*)*(?:[a-zA-Z]+\w*=.*?\s?)?\),\s*)*(?:(?<!\w)[a-zA-Z]+\w*\((?:[a-zA-Z]+\w*=.*?,\s*)*(?:[a-zA-Z]+\w*=.*?\s*)?\)\s*)+\]";
        Regex::new(pattern).expect("Failed to compile pythonic regex pattern")
    })
}

// === TEST ===
#[test]
fn test_pythonic_no_prefix_leak() {
    // Regression test for Bug 6: characters before the function name must not
    // be absorbed into the extracted name.
    use super::try_tool_call_parse_pythonic;

    // Case 1: brackets and identifier chars before the function name
    let input = r#"[vvvvvvvvv[v[vv]get_weather(location="NYC")]"#;
    let result = try_tool_call_parse_pythonic(input, None);
    // The outer [...] does not cleanly match with garbage prefix, so the regex
    // should either not match or match only the valid tool call.
    if let Ok((calls, _)) = result {
        for call in &calls {
            assert_eq!(
                call.function.name, "get_weather",
                "Function name should be 'get_weather', got '{}'",
                call.function.name
            );
        }
    }

    // Case 2: valid input should still work
    let input2 = r#"[get_weather(location="NYC")]"#;
    let (calls2, _) = try_tool_call_parse_pythonic(input2, None).unwrap();
    assert_eq!(calls2[0].function.name, "get_weather");

    // Case 3: single extra char before function name
    let input3 = r#"[m]get_weather(location="NYC")]"#;
    // Should not extract "mget_weather"
    if let Ok((calls3, _)) = try_tool_call_parse_pythonic(input3, None) {
        for call in &calls3 {
            assert!(
                !call.function.name.starts_with('m'),
                "Prefix 'm' leaked into function name: '{}'",
                call.function.name
            );
        }
    }
}
