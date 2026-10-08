//! Uncompressed files, written here by hand, decode to exactly their samples
//! over full scale, and files that are missing, not sound, or too wide are
//! refused in a sentence.

mod common;

use std::fs;

use common::*;
use wwav_decode::{probe, Decoder, Error, Info};

fn info(rate: u32, channels: u16, frames: u64) -> Info {
    Info {
        rate,
        channels,
        frames: Some(frames),
        codec: "pcm".into(),
    }
}

#[test]
fn a_16_bit_wav_decodes_to_its_samples_over_32768() {
    let dir = tempfile::tempdir().unwrap();
    for (channels, rate) in [(1u16, 44_100u32), (2, 48_000)] {
        let samples = random16(20_000, 7);
        let path = dir.path().join(format!("{channels}.wav"));
        fs::write(&path, wav16(channels, rate, &samples)).unwrap();
        let (said, got) = decode(&path).unwrap();
        assert_eq!(said, info(rate, channels, 20_000 / channels as u64));
        assert_same(&got, &over_32768(&samples));
        assert_eq!(got[0], -1.0);
        assert_eq!(got[1], 32767.0 / 32768.0);
    }
}

#[test]
fn a_24_bit_wav_decodes_to_its_samples_over_8388608() {
    let dir = tempfile::tempdir().unwrap();
    let mut seed = 24;
    let mut values: Vec<i32> = (0..9_000)
        .map(|_| (next(&mut seed) as i32) << 8 >> 8)
        .collect();
    values[0] = -8_388_608;
    values[1] = 8_388_607;
    let data: Vec<u8> = values
        .iter()
        .flat_map(|v| v.to_le_bytes()[..3].to_vec())
        .collect();
    let want: Vec<f32> = values.iter().map(|&v| v as f32 / 8_388_608.0).collect();
    for (channels, rate) in [(1u16, 96_000u32), (2, 44_100)] {
        // Plain, and as the extensible header most 24-bit files carry.
        for tag in [1, 0xFFFE] {
            let path = dir.path().join("24.wav");
            fs::write(&path, wav(tag, channels, rate, 24, &data)).unwrap();
            let (said, got) = decode(&path).unwrap();
            assert_eq!(said, info(rate, channels, 9_000 / channels as u64));
            assert_same(&got, &want);
        }
    }
}

#[test]
fn a_32_bit_and_an_8_bit_wav_decode_to_their_samples_over_full_scale() {
    let dir = tempfile::tempdir().unwrap();
    let mut seed = 32;
    let mut wide: Vec<i32> = (0..8_000)
        .map(|_| ((next(&mut seed) << 16) ^ next(&mut seed)) as i32)
        .collect();
    wide[0] = i32::MIN;
    wide[1] = i32::MAX;
    let data: Vec<u8> = wide.iter().flat_map(|v| v.to_le_bytes()).collect();
    let path = dir.path().join("32.wav");
    fs::write(&path, wav(1, 2, 48_000, 32, &data)).unwrap();
    let (said, got) = decode(&path).unwrap();
    assert_eq!(said, info(48_000, 2, 4_000));
    let want: Vec<f32> = wide.iter().map(|&v| v as f32 / 2_147_483_648.0).collect();
    assert_same(&got, &want);

    // 8-bit WAV is unsigned, with silence at 128.
    let narrow: Vec<u8> = (0..=255).collect();
    let path = dir.path().join("8.wav");
    fs::write(&path, wav(1, 1, 22_050, 8, &narrow)).unwrap();
    let (said, got) = decode(&path).unwrap();
    assert_eq!(said, info(22_050, 1, 256));
    let want: Vec<f32> = narrow.iter().map(|&v| (v as f32 - 128.0) / 128.0).collect();
    assert_same(&got, &want);
}

#[test]
fn a_float_wav_decodes_to_the_floats_in_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut seed = 3;
    let mut samples: Vec<f32> = (0..10_000)
        .map(|_| next(&mut seed) as f32 / 8_388_608.0 - 1.0)
        .collect();
    // Over full scale, the smallest float, and a zero with its sign.
    samples[..6].copy_from_slice(&[1.5, -3.25, f32::MIN_POSITIVE, 1e-42, -0.0, 0.0]);
    for (channels, rate) in [(1u16, 48_000u32), (2, 44_100)] {
        let path = dir.path().join("float.wav");
        fs::write(&path, wav_float(channels, rate, &samples)).unwrap();
        let (said, got) = decode(&path).unwrap();
        assert_eq!(said, info(rate, channels, 10_000 / channels as u64));
        assert_same(&got, &samples);
    }

    // 64-bit floats come out as the nearest 32-bit ones.
    let doubles: Vec<f64> = samples.iter().map(|&s| s as f64 * 1.000_000_1).collect();
    let data: Vec<u8> = doubles.iter().flat_map(|s| s.to_le_bytes()).collect();
    let path = dir.path().join("double.wav");
    fs::write(&path, wav(3, 2, 48_000, 64, &data)).unwrap();
    let (said, got) = decode(&path).unwrap();
    assert_eq!(said, info(48_000, 2, 5_000));
    let want: Vec<f32> = doubles.iter().map(|&s| s as f32).collect();
    assert_same(&got, &want);
}

