// Fix for Bug 7: DSML parser silently drops parameters with capitalized or missing string attribute
// File: lib/parsers/src/tool_calling/dsml/parser.rs
// Severity: MEDIUM
//
// Problem: The parameter regex requires `string="true|false"` (lowercase, mandatory).
//   Parameters with `string="True"` or no `string` attribute silently vanish.
// Fix: Make the string attribute case-insensitive and optional. When omitted,
//   default to non-string (attempt JSON parse, fall back to string).

// === ORIGINAL (lines 173-176 in parse_parameters) ===
//     let param_pattern = format!(
//         r#"(?s){}\"([^"]+)\"\s+string=\"(true|false)\"\s*>(.*?){}"#,
//         prefix_escaped, end_escaped
//     );

// === FIXED ===
fn parse_parameters(
    content: &str,
    config: &DsmlParserConfig,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut parameters = serde_json::Map::new();

    let prefix_escaped = regex::escape(&config.parameter_prefix);
    let end_escaped = regex::escape(&config.parameter_end);

    // Make string attribute case-insensitive and optional.
    // Group 1: param name
    // Group 2: optional string attribute value (true/false, any case)
    // Group 3: param value
    let param_pattern = format!(
        r#"(?si){}\"([^"]+)\"(?:\s+string=\"(true|false)\")?\s*>(.*?){}"#,
        prefix_escaped, end_escaped
    );

    let param_regex = Regex::new(&param_pattern)?;

    for param_match in param_regex.captures_iter(content) {
        if let (Some(name_match), Some(value_match)) =
            (param_match.get(1), param_match.get(3))
        {
            let param_name = name_match.as_str().trim();
            let param_value = value_match.as_str().trim();

            // Determine if value is a string type:
            // - Explicit string="true"/"True"/etc. -> string
            // - Explicit string="false"/"False"/etc. -> non-string (JSON parse)
            // - Omitted -> non-string (JSON parse, fall back to string)
            let is_string = param_match
                .get(2)
                .map(|m| m.as_str().eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let value = if is_string {
                serde_json::Value::String(param_value.to_string())
            } else {
                serde_json::from_str(param_value).unwrap_or_else(|_| {
                    serde_json::Value::String(param_value.to_string())
                })
            };

            parameters.insert(param_name.to_string(), value);
        }
    }

    Ok(parameters)
}

// === TEST ===
#[test]
fn test_parse_parameter_with_capitalized_string_attr() {
    let config = DsmlParserConfig::default();
    let input = concat!(
        "<\u{ff5c}DSML\u{ff5c}function_calls>\n",
        "<\u{ff5c}DSML\u{ff5c}invoke name=\"test\">\n",
        "<\u{ff5c}DSML\u{ff5c}parameter name=\"query\" string=\"True\">hello world</\u{ff5c}DSML\u{ff5c}parameter>\n",
        "</\u{ff5c}DSML\u{ff5c}invoke>\n",
        "</\u{ff5c}DSML\u{ff5c}function_calls>"
    );
    let (calls, _) = try_tool_call_parse_dsml(input, &config).unwrap();
    assert_eq!(calls.len(), 1);
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
    assert_eq!(args["query"], serde_json::Value::String("hello world".into()));
}

#[test]
fn test_parse_parameter_without_string_attr() {
    let config = DsmlParserConfig::default();
    let input = concat!(
        "<\u{ff5c}DSML\u{ff5c}function_calls>\n",
        "<\u{ff5c}DSML\u{ff5c}invoke name=\"test\">\n",
        "<\u{ff5c}DSML\u{ff5c}parameter name=\"count\">42</\u{ff5c}DSML\u{ff5c}parameter>\n",
        "</\u{ff5c}DSML\u{ff5c}invoke>\n",
        "</\u{ff5c}DSML\u{ff5c}function_calls>"
    );
    let (calls, _) = try_tool_call_parse_dsml(input, &config).unwrap();
    assert_eq!(calls.len(), 1);
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
    // Without string attr, "42" is parsed as JSON number
    assert_eq!(args["count"], serde_json::json!(42));
}
