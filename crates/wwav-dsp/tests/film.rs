//! F9: frame n's first sample isn't ⌊n × rate ÷ fps⌋ (`docs/PLAN.md`; 9.5:
//! "Frame n sits at n ÷ fps and its sound starts at sample ⌊n × rate ÷ fps⌋").
//! Each check below derives the answer another way than the formula does,
//! over spans long enough for a float clock to drift.

use wwav_dsp::film::Fps;

const DAY: u64 = 24 * 3600;

#[test]
fn ntsc_at_48_khz_repeats_exactly_every_5_frames_for_a_day() {
    // 48,000 × 1001 ÷ 30,000 = 1601.6 samples a frame, so every 5 frames
    // are exactly 8008 samples, and the first five start at these.
    let fps = Fps::new(30_000, 1001);
    let first = [0, 1601, 3203, 4804, 6406];
    let frames = DAY * 30;
    for n in 0..frames {
        let want = (n / 5) * 8008 + first[(n % 5) as usize];
        assert_eq!(fps.start_sample(n, 48_000), want, "frame {n}");
    }
}

#[test]
fn film_rate_23_976_at_44_1_khz_repeats_every_80_frames() {
    // 44,100 × 1001 ÷ 24,000 = 147,147 ÷ 80 samples a frame.
    let fps = Fps::new(24_000, 1001);
    let first: Vec<u64> = (0..80).map(|r| r * 147_147 / 80).collect();
    for n in (0..DAY * 24).step_by(7) {
        let want = (n / 80) * 147_147 + first[(n % 80) as usize];
        assert_eq!(fps.start_sample(n, 44_100), want, "frame {n}");
    }
}

#[test]
fn whole_rates_step_by_whole_or_alternating_samples() {
    // 30 fps at 48 kHz is 1600 samples a frame; 24 fps at 44.1 kHz is 1837.5,
    // so frames alternate 1837 and 1838.
    let thirty = Fps::whole(30);
    let twenty_four = Fps::whole(24);
    for n in 0..DAY * 30 {
        assert_eq!(thirty.start_sample(n, 48_000), n * 1600);
    }
    for n in 0..DAY * 24 {
        assert_eq!(twenty_four.start_sample(n, 44_100), n * 1837 + n / 2);
    }
}

#[test]
fn a_frame_sits_at_n_over_fps() {
    // As MP4 time: frame n is n × den ticks of a clock at num ticks a second.
    let ntsc = Fps::new(30_000, 1001);
    assert_eq!(ntsc.timescale(), 30_000);
    assert_eq!(ntsc.pts(0), 0);
    assert_eq!(ntsc.pts(30_000), 30_030_000); // 1001 s
    let pal = Fps::whole(25);
    assert_eq!((pal.timescale(), pal.pts(50)), (25, 50));
    // Rates are kept in lowest terms.
    assert_eq!(Fps::new(60_000, 2_002), Fps::new(30_000, 1001));
}

#[test]
fn a_film_has_every_frame_its_sound_reaches() {
    for (fps, rate) in [
        (Fps::new(30_000, 1001), 48_000),
        (Fps::whole(30), 48_000),
        (Fps::new(24_000, 1001), 44_100),
        (Fps::whole(24), 44_100),
        (Fps::new(60_000, 1001), 48_000),
    ] {
        for samples in [1, 1600, 1601, 1602, 48_000, 10 * 3600 * 48_000 + 17] {
            let count = fps.frame_count(samples, rate);
            // The last frame starts inside the sound and the next wouldn't.
            assert!(fps.start_sample(count - 1, rate) < samples);
            assert!(fps.start_sample(count, rate) >= samples);
        }
        assert_eq!(fps.frame_count(0, rate), 0);
    }
    // "Encoding · frame 2,410 of 5,712": 3:10.4 at 30 fps.
    assert_eq!(
        Fps::whole(30).frame_count(190 * 48_000 + 19_200, 48_000),
        5_712
    );
}
