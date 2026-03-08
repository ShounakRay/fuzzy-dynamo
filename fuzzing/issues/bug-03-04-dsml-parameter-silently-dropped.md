### [BUG]: DSML parser silently drops parameters with capitalized `string="True"` or missing `string` attribute

### What This Bug Is (Plain English)

When an AI model makes a tool call (like "search the web for X"), its output gets parsed by dynamo to extract the function name and arguments. The DSML parser handles one specific format for this. Each argument has a tag that says whether the value is a string or not — something like `string="true"`.

The problem: the parser is too picky about the exact casing of that tag. If the model writes `string="True"` (capital T, which Python-trained models love to do) instead of `string="true"`, the parser doesn't recognize it and **silently throws away the entire argument**. No error, no warning — the data just vanishes. Same thing if the model skips the tag entirely.

This means a perfectly valid tool call like `search(query="weather in NYC")` could arrive at the tool with zero arguments, and nobody would know why.

### Describe the Bug

The DSML tool call parser in `lib/parsers/src/tool_calling/dsml/parser.rs` uses a regex (lines 173-176) that is too strict about the `string` attribute on `<parameter>` tags:

```rust
let param_pattern = format!(
    r#"(?s){}\"([^"]+)\"\s+string=\"(true|false)\"\s*>(.*?){}"#,
    prefix_escaped, end_escaped
);
```

Two problems:

1. **Case sensitivity**: The regex requires exact lowercase `string="true"` or `string="false"`. If a model emits `string="True"` (capitalized, which Python-trained models frequently produce), the regex won't match and the **entire parameter is silently dropped** with no error.

2. **Missing attribute**: The regex requires the `string` attribute to be present. If a model omits it entirely (e.g., `<｜DSML｜parameter name="count">42</｜DSML｜parameter>`), the parameter is silently dropped. The `string` attribute controls whether the value is treated as a JSON string or raw value, but omitting it is a reasonable model output.

### Steps to Reproduce

**Capitalized `True`:**
```rust
use dynamo_parsers::tool_calling::dsml::try_tool_call_parse_dsml;
use dynamo_parsers::tool_calling::config::DsmlParserConfig;

let input = concat!(
    "<｜DSML｜function_calls>",
    "<｜DSML｜invoke name=\"test\">",
    "<｜DSML｜parameter name=\"x\" string=\"True\">hello</｜DSML｜parameter>",
    "</｜DSML｜invoke>",
    "</｜DSML｜function_calls>"
);
let (calls, _) = try_tool_call_parse_dsml(input, &DsmlParserConfig::default()).unwrap();
// calls[0].function.arguments == "{}" — parameter "x" silently lost
```

**Missing attribute:**
```rust
let input = concat!(
    "<｜DSML｜function_calls>",
    "<｜DSML｜invoke name=\"test\">",
    "<｜DSML｜parameter name=\"count\">42</｜DSML｜parameter>",
    "</｜DSML｜invoke>",
    "</｜DSML｜function_calls>"
);
let (calls, _) = try_tool_call_parse_dsml(input, &DsmlParserConfig::default()).unwrap();
// calls[0].function.arguments == "{}" — parameter "count" silently lost
```

### Expected Behavior

- `string="True"` should be treated the same as `string="true"`
- Parameters without the `string` attribute should be parsed (defaulting to raw value / `string="false"` behavior)

### Actual Behavior

Both cases silently produce an empty arguments object `{}`. No error, no warning — the data is just lost.

### Suggested Fix

1. Make the boolean match case-insensitive: change `(true|false)` to `(?i:true|false)` in the regex
2. Make the `string` attribute optional: change `\s+string=\"(true|false)\"` to `(?:\s+string=\"(?i:true|false)\")?`
3. Default to `false` (raw value) when the attribute is absent

### Environment

- dynamo: main branch
- Crate: `dynamo-parsers`
- File: `lib/parsers/src/tool_calling/dsml/parser.rs`, lines 173-176, 185
