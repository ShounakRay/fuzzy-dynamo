### [BUG]: `TwoPartCodec::decode` integer overflow bypasses `max_message_size` check

### What This Bug Is (Plain English)

Dynamo components talk to each other over the network. Every message starts with a 24-byte header that says "the next chunk is X bytes long, and the chunk after that is Y bytes long." The receiving side reads those two numbers, adds them together to get the total message size, and checks that it's not too big before reading the actual data.

The problem: that addition can overflow. If you send a header claiming absurdly large sizes that add up to *more than the maximum number a computer can store*, the number wraps around to a small value — like an odometer rolling past 999999 back to 000000. Now the "total size" looks tiny, so the safety check says "looks fine!" and lets it through. But the code still tries to read the *original* huge number of bytes from a nearly empty buffer, which crashes the program.

### Who Can Trigger This

This would most likely come up in a multi-tenant or shared infrastructure scenario. Dynamo serves AI/ML models across a cluster, and the components (workers, routers, etc.) all talk to each other using this codec.

- **A malicious insider** — someone with network access to the cluster (a developer, operator, or another team sharing the same infrastructure) could send a crafted packet to crash any dynamo component
- **A compromised service** — if any single component in the cluster gets compromised (or has its own bug), it could use this to take down other components
- **An attacker who reaches the internal network** — if dynamo ports are accidentally exposed or an attacker gets a foothold through some other means

A normal end user hitting an API endpoint wouldn't reach this code directly — there's typically a web server / API layer in front. This is internal component-to-component communication. But in any deployment where you don't fully trust every machine on the network (shared clusters, cloud environments, multi-tenant setups), one bad actor or one compromised node can knock out the whole system.

### Describe the Bug

`TwoPartCodec::decode()` in `lib/runtime/src/pipeline/network/codec/two_part.rs` computes `let total_len = 24 + header_len + body_len` (line 58) using unchecked addition. When a crafted message contains large values for `header_len` or `body_len`, this addition overflows:

- **Debug builds**: panics with `attempt to add with overflow`
- **Release builds**: wraps silently to a small value, **bypassing both the `max_message_size` check and the buffer length check**, then panicking at `split_to()` — a denial-of-service from any network peer

This is a network-facing codec — any peer can send a crafted 24-byte message header to crash the process.

Note: there is a `checked_add` at line 81, but it's inside `#[cfg(debug_assertions)]` for checksum validation only — it is compiled out in release builds and provides no protection.

### Release-Mode Exploit — Step by Step

A crafted 24-byte header can wrap `total_len` to any value. For example, with `header_len = 0` and `body_len = 2^64 - 24` (`0xffffffffffffffe8`):

```
24 + 0 + (2^64 - 24) = 2^64 = 0  (mod 2^64, wrapping)
```

This produces `total_len = 0`, which then:

1. **Line 61**: `max_message_size` check → `0 > 1024` → false → **BYPASSED**
2. **Line 68**: buffer length check → `24 < 0` → false → **BYPASSED**
3. **Line 73**: `src.advance(24)` → buffer now has 0 bytes remaining
4. **Line 99**: `src.split_to(body_len)` → requests `2^64 - 24` bytes from empty buffer → **PANIC**

Wire bytes for this input (big-endian `[header_len, body_len, checksum]`):
```
00 00 00 00 00 00 00 00  ff ff ff ff ff ff ff e8  00 00 00 00 00 00 00 00
```

A second variant with `header_len = 10`, `body_len = 2^64 - 10` wraps `total_len` to exactly 24, also bypassing both checks and panicking at `split_to(10)` on an empty buffer.

### Steps to Reproduce

**Option A — Fuzz artifact replay (requires nightly + cargo-fuzz):**

```bash
cd lib/runtime
cargo +nightly fuzz run fuzz_two_part_decode \
  fuzz/artifacts/fuzz_two_part_decode/crash-1bd347611b550cf294eac849f2b7e9c1a21797f3
```

**Option B — Integration test (debug mode, panics on overflow):**

Save as `lib/runtime/tests/bug18_overflow.rs`:

```rust
use bytes::BytesMut;
use tokio_util::codec::Decoder;
use dynamo_runtime::pipeline::network::codec::TwoPartCodec;

/// Crafted 24-byte header: header_len=0, body_len=2^64-24, checksum=0.
/// In release mode, 24 + 0 + (2^64-24) wraps to 0, bypassing all size checks.
/// In debug mode, panics with "attempt to add with overflow".
const OVERFLOW_INPUT: [u8; 24] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // header_len = 0
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xe8, // body_len = 2^64 - 24
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // checksum = 0
];

#[test]
#[should_panic(expected = "attempt to add with overflow")]
fn overflow_panics_in_debug() {
    let mut codec = TwoPartCodec::new(Some(1024));
    let mut buf = BytesMut::from(OVERFLOW_INPUT.as_slice());
    let _ = codec.decode(&mut buf);
}
```

```bash
cd lib/runtime
cargo test --test bug18_overflow
```

### Expected Behavior

`decode()` should detect the overflow and return an error, not panic (debug) or silently wrap (release).

### Actual Behavior

**Debug**:
```
thread 'overflow_panics_in_debug' panicked at 'attempt to add with overflow'
  at lib/runtime/src/pipeline/network/codec/two_part.rs:58
```

**Release**: `total_len` wraps to 0 (or another small value), bypassing both the `max_message_size` guard and the buffer length check. Code reaches `split_to(body_len)` with an attacker-controlled `body_len` on an empty buffer, causing a panic. This is a **denial-of-service** — any network peer can crash the process with a single 24-byte message.

### Suggested Fix

Replace the unchecked addition with `checked_add()`:

```rust
let total_len = 24usize
    .checked_add(header_len)
    .and_then(|n| n.checked_add(body_len))
    .ok_or(TwoPartCodecError::MessageTooLarge(usize::MAX, 0))?;
```

There is a second identical pattern at line 112 (encode path) that should also be fixed.

### Environment

- dynamo: main branch (commit `57e6a79f5`, 2026-03-04)
- Crate: `dynamo-runtime`
- File: `lib/runtime/src/pipeline/network/codec/two_part.rs`, lines 58 and 112

### Additional Context

Found via fuzzing with `cargo-fuzz` / libfuzzer. The `#[cfg(debug_assertions)]` checksum block (lines 75-95) contains a `checked_add` for `header_len + body_len`, but this is compiled out in release builds, leaving the main `total_len` computation on line 58 completely unprotected. The network-facing nature of this codec makes it exploitable by any connected peer.
