// Minimal copy of dynamo_llm::protocols::codec for fuzzing.
// Stripped of the Annotated dependency to avoid pulling in all of dynamo-llm
// (which has native deps that don't link on macOS).
//
// Source: lib/llm/src/protocols/codec.rs
// Keep in sync with upstream.

pub mod codec;
