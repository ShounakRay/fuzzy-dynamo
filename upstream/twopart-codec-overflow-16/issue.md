# [BUG]: TwoPartCodec integer overflow panic on crafted network input

> **Status: FIXED** — We filed this upstream as issue #6955. Fix merged in PR #6959 using checked arithmetic.

### Describe the Bug

`TwoPartCodec::decode()` in `lib/runtime/src/pipeline/network/codec/two_part.rs` computes `let total_len = 24 + header_len + body_len`. But it uses unchecked addition, so if a crafted message contains large values for `header_len` or `body_len`, this addition overflows. The debug build panics with `attempt to add with overflow` and the release build silently wraps to a small value (bypassing the `max_message_size` check and `src.len() < total_len` check).

The overflow occurs at `two_part.rs:54-58`:

```rust
let header_len = cursor.get_u64() as usize;  // line 54: unchecked u64→usize cast
let body_len = cursor.get_u64() as usize;    // line 55: unchecked u64→usize cast
let _checksum = cursor.get_u64();

let total_len = 24 + header_len + body_len;   // line 58: OVERFLOW here
```

On 64-bit platforms the `u64` to `usize` cast preserves all bits, allowing `usize::MAX`-sized values. The `max_message_size` check on line 61 happens *after* the overflow, so it cannot prevent the panic. In release mode without overflow checks, the value wraps around to a small number, potentially causing the decoder to return garbage data from a tiny buffer.

### Steps to Reproduce

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

Minimal reproduction:

```rust
// Any 24+ byte input where bytes 0..8 and 8..16 (as u64) sum > usize::MAX - 24
let mut input = vec![0u8; 24];
input[..8].copy_from_slice(&u64::MAX.to_be_bytes());  // header_len = MAX
input[8..16].copy_from_slice(&1u64.to_be_bytes());     // body_len = 1
let codec = TwoPartCodec::new(Some(64));
let _ = codec.decode_message(Bytes::from(input));
// PANICS at two_part.rs:58
```

### Expected Behavior

Should detect the overflow and return an error, not panic or silently wrap.

### Actual Behavior

```
thread 'overflow_panics_in_debug' panicked at 'attempt to add with overflow'
  at lib/runtime/src/pipeline/network/codec/two_part.rs:58
```

### Environment

On commit `57e6a79f5` in `dynamo-runtime` @ `lib/runtime/src/pipeline/network/codec/two_part.rs`

### Additional Context

This is a denial-of-service vector on any network-facing service using `TwoPartCodec`. An attacker can send 24 bytes of crafted data to crash the server.

Crash artifacts:
- `fuzz/artifacts/fuzz_network_codecs/crash-77cb620da95152f7285c9db5a9198e50697cb9ae`
- `fuzz/artifacts/fuzz_network_codecs/crash-caf43076d48478d873d7b603d30b1d01c750e27f`
- `fuzz/artifacts/fuzz_network_codecs/crash-f2f205d4dfac27e18a006ec3fc346495b31970be`

The fix (merged in PR #6959) uses checked arithmetic: `24usize.checked_add(header_len).and_then(|v| v.checked_add(body_len))`.

Found via fuzzing with cargo-fuzz / libfuzzer.

### Screenshots

_No response_
