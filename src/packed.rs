#[derive(Clone, Debug)]
pub(crate) struct PackedSlots {
    words: Vec<u64>,
    len: usize,
    width: u8,
    mask: u64,
}

impl PackedSlots {
    pub(crate) fn new(len: usize, width: u8) -> Self {
        assert!((1..=32).contains(&width));
        let bit_len = len
            .checked_mul(width as usize)
            .expect("slot allocation overflow");
        Self {
            words: vec![0; bit_len.div_ceil(64)],
            len,
            width,
            mask: (1_u64 << width) - 1,
        }
    }

    pub(crate) fn get(&self, index: usize) -> u32 {
        assert!(index < self.len);
        let bit = index * self.width as usize;
        let word = bit / 64;
        let shift = bit % 64;
        let mut value = self.words[word] >> shift;
        if shift + self.width as usize > 64 {
            value |= self.words[word + 1] << (64 - shift);
        }
        (value & self.mask) as u32
    }

    pub(crate) fn set(&mut self, index: usize, value: u32) {
        assert!(index < self.len);
        let value = u64::from(value) & self.mask;
        let bit = index * self.width as usize;
        let word = bit / 64;
        let shift = bit % 64;
        self.words[word] &= !(self.mask << shift);
        self.words[word] |= value << shift;

        let spill = shift + self.width as usize;
        if spill > 64 {
            let upper_bits = spill - 64;
            let upper_mask = (1_u64 << upper_bits) - 1;
            self.words[word + 1] &= !upper_mask;
            self.words[word + 1] |= value >> (64 - shift);
        }
    }

    pub(crate) fn swap(&mut self, index: usize, value: u32) -> u32 {
        let previous = self.get(index);
        self.set(index, value);
        previous
    }

    pub(crate) fn bytes(&self) -> usize {
        self.words.len() * size_of::<u64>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_survive_word_boundaries() {
        for width in 1..=32 {
            let mut slots = PackedSlots::new(137, width);
            let mask = (1_u64 << width) - 1;
            for index in 0..137 {
                slots.set(index, ((index as u64 * 37 + 11) & mask) as u32);
            }
            for index in 0..137 {
                assert_eq!(slots.get(index), ((index as u64 * 37 + 11) & mask) as u32);
            }
        }
    }

    #[test]
    fn swap_returns_old_value() {
        let mut slots = PackedSlots::new(3, 9);
        slots.set(1, 17);
        assert_eq!(slots.swap(1, 42), 17);
        assert_eq!(slots.get(1), 42);
    }
}
