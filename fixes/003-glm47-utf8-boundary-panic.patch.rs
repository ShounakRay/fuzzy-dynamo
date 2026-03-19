// Fix for Bug 3: GLM-4.7 parser panics on multibyte UTF-8 function names with leading whitespace
// File: lib/parsers/src/tool_calling/xml/glm47_parser.rs
// Severity: HIGH
//
// Problem: `content[function_name.len()..]` uses the byte length of the *trimmed* function name
//          to index into the *untrimmed* content string, landing inside a multibyte UTF-8 char.
// Fix: Save the byte position from content.find() and reuse it for slicing args_section,
//      instead of using function_name.len() which reflects the trimmed string.

// === ORIGINAL (lines 218-232) ===
//     let arg_key_start = &config.arg_key_start;
//     let function_name = if let Some(pos) = content.find(arg_key_start.as_str()) {
//         content[..pos].trim().to_string()
//     } else {
//         // No arguments, just function name
//         content.trim().to_string()
//     };
//
//     if function_name.is_empty() {
//         anyhow::bail!("Empty function name in tool call");
//     }
//
//     // Parse key-value pairs
//     let mut arguments = HashMap::new();
//     let args_section = &content[function_name.len()..];

// === FIXED ===
    let arg_key_start = &config.arg_key_start;
    let arg_key_pos = content.find(arg_key_start.as_str());
    let function_name = if let Some(pos) = arg_key_pos {
        content[..pos].trim().to_string()
    } else {
        // No arguments, just function name
        content.trim().to_string()
    };

    if function_name.is_empty() {
        anyhow::bail!("Empty function name in tool call");
    }

    // Parse key-value pairs
    let mut arguments = HashMap::new();
    let args_section = if let Some(pos) = arg_key_pos {
        &content[pos..]
    } else {
        ""
    };

// === TEST ===
#[test]
fn test_glm47_multibyte_utf8_no_panic() {
    // Cyrillic 'ш' is 2 bytes in UTF-8. With leading whitespace, the trimmed
    // function_name.len() no longer corresponds to a valid byte offset in content.
    let config = Glm47ParserConfig::default();
    let input = "<tool_call>  .ш\x18\n<arg_key>location</arg_key><arg_value>NYC</arg_value></tool_call>";
    // Must not panic with "byte index is not a char boundary"
    let result = try_tool_call_parse_glm47(input, &config, None);
    assert!(result.is_ok());
}

#[test]
fn test_glm47_cjk_function_name_with_whitespace() {
    let config = Glm47ParserConfig::default();
    let input = "<tool_call> 获取<arg_key>k</arg_key><arg_value>v</arg_value></tool_call>";
    let result = try_tool_call_parse_glm47(input, &config, None);
    assert!(result.is_ok());
    let (calls, _) = result.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "获取");
}
