// Minimal copy of dynamo_llm::protocols::codec for fuzzing.
// Source: lib/llm/src/protocols/codec.rs — keep in sync with upstream.

use bytes::BytesMut;
use serde::Deserialize;
use tokio_util::codec::{Decoder, LinesCodec};

/// An error that occurs when decoding an SSE stream.
#[derive(Debug, thiserror::Error)]
pub enum SseCodecError {
    #[error("SseLineCodec decode error: {0}")]
    DecodeError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct SseLineCodec {
    lines_codec: LinesCodec,
    data_buffer: String,
    event_type_buffer: String,
    last_event_id_buffer: String,
    comments_buffer: Vec<String>,
}

#[derive(Debug)]
pub struct Message {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: Option<String>,
    pub comments: Option<Vec<String>>,
}

impl Message {
    pub fn decode_data<T>(&self) -> Result<T, SseCodecError>
    where
        T: for<'de> Deserialize<'de>,
    {
        serde_json::from_str(self.data.as_ref().ok_or(SseCodecError::DecodeError(
            "no data: message to decode".to_string(),
        ))?)
        .map_err(|e| SseCodecError::DecodeError(format!("failed to deserialized data: {}", e)))
    }
}

impl SseLineCodec {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for SseLineCodec {
    fn default() -> Self {
        Self {
            lines_codec: LinesCodec::new(),
            data_buffer: String::new(),
            event_type_buffer: String::new(),
            last_event_id_buffer: String::new(),
            comments_buffer: Vec::new(),
        }
    }
}

impl Decoder for SseLineCodec {
    type Item = Message;
    type Error = SseCodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            match self
                .lines_codec
                .decode(src)
                .map_err(|e| SseCodecError::DecodeError(e.to_string()))?
            {
                Some(line) => {
                    let line = line.trim_end_matches(&['\r', '\n'][..]);
                    if line.is_empty() {
                        // End of event; dispatch
                        if !self.data_buffer.is_empty()
                            || !self.event_type_buffer.is_empty()
                            || !self.last_event_id_buffer.is_empty()
                            || !self.comments_buffer.is_empty()
                        {
                            // Remove the last '\n' if present in data_buffer
                            if self.data_buffer.ends_with('\n') {
                                self.data_buffer.pop();
                            }

                            let data = if !self.data_buffer.is_empty() {
                                Some(std::mem::take(&mut self.data_buffer))
                            } else {
                                None
                            };

                            let message = Message {
                                id: if self.last_event_id_buffer.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut self.last_event_id_buffer))
                                },
                                event: if self.event_type_buffer.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut self.event_type_buffer))
                                },
                                data,
                                comments: if self.comments_buffer.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut self.comments_buffer))
                                },
                            };
                            return Ok(Some(message));
                        } else {
                            continue;
                        }
                    } else if let Some(comment) = line.strip_prefix(':') {
                        self.comments_buffer.push(comment.trim().into());
                    } else {
                        let (field_name, field_value) = if let Some(idx) = line.find(':') {
                            let (name, value) = line.split_at(idx);
                            let value = value[1..].trim_start_matches(' ');
                            (name, value)
                        } else {
                            (line, "")
                        };

                        match field_name {
                            "event" => {
                                self.event_type_buffer = field_value.to_string();
                            }
                            "data" => {
                                if field_value != "[DONE]" {
                                    if !self.data_buffer.is_empty() {
                                        self.data_buffer.push('\n');
                                    }
                                    self.data_buffer.push_str(field_value);
                                }
                            }
                            "id" => {
                                if !field_value.contains('\0') {
                                    self.last_event_id_buffer = field_value.to_string();
                                }
                            }
                            "retry" => {
                                // Ignored
                            }
                            _ => {
                                // Ignore unknown fields
                            }
                        }
                    }
                }
                None => {
                    return Ok(None);
                }
            }
        }
    }

    fn decode_eof(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let result = self.decode(src)?;
        if result.is_some() {
            return Ok(result);
        }
        if self.data_buffer.is_empty()
            && self.event_type_buffer.is_empty()
            && self.last_event_id_buffer.is_empty()
            && self.comments_buffer.is_empty()
        {
            Ok(None)
        } else {
            if self.data_buffer.ends_with('\n') {
                self.data_buffer.pop();
            }

            let data = if !self.data_buffer.is_empty() {
                Some(std::mem::take(&mut self.data_buffer))
            } else {
                None
            };

            let message = Message {
                id: if self.last_event_id_buffer.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.last_event_id_buffer))
                },
                event: if self.event_type_buffer.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.event_type_buffer))
                },
                data,
                comments: if self.comments_buffer.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut self.comments_buffer))
                },
            };
            Ok(Some(message))
        }
    }
}
