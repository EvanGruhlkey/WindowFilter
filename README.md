# WindowFilter

From-scratch Rust implementations of three approximate membership filters:

- `BloomFilter`: compact membership checks with no deletion.
- `CuckooFilter`: a standard `(2, 4)` filter with four-slot, power-of-two buckets.
- `WindowedCuckooFilter`: a `(2, 2)` filter with overlapping adjacent windows.

The windowed design follows *Smaller and More Flexible Cuckoo Filters*. It stores a
`k`-bit fingerprint, one window-choice bit, and one within-window offset bit. Its
signed-offset addressing permits an arbitrary number of windows, while the conventional
cuckoo filter requires a power-of-two bucket count. All filter storage is bit-packed.

## Usage

```rust
use windowfilter::WindowedCuckooFilter;

let mut filter = WindowedCuckooFilter::new(100_000, 10);

assert!(filter.insert("evan"));
assert!(filter.contains("evan"));
assert!(!filter.contains("bob")); // probabilistic: false positives are possible
assert!(filter.delete("evan"));
assert!(!filter.contains("evan"));
```

The second constructor argument is `k`, giving a target false-positive rate of `2^-k`.
`insert` returns `false` if a cuckoo filter reaches its relocation limit. Failed inserts
are rolled back, preserving every key already in the filter.

Bloom filters cannot safely delete individual keys because their bits are shared. The two
cuckoo filters support deletion, but—as with cuckoo filters generally—call `delete` only
for keys known to have been inserted. Deleting a false-positive key can remove a colliding
fingerprint.

## Benchmark

Run the dependency-free benchmark harness in release mode:

```sh
cargo run --release --bin benchmark -- \
  --items 100000 --queries 200000 --fpr-bits 10
```

It reports:

- physical packed bytes per successfully inserted item;
- empirical false-positive rate over absent keys;
- nanoseconds per mixed lookup (50% present, 50% absent);
- nanoseconds per insertion; and
- the observed load at the first failed insertion for both cuckoo variants.

Bloom filters have no hard maximum load, so that column is reported as `unbounded`. Timing
results depend on the CPU, compiler, and system load. For useful results, close noisy
applications and repeat runs.

## Design notes

The Bloom filter uses Kirsch-Mitzenmacher double hashing and the optimal number of hash
probes for its allocated bit count. The conventional cuckoo filter stores `k + 3`-bit
fingerprints across eight candidate slots. The windowed filter stores `k + 2` bits across
four candidate slots and targets a practical load of 94%.

The implementation uses a deterministic, non-cryptographic 64-bit hash. It is suitable
for experiments and trusted inputs, not adversarial settings. This crate intentionally
uses no external Rust packages so the layouts and benchmark are easy to inspect.

## Reference

Johanna Elena Schmitz, Jens Zentgraf, and Sven Rahmann. [*Smaller and More Flexible Cuckoo
Filters*](https://arxiv.org/abs/2505.05847), arXiv:2505.05847.

## License

MIT
