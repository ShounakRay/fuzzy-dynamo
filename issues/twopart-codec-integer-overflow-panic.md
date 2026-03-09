# Bug 16: TwoPartCodec integer overflow panic on crafted network input

> **Status: FIXED** — We filed this upstream as issue #6955. Fix merged in PR #6959 using checked arithmetic.

## Summary

The `TwoPartCodec::decode` method panics with "attempt to add with overflow" when processing network input containing large `header_len` or `body_len` values. The overflow occurs before the `max_message_size` safety check, making even size-limited codecs vulnerable.

## Severity

**High** — This is a denial-of-service vector on any network-facing service using `TwoPartCodec`. An attacker can send 24 bytes of crafted data to crash the server. The overflow happens unconditionally, even when `max_message_size` is set.

## Steps to Reproduce

```rust
use bytes::Bytes;
use dynamo_runtime::pipeline::network::codec::TwoPartCodec;

// header_len = 0xFFFFFFFFFFFFFFFF, body_len = 0xFFFFFFFFFFFFFFFF
// This causes 24 + header_len + body_len to overflow
let input = b"\xdd\xd0\x00\x00\xff\xff\xff\xff\xff\xff\xff\xff\xf3\x00\x00\x00\xc8\xd0\xd0\xd0\x00\xe2\x00\x00";
let codec = TwoPartCodec::new(Some(1024)); // max_message_size doesn't help!
let _ = codec.decode_message(Bytes::copy_from_slice(input));
// PANICS: "attempt to add with overflow"
```

### Minimal reproduction:

```rust
// Any 24+ byte input where bytes 0..8 and 8..16 (as u64) sum > usize::MAX - 24
let mut input = vec![0u8; 24];
input[..8].copy_from_slice(&u64::MAX.to_be_bytes());  // header_len = MAX
input[8..16].copy_from_slice(&1u64.to_be_bytes());     // body_len = 1
let codec = TwoPartCodec::new(Some(64));
let _ = codec.decode_message(Bytes::from(input));
// PANICS at two_part.rs:58
```

## Root Cause

In `two_part.rs:54-58`:

```rust
let header_len = cursor.get_u64() as usize;  // line 54: unchecked u64→usize cast
let body_len = cursor.get_u64() as usize;    // line 55: unchecked u64→usize cast
let _checksum = cursor.get_u64();

let total_len = 24 + header_len + body_len;   // line 58: OVERFLOW here
```

The problem:
1. `header_len` and `body_len` are read as `u64` and cast to `usize` — on 64-bit platforms this preserves all bits, allowing `usize::MAX`-sized values
2. `24 + header_len + body_len` performs unchecked addition, which panics in debug mode (or wraps in release mode without overflow checks)
3. The `max_message_size` check on line 61 happens *after* the overflow, so it cannot prevent the panic

In release mode without overflow checks, the value wraps around to a small number, potentially causing the decoder to return garbage data from a tiny buffer — a possible memory safety concern.

## Crash Artifacts

- `fuzz/artifacts/fuzz_network_codecs/crash-77cb620da95152f7285c9db5a9198e50697cb9ae`
- `fuzz/artifacts/fuzz_network_codecs/crash-caf43076d48478d873d7b603d30b1d01c750e27f`
- `fuzz/artifacts/fuzz_network_codecs/crash-f2f205d4dfac27e18a006ec3fc346495b31970be`

All three trigger the same overflow at line 58.

## Suggested Fix

Use checked arithmetic and validate before computing `total_len`:

```rust
let header_len = cursor.get_u64() as usize;
let body_len = cursor.get_u64() as usize;
let _checksum = cursor.get_u64();

let total_len = 24usize
    .checked_add(header_len)
    .and_then(|v| v.checked_add(body_len))
    .ok_or_else(|| TwoPartCodecError::MessageTooLarge(usize::MAX, self.max_message_size.unwrap_or(0)))?;
```

This ensures the `max_message_size` check on line 61 is always reached with a valid `total_len`.

Found by: `fuzz_network_codecs` fuzzer.
