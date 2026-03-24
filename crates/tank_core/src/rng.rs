use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{de::Error as DeError, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimSeed(pub u64);

#[derive(Debug, Clone)]
pub struct SimRng {
    seed: SimSeed,
    draws: u64,
    rng: ChaCha8Rng,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SimRngRepr {
    seed: SimSeed,
    draws: u64,
}

impl SimRng {
    pub fn new(seed: SimSeed) -> Self {
        let rng = ChaCha8Rng::from_seed(seed_to_bytes(seed));
        Self {
            seed,
            draws: 0,
            rng,
        }
    }

    pub fn seed(&self) -> SimSeed {
        self.seed
    }

    pub fn draws(&self) -> u64 {
        self.draws
    }

    pub fn next_u64(&mut self) -> u64 {
        let value = self.rng.next_u64();
        self.draws += 1;
        value
    }

    pub fn next_f64(&mut self) -> f64 {
        let value = self.next_u64();
        (value as f64) / (u64::MAX as f64)
    }

    fn from_repr(repr: SimRngRepr) -> Self {
        let mut rng = ChaCha8Rng::from_seed(seed_to_bytes(repr.seed));
        // O(1) restore: set_word_pos takes 32-bit word offsets, and each
        // next_u64() call consumes two 32-bit words (64 bits).
        rng.set_word_pos(repr.draws as u128 * 2);
        Self {
            seed: repr.seed,
            draws: repr.draws,
            rng,
        }
    }
}

impl PartialEq for SimRng {
    fn eq(&self, other: &Self) -> bool {
        self.seed == other.seed && self.draws == other.draws
    }
}

impl Eq for SimRng {}

impl Serialize for SimRng {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SimRngRepr {
            seed: self.seed,
            draws: self.draws,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SimRng {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = SimRngRepr::deserialize(deserializer)?;
        if repr.draws > u32::MAX as u64 {
            return Err(D::Error::custom("draw count is unexpectedly large"));
        }
        Ok(Self::from_repr(repr))
    }
}

fn seed_to_bytes(seed: SimSeed) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    for (index, chunk) in bytes.chunks_mut(8).enumerate() {
        let word = seed
            .0
            .wrapping_add((index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    bytes
}
