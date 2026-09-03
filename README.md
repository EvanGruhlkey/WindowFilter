# WindowFilter

WindowFilter is a dependency-free Rust library for testing whether a key was probably
seen before. It stores compact fingerprints or bits instead of complete keys, so it can
reject missing keys using much less memory than an exact set. Present keys are never
missed, but absent keys have a small, configurable chance of being reported as present.

## Motivation

Applications often need to check a large collection before doing something expensive,
such as reading from disk, querying a database, or processing a duplicate item. An exact
set can answer that question, but storing every complete key becomes costly at large
scales. An approximate membership filter provides a useful first check: a negative answer
is certain, while a positive answer can be confirmed by the underlying data source.

This project builds three filters from scratch to make their memory and performance
tradeoffs easy to compare. Its main goal is to explore how the overlapping-window layout
from *Smaller and More Flexible Cuckoo Filters* reduces space while preserving deletion
and fast lookups. The implementation uses bit-packed storage and includes the same
benchmark harness for all three designs, making the cost of each layout directly visible.

## Implementations

- `BloomFilter` sets several positions in a bit array. It is compact and simple, but it
  cannot safely delete individual keys.
- `CuckooFilter` stores fingerprints in two four-slot buckets. It supports deletion and
  fast relocation, but its power-of-two bucket count can allocate more memory than needed.
- `WindowedCuckooFilter` stores fingerprints in two overlapping two-slot windows. It uses
  arbitrary window counts and fewer bits per item, trading slower insertion for lower
  memory use.

The windowed filter stores a `k`-bit fingerprint, one bit identifying the selected window,
and one bit identifying the position inside that window. Signed offsets let it recover a
fingerprint's alternative window without requiring a power-of-two table size. All three
filters use compact, bit-packed storage.

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
cuckoo filters support deletion, but, as with cuckoo filters generally, call `delete` only
for keys known to have been inserted. Deleting a false-positive key can remove a colliding
fingerprint.

## Benchmark

```text
filter                     bytes/item false-positive    lookup ns    insert ns     max load
Bloom                          1.8034       0.001015        81.59        93.61    unbounded
Cuckoo (2,4)                   2.1299       0.000820        61.19        58.74       0.9780
Windowed cuckoo (2,2)          1.5958       0.000955        60.28       162.97       0.9575
```

The windowed cuckoo filter used the least memory at 1.5958 bytes per item. That is about
12% less memory than the Bloom filter and 25% less than the conventional cuckoo filter.
Its 60.28-nanosecond lookup time was also the fastest result in this run.

The memory savings come with slower writes. A windowed insertion took 162.97 nanoseconds,
compared with 58.74 nanoseconds for the conventional cuckoo filter. Moving fingerprints
through overlapping windows requires more work when slots are occupied.

All three false-positive rates were close to the `2^-10` target of about 0.000977, or one
false positive per 1,024 absent keys. Small differences between the three measurements
are expected when testing a finite number of queries.

The conventional cuckoo filter reached the highest load at 97.80% and had the fastest
insertions, but used the most memory because its bucket count must be a power of two. The
windowed filter reached 95.75% while allocating memory more closely to the requested size.
The Bloom filter has no fixed maximum load, but it does not support deletion.

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
