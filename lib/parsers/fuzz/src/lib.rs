//! Shared helpers for parser fuzz targets.

use dynamo_parsers::reasoning::ReasoningParserType;

/// All reasoning parser types for fuzz target selection.
pub const REASONING_PARSER_TYPES: &[ReasoningParserType] = &[
    ReasoningParserType::DeepseekR1,
    ReasoningParserType::Basic,
    ReasoningParserType::GptOss,
    ReasoningParserType::Qwen,
    ReasoningParserType::NemotronDeci,
    ReasoningParserType::Kimi,
    ReasoningParserType::KimiK25,
    ReasoningParserType::Step3,
    ReasoningParserType::Mistral,
    ReasoningParserType::Granite,
    ReasoningParserType::MiniMaxAppendThink,
];

/// Select a reasoning parser type from a fuzz byte.
pub fn select_parser_type(byte: u8) -> ReasoningParserType {
    REASONING_PARSER_TYPES[byte as usize % REASONING_PARSER_TYPES.len()]
}

/// Iterator that splits a string into chunks using a fuzz-controlled strategy.
/// Handles UTF-8 boundaries correctly.
pub struct StreamingChunker<'a> {
    bytes: &'a [u8],
    pos: usize,
    strategy: u8,
    seed_byte: u8,
}

impl<'a> StreamingChunker<'a> {
    /// Create a new chunker.
    /// `strategy` should be `byte % 4` where byte comes from fuzz data.
    /// Strategies: 0=byte-by-byte, 1=random 1-16, 2=medium 4-32, 3=whole input.
    pub fn new(s: &'a str, strategy: u8, seed_byte: u8) -> Self {
        Self {
            bytes: s.as_bytes(),
            pos: 0,
            strategy: strategy % 4,
            seed_byte,
        }
    }
}

impl<'a> Iterator for StreamingChunker<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.pos >= self.bytes.len() {
            return None;
        }

        let chunk_len = match self.strategy {
            0 => 1,
            1 => {
                let seed = self.pos.wrapping_mul(31).wrapping_add(self.seed_byte as usize);
                (seed % 16).max(1)
            }
            2 => {
                let seed = self.pos.wrapping_mul(37).wrapping_add(self.seed_byte as usize);
                ((seed % 29) + 4).min(self.bytes.len() - self.pos)
            }
            _ => self.bytes.len() - self.pos,
        };

        let mut end = (self.pos + chunk_len).min(self.bytes.len());

        // Extend to complete UTF-8 character
        while end < self.bytes.len() && (self.bytes[end] & 0xC0) == 0x80 {
            end += 1;
        }

        if end == self.pos {
            return None;
        }

        let chunk = unsafe { std::str::from_utf8_unchecked(&self.bytes[self.pos..end]) };
        self.pos = end;
        Some(chunk)
    }
}

/// Truncate a string to `max_len` bytes at a valid char boundary.
pub fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..s.floor_char_boundary(max_len)]
    }
}
