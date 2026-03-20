#![no_main]
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_xml;
use dynamo_parsers::tool_calling::config::XmlParserConfig;

/// Generate structured XML tool call inputs that reach deep parsing code.
/// The XML parser's inner functions (parse_tool_call_block, convert_param_value,
/// try_literal_eval, safe_parse_value) require valid XML structure to reach.
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    func_name: String,
    params: Vec<(String, String)>,
    /// Extra fuzz text injected around the tool call
    prefix: String,
    suffix: String,
    /// Whether to include tool definitions (for schema-aware parsing)
    with_schema: bool,
    /// Parameter type hints for schema
    param_types: Vec<u8>,
}

/// Filter strings that would trigger known bug #15 (strip_quotes panic on single quote char).
/// strip_quotes is called on function names and parameter names captured by regex.
fn is_safe_name(s: &str) -> bool {
    let t = s.trim();
    // Single quote chars trigger the bug
    !(t == "\"" || t == "'")
}

fuzz_target!(|input: FuzzInput| {
    let func_name = if input.func_name.is_empty() { "test_func" } else { &input.func_name };

    // Skip inputs that would trigger known bug #15 (strip_quotes)
    if !is_safe_name(func_name) {
        return;
    }

    let params: Vec<_> = input.params.iter().take(8).collect();
    for (name, _) in &params {
        if !name.is_empty() && !is_safe_name(name) {
            return;
        }
    }

    let config = XmlParserConfig::default();

    // Build a valid XML tool call structure with fuzzed content
    let mut xml = format!(
        "{}<tool_call><function={}>",
        &input.prefix,
        func_name
    );
    for (name, value) in &params {
        let pname = if name.is_empty() { "param" } else { name.as_str() };
        xml.push_str(&format!("<parameter={}>", pname));
        xml.push_str(value);
        xml.push_str("</parameter>");
    }
    xml.push_str("</function></tool_call>");
    xml.push_str(&input.suffix);

    // Parse without tool definitions
    let _ = try_tool_call_parse_xml(&xml, &config, None);

    // Parse with tool definitions to exercise schema-aware type conversion
    if input.with_schema && !params.is_empty() {
        let mut properties = serde_json::Map::new();
        for (i, (name, _)) in params.iter().enumerate() {
            let pname = if name.is_empty() { "param".to_string() } else { name.clone() };
            let type_idx = input.param_types.get(i).copied().unwrap_or(0);
            let type_str = match type_idx % 6 {
                0 => "string",
                1 => "integer",
                2 => "number",
                3 => "boolean",
                4 => "array",
                _ => "object",
            };
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), serde_json::Value::String(type_str.to_string()));
            properties.insert(pname, serde_json::Value::Object(prop));
        }

        let mut params_schema = serde_json::Map::new();
        params_schema.insert("type".to_string(), serde_json::Value::String("object".to_string()));
        params_schema.insert("properties".to_string(), serde_json::Value::Object(properties));

        let tool_def = dynamo_parsers::tool_calling::ToolDefinition {
            name: func_name.to_string(),
            parameters: Some(serde_json::Value::Object(params_schema)),
        };

        let _ = try_tool_call_parse_xml(&xml, &config, Some(&[tool_def]));
    }

    // Also test with Python-style values to exercise try_literal_eval
    let python_values = [
        "True", "False", "None",
        "{'key': 'value'}",
        "{'a': True, 'b': False, 'c': None}",
        "[1, 2, 3]",
        "{'nested': {'deep': True}}",
    ];
    for py_val in &python_values {
        let py_xml = format!(
            "<tool_call><function={}><parameter=val>{}</parameter></function></tool_call>",
            func_name, py_val
        );
        let _ = try_tool_call_parse_xml(&py_xml, &config, None);
    }
});
