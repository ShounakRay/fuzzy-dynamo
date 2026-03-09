### [BUG]: RadixTree `apply_event` panics with `RefCell already mutably borrowed` on hash collisions

### Describe the Bug

`RadixTree::apply_event()` in `lib/kv-router/src/radix_tree.rs` panics with `RefCell already mutably borrowed` when processing store events whose block hashes collide.

The `worker_lookup` hash map caches `Rc<RefCell<RadixBlock>>` by `ExternalSequenceBlockHash`. When store events create sequences with duplicate block hashes, the same `Rc` gets reused, creating a cycle where a node and its child are the same object. At line 361, `current.borrow_mut()` takes a mutable borrow on the parent. Then at line 371, `block.borrow()` tries to immutably borrow what turns out to be the same `RefCell` — triggering the panic.

There is a self-reference guard at line 400 (`try_borrow_mut`), but it runs **after** the crash point at line 371. The guard only protects new-child creation, not existing-child access.

### Steps to Reproduce

Found via `cargo-fuzz` with a 10-byte input:

```bash
cd lib/kv-router
cargo +nightly fuzz run fuzz_radix_tree_events -- -max_total_time=60
```

Minimal crashing input (hex): `00 00 0a 00 00 00 00 00 00 00`

This encodes a store event with `worker_id=0` and block hashes that collide, causing the `Rc` reuse cycle.

To reproduce with the crash artifact directly:

```bash
cd lib/kv-router
cargo +nightly fuzz run fuzz_radix_tree_events \
  fuzz/artifacts/fuzz_radix_tree_events/crash-0a2d64ba6898a5b06f8a7f1cba83f36e0aa85944
```

### Expected Behavior

`apply_event` should handle hash collisions gracefully — either skip the self-referential insertion or return an error.

### Actual Behavior

```
thread 'main' panicked at 'already mutably borrowed: BorrowError'
  at lib/kv-router/src/radix_tree.rs:371
```

Full backtrace shows `borrow()` on a `RefCell` that is already held by `borrow_mut()` from line 361.

### Suggested Fix

Move the self-reference check before line 371. Before accessing `block.borrow()` in the `Some(block)` arm, check if `block` is the same `Rc` as `current`:

```rust
Some(block) => {
    // Check for self-reference BEFORE borrowing
    if Rc::ptr_eq(block, &current) {
        tracing::warn!("self-referential block detected, skipping");
        continue;
    }
    if block.borrow().block_hash != Some(block_data.block_hash) {
        // existing warning...
    }
    block.clone()
}
```

### Environment

- dynamo: main branch
- Crate: `dynamo-kv-router`
- File: `lib/kv-router/src/radix_tree.rs`, lines 361-371

### Additional Context

Found via fuzzing with `cargo-fuzz` / libfuzzer. This is a novel bug — no existing tests cover indirect `Rc<RefCell>` cycles in the radix tree. Since block hashes derive from token data, a malicious or unlucky request sequence can trigger this in production, crashing the KV router.
