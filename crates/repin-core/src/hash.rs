use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    Blake3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContentHash {
    pub algorithm: HashAlgorithm,
    #[serde(with = "hex_bytes")]
    pub digest: [u8; 32],
}

impl ContentHash {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let hash = blake3::hash(bytes);
        Self {
            algorithm: HashAlgorithm::Blake3,
            digest: *hash.as_bytes(),
        }
    }

    pub fn to_hex(&self) -> String {
        let prefix = match self.algorithm {
            HashAlgorithm::Blake3 => "blake3",
        };
        format!("{}:{}", prefix, hex::encode(self.digest))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

mod hex {
    pub fn encode(bytes: [u8; 32]) -> String {
        let mut s = String::with_capacity(64);
        for b in bytes {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&super::hex::encode(*bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.len() != 64 {
            return Err(de::Error::custom("expected 64 hex characters"));
        }
        let mut out = [0u8; 32];
        for (i, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
            let hex_str = std::str::from_utf8(chunk).map_err(de::Error::custom)?;
            out[i] = u8::from_str_radix(hex_str, 16).map_err(de::Error::custom)?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = ContentHash::of_bytes(b"hello world");
        let h2 = ContentHash::of_bytes(b"hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.algorithm, HashAlgorithm::Blake3);
    }

    #[test]
    fn test_content_hash_serde_roundtrip() {
        let h = ContentHash::of_bytes(b"foo bar");
        let json = serde_json::to_string(&h).unwrap();
        let deserialized: ContentHash = serde_json::from_str(&json).unwrap();
        assert_eq!(h, deserialized);
    }
}
