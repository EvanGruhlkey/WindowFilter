use crate::hash::{hash, mix};
use crate::packed::PackedSlots;

const WINDOW_SIZE: usize = 2;
const DEFAULT_MAX_KICKS: usize = 500;

/// A `(2, 2)` cuckoo filter whose adjacent windows overlap by one slot.
#[derive(Clone, Debug)]
pub struct WindowedCuckooFilter {
    slots: PackedSlots,
    window_count: usize,
    fingerprint_bits: u8,
    len: usize,
    rng: u64,
    max_kicks: usize,
}

impl WindowedCuckooFilter {
    /// Creates a filter for `capacity` keys and a target FPR of `2^-fpr_bits`.
    pub fn new(capacity: usize, fpr_bits: u8) -> Self {
        assert!(capacity > 0, "capacity must be positive");
        assert!(
            (1..=30).contains(&fpr_bits),
            "false-positive bits must be in 1..=30"
        );

        let slot_count = (capacity as f64 / 0.94).ceil().max(3.0) as usize;
        let window_count = slot_count - WINDOW_SIZE + 1;
        Self {
            slots: PackedSlots::new(slot_count, fpr_bits + 2),
            window_count,
            fingerprint_bits: fpr_bits,
            len: 0,
            rng: 0xbb67ae8584caa73b,
            max_kicks: DEFAULT_MAX_KICKS,
        }
    }

    pub fn with_max_kicks(mut self, max_kicks: usize) -> Self {
        assert!(max_kicks > 0, "maximum kicks must be positive");
        self.max_kicks = max_kicks;
        self
    }

    pub fn insert(&mut self, key: &str) -> bool {
        let mut fingerprint = self.fingerprint(key);
        let mut first = self.first_window(key);
        let mut second = self.add_offset(first, fingerprint);
        if self.insert_empty(fingerprint, first, second) {
            self.len += 1;
            return true;
        }

        let mut path = Vec::with_capacity(self.max_kicks);
        for _ in 0..self.max_kicks {
            let coordinate = self.random() as usize % (2 * WINDOW_SIZE);
            let choice = coordinate / WINDOW_SIZE;
            let window_offset = coordinate % WINDOW_SIZE;
            let window = if choice == 0 { first } else { second };
            let slot = window + window_offset;
            let encoded = self.encode(fingerprint, choice, window_offset);
            let displaced = self.slots.swap(slot, encoded);
            path.push((slot, displaced));

            fingerprint = self.decode_fingerprint(displaced);
            let old_offset = self.decode_window_offset(displaced);
            let old_choice = self.decode_choice(displaced);
            let current = slot - old_offset;
            if old_choice == 0 {
                first = current;
                second = self.add_offset(current, fingerprint);
            } else {
                second = current;
                first = self.subtract_offset(current, fingerprint);
            }
            if self.insert_empty(fingerprint, first, second) {
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
        let first = self.first_window(key);
        let second = self.add_offset(first, fingerprint);
        Self::coordinates(first, second).any(|(slot, choice, offset)| {
            self.slots.get(slot) == self.encode(fingerprint, choice, offset)
        })
    }

    pub fn delete(&mut self, key: &str) -> bool {
        let fingerprint = self.fingerprint(key);
        let first = self.first_window(key);
        let second = self.add_offset(first, fingerprint);
        for (slot, choice, offset) in Self::coordinates(first, second) {
            if self.slots.get(slot) == self.encode(fingerprint, choice, offset) {
                self.slots.set(slot, 0);
                self.len -= 1;
                return true;
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
        self.window_count + WINDOW_SIZE - 1
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

    pub fn bits_per_slot(&self) -> u8 {
        self.fingerprint_bits + 2
    }

    fn insert_empty(&mut self, fingerprint: u32, first: usize, second: usize) -> bool {
        for (slot, choice, offset) in Self::coordinates(first, second) {
            if self.slots.get(slot) == 0 {
                self.slots
                    .set(slot, self.encode(fingerprint, choice, offset));
                return true;
            }
        }
        false
    }

    fn coordinates(first: usize, second: usize) -> impl Iterator<Item = (usize, usize, usize)> {
        [first, second]
            .into_iter()
            .enumerate()
            .flat_map(|(choice, window)| {
                (0..WINDOW_SIZE).map(move |offset| (window + offset, choice, offset))
            })
    }

    fn fingerprint(&self, key: &str) -> u32 {
        let mask = (1_u64 << self.fingerprint_bits) - 1;
        (hash(key, 0x3c6ef372fe94f82b) & mask).max(1) as u32
    }

    fn first_window(&self, key: &str) -> usize {
        hash(key, 0xa54ff53a5f1d36f1) as usize % self.window_count
    }

    fn fingerprint_offset(&self, fingerprint: u32) -> usize {
        1 + mix(u64::from(fingerprint) ^ 0x510e527fade682d1) as usize % (self.window_count - 1)
    }

    fn add_offset(&self, window: usize, fingerprint: u32) -> usize {
        (window + self.fingerprint_offset(fingerprint)) % self.window_count
    }

    fn subtract_offset(&self, window: usize, fingerprint: u32) -> usize {
        let offset = self.fingerprint_offset(fingerprint);
        if window >= offset {
            window - offset
        } else {
            self.window_count - (offset - window)
        }
    }

    fn encode(&self, fingerprint: u32, choice: usize, window_offset: usize) -> u32 {
        fingerprint
            | (window_offset as u32) << self.fingerprint_bits
            | (choice as u32) << (self.fingerprint_bits + 1)
    }

    fn decode_fingerprint(&self, encoded: u32) -> u32 {
        encoded & ((1_u32 << self.fingerprint_bits) - 1)
    }

    fn decode_window_offset(&self, encoded: u32) -> usize {
        (encoded >> self.fingerprint_bits) as usize & 1
    }

    fn decode_choice(&self, encoded: u32) -> usize {
        (encoded >> (self.fingerprint_bits + 1)) as usize & 1
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
        let mut filter = WindowedCuckooFilter::new(1_000, 10);
        assert!(filter.insert("evan"));
        assert!(filter.contains("evan"));
        assert!(!filter.contains("bob"));
        assert!(filter.delete("evan"));
        assert!(!filter.contains("evan"));
        assert!(filter.is_empty());
    }

    #[test]
    fn retains_keys_at_the_design_load() {
        let mut filter = WindowedCuckooFilter::new(2_000, 12).with_max_kicks(2_000);
        let mut inserted = Vec::new();
        for value in 0..2_000 {
            let key = format!("key-{value}");
            assert!(filter.insert(&key), "insertion failed at {value}");
            inserted.push(key);
        }
        assert!(inserted.iter().all(|key| filter.contains(key)));
        assert_eq!(filter.len(), 2_000);
        assert_eq!(filter.fingerprint_bits(), 12);
        assert_eq!(filter.bits_per_slot(), 14);
        assert!(filter.load_factor() >= 0.939);
    }

    #[test]
    fn uses_flexible_non_power_of_two_storage() {
        let filter = WindowedCuckooFilter::new(1_000, 10);
        assert!(!filter.slot_count().is_power_of_two());
        assert_eq!(filter.memory_bytes(), 1_600);
    }
}
