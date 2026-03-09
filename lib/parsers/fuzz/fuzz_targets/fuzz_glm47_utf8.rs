#![no_main]
use libfuzzer_sys::fuzz_target;
use dynamo_parsers::config::Glm47ParserConfig;
use dynamo_parsers::xml::try_tool_call_parse_glm47;

/// Targeted fuzzer for the GLM-4.7 parser's byte offset calculation.
///
/// The parser extracts function names by trimming whitespace, then uses the
/// trimmed name's byte length to slice back into the original content. With
/// leading whitespace + multibyte UTF-8 function names, the byte offset
/// is wrong and can land in the middle of a multibyte character.
fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };

    let config = Glm47ParserConfig::default();

    // Direct parse — should not panic
    let result = std::panic::catch_unwind(|| {
        try_tool_call_parse_glm47(text, &config, None)
    });

    if let Err(panic) = result {
        // Re-panic to generate crash artifact
        let msg = if let Some(s) = panic.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = panic.downcast_ref::<&str>() {
            s.to_string()
        } else {
            "unknown panic".to_string()
        };
        panic!("GLM-4.7 parser panicked on input: {}", msg);
    }

    // Also try with a crafted input that has leading spaces + multibyte chars
    // This is the pattern most likely to trigger the byte offset bug
    if text.len() < 50 {
        let crafted = format!(
            "<tool_call>  {}<arg_key>location</arg_key><arg_value>NYC</arg_value></tool_call>",
            text
        );
        let result2 = std::panic::catch_unwind(|| {
            try_tool_call_parse_glm47(&crafted, &config, None)
        });
        if let Err(panic) = result2 {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown panic".to_string()
            };
            panic!(
                "GLM-4.7 parser panicked on crafted input with func_name='{}': {}",
                text, msg
            );
        }
    }
});
