# Upstream Fix Patches

Proposed fixes for 14 open bugs found via fuzzing. Each `.patch.rs` file contains
the original buggy code, the fixed version, and a regression test.

Verified against `upstream/main` as of 2026-03-19 (commit 50af3435f).

## HIGH — Crash/Panic (6)

| # | File | Bug | Fix |
|---|------|-----|-----|
| 003 | `glm47_parser.rs` | UTF-8 byte offset panic on multibyte function names | Use original `pos` from `find()` instead of trimmed `function_name.len()` |
| 005 | `zmq_wire.rs` | OOB index in `create_stored_blocks` | Validate token_ids length before loop |
| 013 | `xml/parser.rs` | `strip_quotes` panics on single-char quote | Guard `len >= 2` before slicing |
| 001 | `protocols.rs` | `compute_block_hash_for_seq` zero block_size | Early return empty vec when `kv_block_size == 0` |
| 012 | `selector.rs` | `select_worker` div-by-zero on zero block_size | Early return `Err(NoEndpoints)` |
| 011 | `protocols.rs` | `to_block_level` div-by-zero | Early return when `block_size == 0` |

## HIGH — Correctness (4)

| # | File | Bug | Fix |
|---|------|-----|-----|
| 004 | `positional.rs` | Jump optimization skips removed blocks | Cascade-remove blocks at positions > P |
| 008 | `granite_parser.rs` | Streaming vs oneshot mismatch | Only buffer prefixes of relevant tokens based on current mode |
| 006 | `pythonic_parser.rs` | Prefix chars absorbed into function name | Add `(?<!\w)` negative lookbehind to regex |
| 014 | `xml/parser.rs` | `try_literal_eval` corrupts strings with Python keywords | Use `\bTrue\b` word-boundary regex |

## MEDIUM — Logic (4)

| # | File | Bug | Fix |
|---|------|-----|-----|
| 009 | `kimi_k2_parser.rs` | OnceLock caches first config's regex forever | Remove OnceLock, compile regex per call |
| 007 | `dsml/parser.rs` | Parameters silently dropped | Make `string` attribute optional and case-insensitive |
| 010 | `minimax_parser.rs` | Streaming/oneshot trailing newline mismatch | Override `detect_and_parse_reasoning` directly |
| 002 | `concurrent_radix_tree.rs` | Deadlock on duplicate block hashes | Add `Arc::ptr_eq` self-reference check |

## Already Fixed Upstream (2)

| # | Bug | PR |
|---|-----|----|
| 015 | RadixTree score underreporting | PRs #5973, #6122 |
| 016 | TwoPartCodec integer overflow | PR #6959 |
