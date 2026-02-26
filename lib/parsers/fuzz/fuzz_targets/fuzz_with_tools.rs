#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::*;
use dynamo_parsers::tool_calling::config::Glm47ParserConfig;
use dynamo_parsers::tool_calling::xml::try_tool_call_parse_glm47;

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    let Ok(s) = std::str::from_utf8(&data[3..]) else { return };

    let param_types: &[&str] = &[
        "string", "integer", "int", "int32", "int64", "uint",
        "number", "float", "float32", "float64", "double",
        "boolean", "bool", "binary", "object", "dict", "dictionary",
        "array", "arr", "list", "enum", "text", "varchar", "char",
        "long", "short", "unsigned", "null", "custom_type",
    ];
    let func_names = ["test_func", "get_weather", "search", "execute_bash"];
    let param_names = ["param1", "query", "location", "count", "enabled", "config", "tags", "data"];

    let func_name = func_names[data[1] as usize % func_names.len()];
    let num_params = ((data[2] % 4) + 1) as usize;
    let mut properties = serde_json::Map::new();
    for i in 0..num_params {
        let ptype = param_types[(data[0] as usize + i) % param_types.len()];
        properties.insert(param_names[i % param_names.len()].into(), serde_json::json!({"type": ptype}));
    }

    let tools = vec![ToolDefinition {
        name: func_name.into(),
        parameters: Some(serde_json::json!({"type": "object", "properties": properties})),
    }];
    let tools_slice = Some(tools.as_slice());

    let _ = try_tool_call_parse_xml(s, &XmlParserConfig::default(), tools_slice);
    let _ = try_tool_call_parse_glm47(s, &Glm47ParserConfig::default(), tools_slice);
    let _ = try_tool_call_parse_kimi_k2(s, &KimiK2ParserConfig::default(), tools_slice);
});
