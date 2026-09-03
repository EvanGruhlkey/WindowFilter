use crate::hash::hash;

const LN_2: f64 = std::f64::consts::LN_2;

/// A space-efficient Bloom filter with no false negatives.
///
/// Bloom filters do not support deletion because each bit can belong to many keys.
#[derive(Clone, Debug)]
pub struct BloomFilter {
    bits: Vec<u64>,
    bit_count: usize,
    hash_count: u32,
    len: usize,
    capacity: usize,
}

impl BloomFilter {
    /// Creates a filter sized for `capacity` keys and a target FPR of `2^-fpr_bits`.
    pub fn new(capacity: usize, fpr_bits: u8) -> Self {
        assert!(capacity > 0, "capacity must be positive");
        assert!(fpr_bits > 0, "false-positive bits must be positive");

        let requested_bits = (capacity as f64 * f64::from(fpr_bits) / LN_2).ceil() as usize;
        let words = requested_bits.max(64).div_ceil(64);
        let bit_count = words * 64;
        let hash_count = ((bit_count as f64 / capacity as f64) * LN_2)
            .round()
            .max(1.0) as u32;
        Self {
            bits: vec![0; words],
            bit_count,
            hash_count,
            len: 0,
            capacity,
        }
    }

    /// Adds a key. A Bloom filter cannot become full, so this always returns `true`.
    pub fn insert(&mut self, key: &str) -> bool {
        let (first, step) = self.base_hashes(key);
        for index in Self::locations(first, step, self.hash_count, self.bit_count) {
            self.bits[index / 64] |= 1_u64 << (index % 64);
        }
        self.len += 1;
        true
    }

    pub fn contains(&self, key: &str) -> bool {
        let (first, step) = self.base_hashes(key);
        Self::locations(first, step, self.hash_count, self.bit_count)
            .all(|index| self.bits[index / 64] & (1_u64 << (index % 64)) != 0)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn memory_bytes(&self) -> usize {
        self.bits.len() * size_of::<u64>()
    }

    pub fn bit_load_factor(&self) -> f64 {
        let set = self
            .bits
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum::<usize>();
        set as f64 / self.bit_count as f64
    }

    pub fn estimated_false_positive_rate(&self) -> f64 {
        let exponent = -(self.hash_count as f64 * self.len as f64 / self.bit_count as f64);
        (1.0 - exponent.exp()).powi(self.hash_count as i32)
    }

    fn base_hashes(&self, key: &str) -> (u64, u64) {
        (
            hash(key, 0x243f6a8885a308d3),
            hash(key, 0x13198a2e03707344) | 1,
        )
    }

    fn locations(
        first: u64,
        step: u64,
        hash_count: u32,
        bit_count: usize,
    ) -> impl Iterator<Item = usize> {
        (0..hash_count).map(move |round| {
            first.wrapping_add(u64::from(round).wrapping_mul(step)) as usize % bit_count
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserted_keys_are_always_found() {
        let mut filter = BloomFilter::new(1_000, 10);
        for value in 0..1_000 {
            assert!(filter.insert(&format!("key-{value}")));
        }
        for value in 0..1_000 {
            assert!(filter.contains(&format!("key-{value}")));
        }
    }

    #[test]
    fn reports_compact_storage() {
        let filter = BloomFilter::new(1_000, 10);
        assert!(filter.memory_bytes() < 2_000);
        assert_eq!(filter.capacity(), 1_000);
        assert!(filter.is_empty());
    }
}
