# RadixTree RefCell Aliasing Panic

## Summary

`RadixTree::apply_event()` panics with `RefCell already mutably borrowed` when
processing store events containing duplicate `ExternalSequenceBlockHash` values.
The same `Rc<RefCell<RadixBlock>>` gets cached for multiple hash entries in
`worker_lookup`, creating self-referential nodes where a parent and child point
to the same `RefCell`. The panic occurs at line 371 of `radix_tree.rs` when
attempting an immutable borrow on a node already held via mutable borrow.

The self-reference check at line 400 (`try_borrow_mut`) was added as a safety
guard but runs too late — the panic has already occurred during block validation.

## Steps to Reproduce

### Via fuzzing

```bash
cd lib/kv-router/fuzz
~/.cargo/bin/cargo +nightly fuzz run fuzz_radix_tree_events -- -max_total_time=60
```

### With crash artifact

```bash
~/.cargo/bin/cargo +nightly fuzz run fuzz_radix_tree_events \
  artifacts/fuzz_radix_tree_events/crash-0a2d64ba6898a5b06f8a7f1cba83f36e0aa85944
```

### Standalone Rust code

```rust
use dynamo_kv_router::RadixTree;
use dynamo_kv_router_fuzz::make_store_event;

let mut tree = RadixTree::new();
// Store event with duplicate block hashes (same ExternalSequenceBlockHash)
let event = make_store_event(0, 0, &[1, 1, 1], None);
let _ = tree.apply_event(event); // panics: already mutably borrowed
```

## Root Cause

In `radix_tree.rs`, the `apply_event` Stored handler iterates over blocks and
uses `worker_lookup` to cache `Rc<RefCell<RadixBlock>>` pointers keyed by
`ExternalSequenceBlockHash`. When two blocks produce the same
`ExternalSequenceBlockHash`, the lookup returns the same `Rc`:

1. Line 361: `let mut parent_mut = current.borrow_mut()` — acquires mutable borrow
2. Line 371: `block.borrow()` — attempts immutable borrow on what may be the
   same `RefCell` as `current` (if hash collision caused aliasing)
3. **Panic**: `RefCell<RadixBlock> already mutably borrowed`

The existing guard at line 400 (`child.try_borrow_mut().is_err()`) detects
self-references but only runs _after_ the crash site at line 371.

## Suggested Fix

Move the self-reference check before the borrow at line 371:

```rust
let child = match parent_mut.children.get(&block_data.tokens_hash) {
    Some(block) => {
        // Check for self-reference BEFORE borrowing
        if Rc::ptr_eq(block, &current) {
            return Err(KvCacheEventError::InvalidBlockSequence);
        }
        // Now safe — block is not aliased to current
        if block.borrow().block_hash != Some(block_data.block_hash) {
            // ... warning ...
        }
        block.clone()
    }
    None => { /* ... */ }
};
```

`Rc::ptr_eq()` is a cheap pointer comparison that catches the aliasing condition
before any borrow is attempted.

## Impact

- **Severity**: High — causes a panic (DoS) on adversarial input
- **Exploitability**: Requires hash collisions in store event sequences; rare in
  normal traffic but trivially achievable via fuzzing
- **Workaround**: Fuzz targets deduplicate hashes before calling `apply_event`
  (see `fuzz_radix_tree_consistency.rs` and `fuzz_differential_indexers.rs`)
- **Note**: `ConcurrentRadixTree` uses `Arc<RwLock<>>` and is not affected
