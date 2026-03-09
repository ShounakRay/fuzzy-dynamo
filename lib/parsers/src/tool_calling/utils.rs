// SPDX-FileCopyrightText: Copyright (c) 2025-2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

/// Check if `chunk` ends with any prefix of `token` (streaming partial match).
/// Uses character-based iteration to avoid UTF-8 boundary panics.
pub fn chunk_ends_with_token_prefix(chunk: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let chars: Vec<char> = token.chars().collect();
    for i in 1..chars.len() {
        let prefix: String = chars[..i].iter().collect();
        if chunk.ends_with(&prefix) {
            return true;
        }
    }
    false
}

/// Decode common XML/HTML entities. Covers named entities (&lt; &gt; &amp;
/// &quot; &apos;) and numeric entities for apostrophe (&#x27; &#39;).
pub fn decode_xml_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_ends_with_token_prefix_basic() {
        assert!(chunk_ends_with_token_prefix("<tool_c", "<tool_call>"));
        assert!(chunk_ends_with_token_prefix("<", "<tool_call>"));
        assert!(!chunk_ends_with_token_prefix("no match", "<tool_call>"));
    }

    #[test]
    fn test_chunk_ends_with_token_prefix_multibyte_utf8() {
        // Must not panic on multibyte UTF-8 tokens
        let token = "<\u{5DE5}\u{5177}>"; // "<工具>"
        assert!(chunk_ends_with_token_prefix("partial <\u{5DE5}", token));
        assert!(chunk_ends_with_token_prefix("partial <", token));
        assert!(!chunk_ends_with_token_prefix("no match", token));
    }

    #[test]
    fn test_chunk_ends_with_token_prefix_empty_token() {
        assert!(!chunk_ends_with_token_prefix("any chunk", ""));
    }

    #[test]
    fn test_decode_xml_entities() {
        assert_eq!(decode_xml_entities("&lt;div&gt;"), "<div>");
        assert_eq!(decode_xml_entities("a &amp; b"), "a & b");
        assert_eq!(decode_xml_entities("&quot;quoted&quot;"), "\"quoted\"");
        assert_eq!(decode_xml_entities("&apos;apos&apos;"), "'apos'");
        assert_eq!(decode_xml_entities("&#x27;hex&#x27;"), "'hex'");
        assert_eq!(decode_xml_entities("&#39;dec&#39;"), "'dec'");
        assert_eq!(decode_xml_entities("no entities"), "no entities");
    }
}
