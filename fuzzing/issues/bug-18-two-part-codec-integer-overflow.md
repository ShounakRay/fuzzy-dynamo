### [BUG]: `TwoPartCodec::decode` integer overflow bypasses `max_message_size` check

### Describe the Bug

`TwoPartCodec::decode_message()` in `lib/runtime/src/pipeline/network/codec/two_part.rs` computes `let total_len = 24 + header_len + body_len` (line 58) using unchecked addition. When a crafted message contains large values for `header_len` or `body_len`, this addition overflows:

- **Debug builds**: panics with `attempt to add with overflow`
- **Release builds**: wraps silently to a small value, potentially **bypassing the `max_message_size` check** at lines 61-64, then causing out-of-bounds reads or incorrect buffer sizing

This is a network-facing codec — any peer can send a crafted 24-byte message header to trigger it.

### Steps to Reproduce

Found via `cargo-fuzz`:

```bash
cd lib/runtime
cargo +nightly fuzz run fuzz_two_part_decode -- -max_total_time=60
```

Minimal crashing input (24 bytes):
```
[0, 58, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10]
```

This encodes `header_len=58` and a large `body_len` that causes `24 + 58 + body_len` to overflow `usize`.

To reproduce with the crash artifact directly:

```bash
cd lib/runtime
cargo +nightly fuzz run fuzz_two_part_decode \
  fuzz/artifacts/fuzz_two_part_decode/crash-1bd347611b550cf294eac849f2b7e9c1a21797f3
```

### Expected Behavior

`decode_message` should detect the overflow and return an error, not panic (debug) or silently wrap (release).

### Actual Behavior

**Debug**:
```
thread 'main' panicked at 'attempt to add with overflow'
  at lib/runtime/src/pipeline/network/codec/two_part.rs:58
```

**Release**: `total_len` wraps to a small value. The `max_message_size` check at line 61 passes (small wrapped value < max), then subsequent code reads past the actual buffer boundary.

### Suggested Fix

Replace the unchecked addition with `checked_add()`:

```rust
let total_len = 24usize
    .checked_add(header_len)
    .and_then(|n| n.checked_add(body_len))
    .ok_or(TwoPartCodecError::MessageTooLarge(usize::MAX, 0))?;
```

### Environment

- dynamo: main branch
- Crate: `dynamo-runtime`
- File: `lib/runtime/src/pipeline/network/codec/two_part.rs`, line 58

### Additional Context

Found via fuzzing with `cargo-fuzz` / libfuzzer. This was predicted by code audit (the unchecked `24 + header_len + body_len` pattern) and confirmed by the fuzzer.

The release-mode behavior is particularly concerning: silent wrapping means a crafted message header can bypass the `max_message_size` guard and potentially cause memory safety issues downstream. This should be treated as a security-sensitive fix.
