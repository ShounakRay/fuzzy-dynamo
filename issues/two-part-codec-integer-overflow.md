# TwoPartCodec: Integer Overflow in decode

## Summary

`TwoPartCodec::decode` panics with `attempt to add with overflow` when
processing crafted input. The method reads `header_len` and `body_len` as `u64`
values from untrusted input, casts them to `usize`, and computes
`24 + header_len + body_len` without overflow checking. This causes an
arithmetic overflow panic in debug mode and silent wraparound in release mode.

## Steps to Reproduce

### Via fuzzing

```bash
cd lib/runtime/fuzz
~/.cargo/bin/cargo +nightly fuzz run fuzz_codec_crash_oracle \
  artifacts/fuzz_codec_crash_oracle/crash-7f3da18e2fa6839be3c7450737056de97a892e37
```

## Root Cause

In `two_part.rs:58`, the expression `24 + header_len + body_len` overflows when
`header_len` and `body_len` are large `u64` values read from the input buffer.
The fuzz target comment states: "All decoders must return Ok or Err on arbitrary
input, never panic." — this violates that contract.

## Impact

- **Severity**: High — panic on untrusted network input (DoS)
- **Affected code**: `TwoPartCodec::decode` in `lib/runtime/src/two_part.rs`
- **Workaround**: None — any network peer can send a crafted frame to trigger
  the panic

## Suggested Fix

Use checked arithmetic or `saturating_add`:

```rust
// Before:
let total = 24 + header_len + body_len;

// After:
let total = 24usize
    .checked_add(header_len)
    .and_then(|v| v.checked_add(body_len))
    .ok_or_else(|| /* return Err */)?;
```

Alternatively, validate that `header_len` and `body_len` are within reasonable
bounds before performing arithmetic (e.g., each must be less than the remaining
buffer length).
