//! Strong, reproducible passwords for generated Docker setups.
//!
//! A random 32-byte seed comes from the operating system CSPRNG. Each password is
//! derived from the seed and a label with SHA-256 in counter mode, so the preview
//! shown to the user and the files written later contain exactly the same values
//! while every project still gets different passwords. Passwords are 32
//! alphanumeric characters (~190 bits), safe inside URLs and connection strings.
use sha2::{Digest, Sha256};

pub(crate) const PASSWORD_LENGTH: usize = 32;
const ALPHABET: &[u8; 62] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const SEED_BYTES: usize = 32;

pub(crate) type Seed = [u8; SEED_BYTES];

/// A fresh seed from the OS CSPRNG.
pub(crate) fn new_seed() -> Result<Seed, String> {
    let mut seed = [0u8; SEED_BYTES];
    getrandom::fill(&mut seed).map_err(|e| {
        tr!(
            "Não foi possível gerar números aleatórios seguros: {e}",
            "Could not generate secure random numbers: {e}"
        )
    })?;
    Ok(seed)
}

pub(crate) fn seed_to_hex(seed: &Seed) -> String {
    seed.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn seed_from_hex(value: &str) -> Result<Seed, String> {
    if value.len() != SEED_BYTES * 2 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(tr!(
            "Semente de senhas inválida. Gere o preview novamente.",
            "Invalid password seed. Generate the preview again."
        ));
    }
    let mut seed = [0u8; SEED_BYTES];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| tr!("Semente de senhas inválida.", "Invalid password seed."))?;
    }
    Ok(seed)
}

/// Derives the password for `label`. Rejection sampling avoids modulo bias:
/// bytes >= 248 (the largest multiple of 62 below 256) are discarded.
pub(crate) fn password(seed: &Seed, label: &str) -> String {
    let mut out = String::with_capacity(PASSWORD_LENGTH);
    let mut counter: u32 = 0;
    while out.len() < PASSWORD_LENGTH {
        let mut hasher = Sha256::new();
        hasher.update(seed);
        hasher.update([0u8]);
        hasher.update(label.as_bytes());
        hasher.update(counter.to_be_bytes());
        for byte in hasher.finalize() {
            if byte < 248 && out.len() < PASSWORD_LENGTH {
                out.push(ALPHABET[(byte % 62) as usize] as char);
            }
        }
        counter += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn passwords_are_32_alphanumeric_characters() {
        let seed = new_seed().unwrap();
        for label in ["postgres", "mysql", "mysql-root", "mongodb", "rabbitmq"] {
            let value = password(&seed, label);
            assert_eq!(value.len(), PASSWORD_LENGTH);
            assert!(value.bytes().all(|b| b.is_ascii_alphanumeric()), "{value}");
        }
    }

    #[test]
    fn derivation_is_deterministic_and_separated_by_label_and_seed() {
        let a = new_seed().unwrap();
        let b = new_seed().unwrap();
        assert_ne!(a, b);
        assert_eq!(password(&a, "postgres"), password(&a, "postgres"));
        assert_ne!(password(&a, "postgres"), password(&a, "mysql"));
        assert_ne!(password(&a, "postgres"), password(&b, "postgres"));
        let all: HashSet<_> = (0..200).map(|i| password(&a, &format!("l{i}"))).collect();
        assert_eq!(all.len(), 200);
    }

    #[test]
    fn output_uses_the_whole_alphabet_without_obvious_bias() {
        let seed = new_seed().unwrap();
        let mut counts = [0usize; 62];
        for i in 0..400 {
            for byte in password(&seed, &format!("x{i}")).bytes() {
                counts[ALPHABET.iter().position(|a| *a == byte).unwrap()] += 1;
            }
        }
        // 12800 samples over 62 symbols: expected ~206 each. Stay far from the extremes.
        assert!(
            counts.iter().all(|&c| (120..320).contains(&c)),
            "{counts:?}"
        );
    }

    #[test]
    fn seed_hex_round_trips_and_rejects_bad_input() {
        let seed = new_seed().unwrap();
        assert_eq!(seed_from_hex(&seed_to_hex(&seed)).unwrap(), seed);
        for bad in ["", "zz", &"g".repeat(64), &"a".repeat(63), &"a".repeat(65)] {
            assert!(seed_from_hex(bad).is_err(), "{bad:?}");
        }
    }
}
