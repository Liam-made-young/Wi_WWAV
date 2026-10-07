//! Ids shared by every part of the app.
//!
//! - ULIDs name every media file and session: 48 bits of milliseconds and 80
//!   random bits in Crockford base32, monotonic within a millisecond, so they
//!   sort by creation time as plain strings (MI-WWAV-OS `engine/src/ids.rs`).
//! - A `song_id` or `film_id` is 128 bits as 32 lowercase hex (`docs/SPEC.md`
//!   6.1). A song that came from the platform takes FNV-1a 128 of
//!   `"wwav-track:" + trackId` (`prana/web/src/sim/wwavdisc.js`).
//! - FNV-1a 32 and 64 are the stable hashes layouts derive from ("never any
//!   system RNG").

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const RAND_MASK: u128 = (1u128 << 80) - 1;

struct Mono {
    last_ms: u64,
    last_rand: u128,
}

static MONO: Mutex<Mono> = Mutex::new(Mono {
    last_ms: 0,
    last_rand: 0,
});

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).expect("OS entropy unavailable");
    buf
}

fn random80() -> u128 {
    random_bytes::<10>()
        .iter()
        .fold(0u128, |v, &b| (v << 8) | b as u128)
}

/// A new ULID. Within one millisecond the random part counts up by one, and a
/// clock that steps backwards keeps the last millisecond, so ids from one
/// process always sort in the order they were made.
pub fn ulid() -> String {
    let now = now_ms();
    let (ms, rand) = {
        let mut m = MONO.lock().unwrap_or_else(|e| e.into_inner());
        if now <= m.last_ms {
            m.last_rand = (m.last_rand + 1) & RAND_MASK;
            if m.last_rand == 0 {
                m.last_ms += 1; // 2^80 ids in one millisecond: borrow the next one
            }
        } else {
            m.last_ms = now;
            m.last_rand = random80();
        }
        (m.last_ms, m.last_rand)
    };
    encode_ulid(ms, rand)
}

/// The 26-character Crockford base32 form of a timestamp and 80 random bits.
pub fn encode_ulid(ms: u64, rand: u128) -> String {
    let n: u128 = ((ms as u128 & ((1u128 << 48) - 1)) << 80) | (rand & RAND_MASK);
    let mut out = [0u8; 26];
    for (i, c) in out.iter_mut().enumerate() {
        let shift = (25 - i) * 5;
        *c = CROCKFORD[((n >> shift) & 0x1f) as usize];
    }
    String::from_utf8(out.to_vec()).expect("ascii")
}

/// The 128-bit value of a ULID, or None if it isn't one.
pub fn decode_ulid(s: &str) -> Option<u128> {
    if s.len() != 26 {
        return None;
    }
    let mut n: u128 = 0;
    for (i, b) in s.bytes().enumerate() {
        let v = CROCKFORD
            .iter()
            .position(|&c| c == b.to_ascii_uppercase())? as u128;
        if i == 0 && v > 7 {
            return None; // over 128 bits
        }
        n = (n << 5) | v;
    }
    Some(n)
}

pub fn is_ulid(s: &str) -> bool {
    decode_ulid(s).is_some() && s.bytes().all(|b| !b.is_ascii_lowercase())
}

/// When a ULID was made, in milliseconds since the epoch.
pub fn ulid_ms(s: &str) -> Option<u64> {
    decode_ulid(s).map(|n| (n >> 80) as u64)
}

/// 128 random bits as 32 lowercase hex: a new `song_id` or `film_id`.
pub fn new_work_id() -> String {
    hex(&random_bytes::<16>())
}

pub fn is_work_id(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

const FNV128_OFFSET: u128 = 0x6c62272e07bb014262b821756295c58d;
const FNV128_PRIME: u128 = 0x0000000001000000000000000000013b;

/// FNV-1a 128 of `bytes`.
pub fn fnv1a128(bytes: &[u8]) -> u128 {
    bytes.iter().fold(FNV128_OFFSET, |h, &b| {
        (h ^ b as u128).wrapping_mul(FNV128_PRIME)
    })
}

/// The `song_id` of a song that came from the platform: FNV-1a 128 of
/// `"wwav-track:" + trackId` as 32 lowercase hex, "so a song has the same id
/// everywhere and its remixes share a parent" (`prana/SPEC.md` §12).
pub fn song_id_for_track(track_id: &str) -> String {
    format!(
        "{:032x}",
        fnv1a128(format!("wwav-track:{track_id}").as_bytes())
    )
}

/// FNV-1a 32.
pub fn fnv1a32(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5u32, |h, &b| {
        (h ^ b as u32).wrapping_mul(0x0100_0193)
    })
}

/// FNV-1a 64.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ulids_are_26_chars_unique_and_sorted() {
        let mut prev = String::new();
        for _ in 0..20_000 {
            let id = ulid();
            assert_eq!(id.len(), 26);
            assert!(is_ulid(&id));
            assert!(id > prev, "ULIDs must sort ascending: {prev} then {id}");
            prev = id;
        }
    }

    #[test]
    fn ulid_round_trips_and_carries_its_time() {
        let id = encode_ulid(1_759_795_200_000, 0x1234_5678_9abc_def0_1234);
        assert_eq!(ulid_ms(&id), Some(1_759_795_200_000));
        assert_eq!(
            decode_ulid(&id).unwrap() & RAND_MASK,
            0x1234_5678_9abc_def0_1234
        );
        assert!(decode_ulid("8ZZZZZZZZZZZZZZZZZZZZZZZZZ").is_none());
        assert!(!is_ulid("01jc5q8v3m2t7r9x4k6w0yhznb"));
        let before = now_ms();
        let ms = ulid_ms(&ulid()).unwrap();
        assert!(ms >= before && ms <= now_ms() + 1);
    }

    #[test]
    fn earlier_times_sort_first_as_strings() {
        assert!(encode_ulid(1, RAND_MASK) < encode_ulid(2, 0));
    }

    #[test]
    fn song_ids_match_prana_wwavdisc() {
        // Computed with songIdOf() in prana/web/src/sim/wwavdisc.js.
        assert_eq!(song_id_for_track("42"), "b0e220c3923bbb508b02d627e8350786");
        assert_eq!(
            song_id_for_track("6650f0a1c2b3"),
            "498775e1ade30f8ab670f3131b91df7f"
        );
        assert_eq!(
            song_id_for_track("trk_ä"),
            "82147334438dbf043dc6ee2b2807a9d7"
        );
    }

    #[test]
    fn work_ids_are_32_lowercase_hex() {
        let id = new_work_id();
        assert!(is_work_id(&id), "{id}");
        assert_ne!(id, new_work_id());
        assert!(!is_work_id("B0E220C3923BBB508B02D627E8350786"));
    }

    #[test]
    fn fnv_vectors() {
        // Published FNV-1a test vectors.
        assert_eq!(fnv1a32(b""), 0x811c9dc5);
        assert_eq!(fnv1a32(b"a"), 0xe40c292c);
        assert_eq!(fnv1a32(b"foobar"), 0xbf9cf968);
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
    }
}
