//! What the export sheet says about length, size and conversion (5.13, 6.1,
//! 6.7). A `.wwav` is 20 bytes a frame at 44.1 kHz (4 for the master, 16 for
//! the stems) plus its chunks, and the packer refuses anything over 4 GB,
//! RIFF's 32-bit size, which is about 81 minutes.

use crate::resample::resampled_len;

/// Every `.wwav` 0.1 is 44.1 kHz, 16-bit.
pub const WWAV_RATE: u32 = 44_100;

/// The largest file `wwav_pack.py pack` writes.
pub const MAX_WWAV_BYTES: u64 = 0xFFFF_FFFF;

/// The stems start on a 512-byte boundary, one storage block (`ALIGN`).
const ALIGN: u64 = 512;

/// 6.7, for a `.swav` export.
pub const SWAV_SOUND_LINE: &str =
    "The film's sound is AAC at 320 kbps. The .wwav beside it holds the lossless stems.";

/// Frames in the `.wwav` of a session: the resampler's exact output length.
pub fn wwav_frames(session_frames: u64, session_rate: u32) -> u64 {
    resampled_len(session_frames, session_rate, WWAV_RATE)
}

/// The file's size in bytes, laid out as `pack` lays it out: a 44-byte head
/// and the master, `wmet`, `wstm` (a 16-byte header, zeros to the next
/// 512-byte boundary, the stems), `wlin`, then `wrmx` for a remix. The
/// lengths are the JSON bodies; each chunk adds 8 bytes and pads to even.
pub fn wwav_bytes(frames: u64, wmet: u64, wlin: u64, wrmx: Option<u64>) -> u64 {
    let chunk = |len: u64| 8 + len + (len & 1);
    let stems_header_at = 44 + frames * 4 + chunk(wmet) + 8;
    let pad = (ALIGN - (stems_header_at + 16) % ALIGN) % ALIGN;
    stems_header_at + 16 + pad + frames * 16 + chunk(wlin) + wrmx.map_or(0, chunk)
}

/// Whole minutes a `.wwav` holds: 81.
pub fn minutes_held() -> u64 {
    MAX_WWAV_BYTES / (20 * WWAV_RATE as u64 * 60)
}

/// "3:58 → about 210 MB." for `frames` at 44.1 kHz (`wwav_frames`) and the
/// file's `wwav_bytes`. Megabytes are 1,000,000 bytes, as 6.1's table counts
/// them; from 1,000 MB the size reads in gigabytes to one decimal.
pub fn size_line(frames: u64, bytes: u64) -> String {
    let secs = frames / WWAV_RATE as u64;
    let mb = (bytes + 500_000) / 1_000_000;
    let size = if mb < 1_000 {
        format!("{mb} MB")
    } else {
        let tenths = (bytes + 50_000_000) / 100_000_000;
        format!("{}.{} GB", tenths / 10, tenths % 10)
    };
    format!("{}:{:02} → about {size}.", secs / 60, secs % 60)
}

/// The refusal when the file would pass the limit: "A .wwav holds about 81
/// minutes. This session is 94." Takes the same `frames` and `bytes` as
/// `size_line`. The session's minutes round up, so a refused session never
/// reads as one that fits.
pub fn refusal(frames: u64, bytes: u64) -> Option<String> {
    if bytes <= MAX_WWAV_BYTES {
        return None;
    }
    let minutes = frames.div_ceil(WWAV_RATE as u64 * 60);
    Some(format!(
        "A .wwav holds about {} minutes. This session is {minutes}.",
        minutes_held()
    ))
}

/// What exporting a `.wwav` from a session at this rate changes.
pub fn conversion_line(session_rate: u32) -> String {
    if session_rate == WWAV_RATE {
        "The .wwav will be 16-bit, dithered.".to_string()
    } else {
        format!(
            "This session is {} kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered.",
            khz(session_rate)
        )
    }
}

/// What posting a WAV at this rate and depth needs, or None when it is
/// already 44.1 kHz, 16-bit. The sheet offers **Convert a copy** beside it.
pub fn posting_line(rate: u32, bits: u16) -> Option<String> {
    if rate == WWAV_RATE && bits == 16 {
        return None;
    }
    Some(format!(
        "This is {} kHz, {bits}-bit. Posting needs 44.1 kHz, 16-bit.",
        khz(rate)
    ))
}

/// 48000 → "48", 44100 → "44.1", 22050 → "22.05".
fn khz(rate: u32) -> String {
    let (whole, part) = (rate / 1_000, rate % 1_000);
    if part == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{part:03}")
            .trim_end_matches('0')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_read_in_khz() {
        assert_eq!(khz(48_000), "48");
        assert_eq!(khz(44_100), "44.1");
        assert_eq!(khz(22_050), "22.05");
        assert_eq!(khz(88_200), "88.2");
        assert_eq!(khz(8_000), "8");
        assert_eq!(khz(11_025), "11.025");
    }
}
