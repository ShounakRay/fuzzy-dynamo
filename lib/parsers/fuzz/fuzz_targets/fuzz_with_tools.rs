#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::Glm47ParserConfig;
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;

// Tool-definition-aware fuzz harness.
//
// All existing harnesses pass tools: None, meaning the convert_param_value()
// and coerce_value() code paths (type-aware parameter conversion) are never
// exercised. This harness generates ToolDefinition structs with various
// parameter types and passes them to XML, GLM47, and Kimi K2 parsers.
fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    let Ok(s) = std::str::from_utf8(&data[3..]) else { return };

    // Use first 3 bytes to construct tool definition parameters
    let type_selector = data[0];
    let name_selector = data[1];
    let multi_param = data[2];

    // Select parameter types based on fuzzer input
    let param_types: &[&str] = &[
        "string", "integer", "int", "int32", "int64", "uint",
        "number", "float", "float32", "float64", "double",
        "boolean", "bool", "binary",
        "object", "dict", "dictionary",
        "array", "arr", "list",
        "enum", "text", "varchar", "char",
        "long", "short", "unsigned",
        "null",
        "custom_type",
    ];

    let func_names = ["test_func", "get_weather", "search", "execute_bash"];
    let param_names = ["param1", "query", "location", "count", "enabled", "config", "tags", "data"];

    let func_name = func_names[name_selector as usize % func_names.len()];

    // Build tool definition with 1-4 parameters
    let num_params = ((multi_param % 4) + 1) as usize;
    let mut properties = serde_json::Map::new();

    for i in 0..num_params {
        let pname = param_names[i % param_names.len()];
        let ptype = param_types[(type_selector as usize + i) % param_types.len()];
        properties.insert(
            pname.to_string(),
            serde_json::json!({"type": ptype}),
        );
    }

    let tools = vec![ToolDefinition {
        name: func_name.to_string(),
        parameters: Some(serde_json::json!({
            "type": "object",
            "properties": properties,
        })),
    }];

    let tools_slice = Some(tools.as_slice());

    // Exercise XML parser with tool definitions (convert_param_value path)
    let _ = try_tool_call_parse_xml(s, &XmlParserConfig::default(), tools_slice);

    // Exercise GLM-4.7 parser with tool definitions (coerce_value path)
    let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), tools_slice);

    // Exercise Kimi K2 parser with tool definitions (JSON validation path)
    let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), tools_slice);
});
