use crate::hash::{hash, mix};
use crate::packed::PackedSlots;

const BUCKET_SIZE: usize = 4;
const DEFAULT_MAX_KICKS: usize = 10_000;

/// A standard `(2, 4)` bucketed cuckoo filter.
#[derive(Clone, Debug)]
pub struct CuckooFilter {
    slots: PackedSlots,
    bucket_count: usize,
    fingerprint_bits: u8,
    len: usize,
    rng: u64,
    max_kicks: usize,
}

impl CuckooFilter {
    /// Creates a filter for `capacity` keys and a target FPR of `2^-fpr_bits`.
    pub fn new(capacity: usize, fpr_bits: u8) -> Self {
        assert!(capacity > 0, "capacity must be positive");
        assert!(
            (1..=29).contains(&fpr_bits),
            "false-positive bits must be in 1..=29"
        );

        let required_slots = (capacity as f64 / 0.95).ceil() as usize;
        let bucket_count = required_slots
            .div_ceil(BUCKET_SIZE)
            .max(2)
            .next_power_of_two();
        let fingerprint_bits = fpr_bits + 3;
        Self {
            slots: PackedSlots::new(bucket_count * BUCKET_SIZE, fingerprint_bits),
            bucket_count,
            fingerprint_bits,
            len: 0,
            rng: 0x6a09e667f3bcc909,
            max_kicks: DEFAULT_MAX_KICKS,
        }
    }

    pub fn with_max_kicks(mut self, max_kicks: usize) -> Self {
        assert!(max_kicks > 0, "maximum kicks must be positive");
        self.max_kicks = max_kicks;
        self
    }

    pub fn insert(&mut self, key: &str) -> bool {
        let fingerprint = self.fingerprint(key);
        let first = self.first_bucket(key);
        let second = self.alternate(first, fingerprint);

        if self.insert_empty(first, fingerprint) || self.insert_empty(second, fingerprint) {
            self.len += 1;
            return true;
        }

        let mut bucket = if self.random() & 1 == 0 {
            first
        } else {
            second
        };
        let mut carried = fingerprint;
        let mut path = Vec::with_capacity(self.max_kicks);
        for _ in 0..self.max_kicks {
            let slot = bucket * BUCKET_SIZE + self.random() as usize % BUCKET_SIZE;
            let displaced = self.slots.swap(slot, carried);
            path.push((slot, displaced));
            carried = displaced;
            bucket = self.alternate(bucket, carried);
            if self.insert_empty(bucket, carried) {
                self.len += 1;
                return true;
            }
        }

        for (slot, previous) in path.into_iter().rev() {
            self.slots.set(slot, previous);
        }
        false
    }

    pub fn contains(&self, key: &str) -> bool {
        let fingerprint = self.fingerprint(key);
        let first = self.first_bucket(key);
        self.bucket_contains(first, fingerprint)
            || self.bucket_contains(self.alternate(first, fingerprint), fingerprint)
    }

    pub fn delete(&mut self, key: &str) -> bool {
        let fingerprint = self.fingerprint(key);
        let first = self.first_bucket(key);
        let second = self.alternate(first, fingerprint);
        for bucket in [first, second] {
            for offset in 0..BUCKET_SIZE {
                let slot = bucket * BUCKET_SIZE + offset;
                if self.slots.get(slot) == fingerprint {
                    self.slots.set(slot, 0);
                    self.len -= 1;
                    return true;
                }
            }
        }
        false
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn slot_count(&self) -> usize {
        self.bucket_count * BUCKET_SIZE
    }

    pub fn memory_bytes(&self) -> usize {
        self.slots.bytes()
    }

    pub fn load_factor(&self) -> f64 {
        self.len as f64 / self.slot_count() as f64
    }

    pub fn fingerprint_bits(&self) -> u8 {
        self.fingerprint_bits
    }

    fn fingerprint(&self, key: &str) -> u32 {
        let mask = (1_u64 << self.fingerprint_bits) - 1;
        (hash(key, 0xa4093822299f31d0) & mask).max(1) as u32
    }

    fn first_bucket(&self, key: &str) -> usize {
        hash(key, 0x082efa98ec4e6c89) as usize & (self.bucket_count - 1)
    }

    fn alternate(&self, bucket: usize, fingerprint: u32) -> usize {
        let mut offset = mix(u64::from(fingerprint) ^ 0x452821e638d01377) as usize;
        offset &= self.bucket_count - 1;
        bucket ^ offset.max(1)
    }

    fn insert_empty(&mut self, bucket: usize, fingerprint: u32) -> bool {
        for offset in 0..BUCKET_SIZE {
            let slot = bucket * BUCKET_SIZE + offset;
            if self.slots.get(slot) == 0 {
                self.slots.set(slot, fingerprint);
                return true;
            }
        }
        false
    }

    fn bucket_contains(&self, bucket: usize, fingerprint: u32) -> bool {
        (0..BUCKET_SIZE).any(|offset| self.slots.get(bucket * BUCKET_SIZE + offset) == fingerprint)
    }

    fn random(&mut self) -> u64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        self.rng
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_insert_lookup_and_delete() {
        let mut filter = CuckooFilter::new(1_000, 10);
        assert!(filter.insert("evan"));
        assert!(filter.contains("evan"));
        assert!(!filter.contains("bob"));
        assert!(filter.delete("evan"));
        assert!(!filter.contains("evan"));
        assert!(filter.is_empty());
    }

    #[test]
    fn retains_every_successful_insert() {
        let mut filter = CuckooFilter::new(2_000, 12).with_max_kicks(2_000);
        let mut inserted = Vec::new();
        for value in 0..2_000 {
            let key = format!("key-{value}");
            assert!(filter.insert(&key));
            inserted.push(key);
        }
        assert!(inserted.iter().all(|key| filter.contains(key)));
        assert_eq!(filter.len(), 2_000);
        assert_eq!(filter.fingerprint_bits(), 15);
    }

    #[test]
    fn failed_insert_rolls_back_relocations() {
        let mut filter = CuckooFilter::new(1, 20).with_max_kicks(16);
        let mut inserted = Vec::new();
        for value in 0..100 {
            let key = format!("tiny-{value}");
            if filter.insert(&key) {
                inserted.push(key);
            } else {
                assert!(inserted.iter().all(|key| filter.contains(key)));
                return;
            }
        }
        panic!("tiny filter never filled");
    }
}
