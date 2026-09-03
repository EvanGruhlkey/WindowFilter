const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

pub(crate) fn hash(key: &str, seed: u64) -> u64 {
    let mut state = FNV_OFFSET ^ seed;
    for byte in key.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    mix(state ^ key.len() as u64)
}

pub(crate) fn mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58476d1ce4e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashing_is_stable_and_seeded() {
        assert_eq!(hash("evan", 7), hash("evan", 7));
        assert_ne!(hash("evan", 7), hash("evan", 8));
        assert_ne!(hash("evan", 7), hash("bob", 7));
    }
}

