//! F9: a session over 81 minutes isn't refused with the spec's sentence
//! (`docs/PLAN.md`). Also the export sheet's size line (5.13, 6.1) and the
//! conversions said out loud (6.7).

use wwav_dsp::sheet::{
    conversion_line, minutes_held, posting_line, refusal, size_line, wwav_bytes, wwav_frames,
    SWAV_SOUND_LINE,
};

/// `wmet` and `wlin` of a typical original, as `wwav_pack.py` writes them.
const WMET: &str = r#"{"wwav": "0.1", "song_id": "0123456789abcdef0123456789abcdef", "title": "Test Song", "artist": "LMY", "bpm": 86, "key": "A minor", "frames": 10495800, "type": "original", "created": "2026-10-07"}"#;
const WLIN: &str = r#"{"parent_id": null, "root_id": "0123456789abcdef0123456789abcdef", "generation": 0, "creator": "", "device_id": ""}"#;

#[test]
fn bytes_match_files_the_packer_wrote() {
    // Sizes of files `wwav_pack.py pack` wrote from silent song folders
    // (2026-10-07): frames, wmet and wlin lengths, bytes. The first has an
    // odd wmet, padded to even; both pad wstm's header to 512 bytes.
    assert_eq!(wwav_bytes(1_001, 191, 115, None), 20_748);
    assert_eq!(wwav_bytes(132_307, 194, 115, None), 2_646_956);
    // A remix adds wrmx after wlin, padded to even like every chunk.
    assert_eq!(wwav_bytes(1_001, 191, 115, Some(301)), 20_748 + 8 + 301 + 1);
    // An empty song is all overhead: head, wmet, wstm's header and pad, wlin.
    assert_eq!(wwav_bytes(0, 2, 2, None), 44 + 8 + 2 + 8 + 16 + 434 + 8 + 2);
}

#[test]
fn three_fifty_eight_is_about_210_mb() {
    let frames = 238 * 44_100;
    let bytes = wwav_bytes(frames, WMET.len() as u64, WLIN.len() as u64, None);
    assert_eq!((WMET.len(), WLIN.len()), (195, 115));
    assert_eq!(size_line(frames, bytes), "3:58 → about 210 MB.");
}

#[test]
fn sizes_read_as_the_spec_writes_them() {
    let line = |secs: u64| {
        let frames = secs * 44_100;
        size_line(frames, wwav_bytes(frames, 195, 115, None))
    };
    assert_eq!(line(60), "1:00 → about 53 MB."); // 52.9 MB a minute (6.1)
    assert_eq!(line(7), "0:07 → about 6 MB.");
    assert_eq!(line(30 * 60), "30:00 → about 1.6 GB.");
    assert_eq!(line(81 * 60), "81:00 → about 4.3 GB.");
}

#[test]
fn a_wwav_holds_about_81_minutes() {
    assert_eq!(minutes_held(), 81);
}

#[test]
fn a_94_minute_session_is_refused_with_the_spec_sentence() {
    let session = 94 * 60 * 48_000; // a 48 kHz session
    let frames = wwav_frames(session, 48_000);
    let bytes = wwav_bytes(frames, 195, 115, None);
    assert_eq!(
        refusal(frames, bytes).as_deref(),
        Some("A .wwav holds about 81 minutes. This session is 94.")
    );
}

#[test]
fn the_limit_is_the_packers_4_gb() {
    // `pack` refuses a file over 0xFFFFFFFF bytes. Find the longest song
    // that fits and check one frame more is refused.
    let fits = |f: u64| wwav_bytes(f, 195, 115, None) <= 0xFFFF_FFFF;
    let (mut lo, mut hi) = (0u64, 1 << 30);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if fits(mid) {
            lo = mid
        } else {
            hi = mid
        }
    }
    let longest = lo;
    assert!(longest / 44_100 / 60 == 81, "{longest} frames");
    assert_eq!(refusal(longest, wwav_bytes(longest, 195, 115, None)), None);
    assert_eq!(
        refusal(longest + 1, wwav_bytes(longest + 1, 195, 115, None)).as_deref(),
        Some("A .wwav holds about 81 minutes. This session is 82.")
    );
    // 81 minutes on the dot fits.
    let frames = 81 * 60 * 44_100;
    assert_eq!(refusal(frames, wwav_bytes(frames, 195, 115, None)), None);
}

#[test]
fn wwav_frames_follow_the_resampler() {
    assert_eq!(wwav_frames(48_000 * 60, 48_000), 44_100 * 60);
    assert_eq!(wwav_frames(44_100 * 60, 44_100), 44_100 * 60);
    assert_eq!(wwav_frames(161, 48_000), 148);
    assert_eq!(wwav_frames(96_000, 96_000), 44_100);
}

#[test]
fn the_conversions_are_said_out_loud() {
    assert_eq!(
        conversion_line(48_000),
        "This session is 48 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered."
    );
    assert_eq!(
        conversion_line(96_000),
        "This session is 96 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered."
    );
    assert_eq!(
        conversion_line(88_200),
        "This session is 88.2 kHz. The .wwav will be 44.1 kHz, 16-bit, resampled and dithered."
    );
    assert_eq!(
        conversion_line(44_100),
        "The .wwav will be 16-bit, dithered."
    );
    assert_eq!(
        SWAV_SOUND_LINE,
        "The film's sound is AAC at 320 kbps. The .wwav beside it holds the lossless stems."
    );
    assert_eq!(
        posting_line(48_000, 24).as_deref(),
        Some("This is 48 kHz, 24-bit. Posting needs 44.1 kHz, 16-bit.")
    );
    assert_eq!(
        posting_line(44_100, 24).as_deref(),
        Some("This is 44.1 kHz, 24-bit. Posting needs 44.1 kHz, 16-bit.")
    );
    assert_eq!(posting_line(44_100, 16), None);
}