/// An AIFF's `COMM` chunk: channels, frames, bits, and the rate as an 80-bit
/// float.
fn aiff(channels: u16, rate: u16, samples: &[i16], after: &[u8]) -> Vec<u8> {
    let mut comm = Vec::new();
    comm.extend_from_slice(&channels.to_be_bytes());
    comm.extend_from_slice(&((samples.len() / channels as usize) as u32).to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    comm.extend_from_slice(&(16_383u16 + 15).to_be_bytes());
    comm.extend_from_slice(&((rate as u64) << 48).to_be_bytes());
    let mut sound = vec![0u8; 8];
    sound.extend(samples.iter().flat_map(|s| s.to_be_bytes()));
    let mut body = b"AIFF".to_vec();
    for (id, chunk) in [(b"COMM", &comm), (b"SSND", &sound)] {
        body.extend_from_slice(id);
        body.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        body.extend_from_slice(chunk);
    }
    body.extend_from_slice(after);
    let mut out = b"FORM".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}

/// Symphonia 0.5.5 counts an AIFF's sound chunk 8 bytes too long and reads
/// that far, into whatever chunk comes next.
#[test]
fn an_aiff_with_a_chunk_after_its_sound_doesnt_end_in_a_click() {
    let dir = tempfile::tempdir().unwrap();
    let samples = random16(6_000, 11);
    let note = b"ANNO\x00\x00\x00\x10made by the test";
    for (channels, after) in [(2u16, &note[..]), (1, &note[..]), (2, &[][..])] {
        let path = dir.path().join("clip.aiff");
        fs::write(&path, aiff(channels, 44_100, &samples, after)).unwrap();
        let (said, got) = decode(&path).unwrap();
        assert_eq!(said, info(44_100, channels, 6_000 / channels as u64));
        assert_same(&got, &over_32768(&samples));
    }

    // The same behind an ID3v2 tag: ten bytes of header and 64 of padding.
    let mut tagged = b"ID3\x04\x00\x00\x00\x00\x00\x40".to_vec();
    tagged.extend_from_slice(&[0; 64]);
    tagged.extend_from_slice(&aiff(2, 44_100, &samples, note));
    let path = dir.path().join("tagged.aiff");
    fs::write(&path, tagged).unwrap();
    let (said, got) = decode(&path).unwrap();
    assert_eq!(said, info(44_100, 2, 3_000));
    assert_same(&got, &over_32768(&samples));
}

#[test]
fn probe_says_what_decoding_will_give() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.wav");
    fs::write(&path, wav16(2, 48_000, &random16(4_800, 1))).unwrap();
    let said = probe(&path).unwrap();
    assert_eq!(said, info(48_000, 2, 2_400));
    assert_eq!(&said, Decoder::open(&path).unwrap().info());

    // No sound at all is still a file with a rate and a length.
    let path = dir.path().join("empty.wav");
    fs::write(&path, wav16(1, 44_100, &[])).unwrap();
    assert_eq!(probe(&path).unwrap(), info(44_100, 1, 0));
    assert!(decode(&path).unwrap().1.is_empty());
}

#[test]
fn a_missing_file_is_no_such_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gone.mp3");
    let want = format!("No file at {}.", path.display());
    for got in [probe(&path).err(), Decoder::open(&path).err()] {
        assert!(
            matches!(&got, Some(Error::NoSuchFile(s)) if *s == want),
            "{got:?}"
        );
        assert_eq!(got.unwrap().to_string(), want);
    }
}

#[test]
fn a_file_that_isnt_sound_is_refused_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let words = "These are words, not sound, whatever the name says.\n".repeat(400);
    let noise: Vec<u8> = {
        let mut seed = 99;
        (0..200_000).map(|_| next(&mut seed) as u8).collect()
    };
    for (name, bytes) in [
        ("words.mp3", words.as_bytes()),
        ("words.wav", words.as_bytes()),
        ("short.flac", &b"fLa"[..]),
        ("empty.m4a", &[][..]),
        ("noise.ogg", &noise[..]),
        ("noise.mp3", &noise[..]),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, bytes).unwrap();
        let got = decode(&path).map(|(info, samples)| (info, samples.len()));
        assert!(
            matches!(got, Err(Error::Unsupported(_) | Error::Bad(_))),
            "{name}: {got:?}"
        );
        let sentence = got.unwrap_err().to_string();
        assert!(
            sentence.starts_with(name) && sentence.ends_with('.'),
            "{sentence}"
        );
    }
    let got = probe(&dir.path().join("words.mp3"))
        .unwrap_err()
        .to_string();
    assert_eq!(got, "words.mp3 is not a format this reads.");

    let got = probe(dir.path()).unwrap_err();
    assert!(matches!(got, Error::Unsupported(_)), "{got:?}");
}

#[test]
fn a_6_channel_wav_is_refused_as_too_wide() {
    let dir = tempfile::tempdir().unwrap();
    let data = vec![0u8; 6 * 2 * 1000];
    for tag in [1, 0xFFFE] {
        let path = dir.path().join("surround.wav");
        fs::write(&path, wav(tag, 6, 48_000, 16, &data)).unwrap();
        for got in [probe(&path).err(), decode(&path).err()] {
            let want = "surround.wav has 6 channels; clips are mono or stereo.";
            assert!(
                matches!(&got, Some(Error::Unsupported(s)) if s == want),
                "{got:?}"
            );
        }
    }
}

#[test]
fn a_wav_cut_short_decodes_as_far_as_it_goes() {
    let dir = tempfile::tempdir().unwrap();
    let samples = random16(20_000, 5);
    let whole = wav16(2, 48_000, &samples);
    let path = dir.path().join("cut.wav");
    // Cut in the middle of a frame: the header still promises it all.
    fs::write(&path, &whole[..44 + 4 * 6_000 + 3]).unwrap();
    let (said, got) = decode(&path).unwrap();
    assert_eq!(said.frames, Some(10_000));
    assert_same(&got, &over_32768(&samples[..12_000]));
}

/// The engine decodes on a worker thread.
#[test]
fn a_decoder_can_be_sent_to_another_thread() {
    fn sendable<T: Send>() {}
    sendable::<Decoder>();
    sendable::<Error>();
}
