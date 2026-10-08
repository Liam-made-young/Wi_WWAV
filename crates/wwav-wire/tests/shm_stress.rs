//! F8: "a reader ever sees a torn clock under a writer at full rate".
//!
//! One writer publishes the clock as fast as it can, with every field a
//! function of one counter `k`; readers check that every clock they read
//! comes from a single `k`. A torn read is one whose fields disagree. The
//! meter ring gets the same treatment.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use wwav_wire::shm::{ClockFields, Shm};

fn fields(k: u64) -> ClockFields {
    ClockFields {
        sample_pos: k as i64 * 128 - 312,
        host_time_ns: 1_000 + k.wrapping_mul(2_666_667),
        rate: k as f64 * 0.5,
        state: (k % 3) as u32,
        dropouts: (k as u32).wrapping_mul(7),
        callbacks: k,
    }
}

const READERS: usize = 3;
const READS_EACH: usize = 2_000_000;

#[test]
fn no_reader_sees_a_torn_clock_under_a_writer_at_full_rate() {
    let shm = Arc::new(Shm::create_for_app().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let start = Arc::new(Barrier::new(READERS + 1));

    let writer = {
        let (shm, stop, start) = (shm.clone(), stop.clone(), start.clone());
        thread::spawn(move || {
            let clock = &shm.region().clock;
            let mut k = 0u64;
            start.wait();
            while !stop.load(Ordering::Relaxed) {
                k += 1;
                clock.write(&fields(k));
            }
            k
        })
    };

    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let (shm, start) = (shm.clone(), start.clone());
            thread::spawn(move || {
                let clock = &shm.region().clock;
                let (mut torn, mut missed, mut changes, mut last) = (0u64, 0u64, 0u64, 0u64);
                start.wait();
                for _ in 0..READS_EACH {
                    match clock.read() {
                        Some(f) if f.callbacks == 0 => {}
                        Some(f) => {
                            if f != fields(f.callbacks) {
                                torn += 1;
                            }
                            if f.callbacks != last {
                                changes += 1;
                                last = f.callbacks;
                            }
                        }
                        None => missed += 1,
                    }
                }
                (torn, missed, changes)
            })
        })
        .collect();

    let results: Vec<(u64, u64, u64)> = readers.into_iter().map(|r| r.join().unwrap()).collect();
    stop.store(true, Ordering::Relaxed);
    let written = writer.join().unwrap();

    let torn: u64 = results.iter().map(|r| r.0).sum();
    let missed: u64 = results.iter().map(|r| r.1).sum();
    let changes: u64 = results.iter().map(|r| r.2).sum();
    eprintln!(
        "{} reads, {torn} torn, {missed} gave up, {changes} saw a new block, {written} writes",
        READERS * READS_EACH
    );
    assert_eq!(torn, 0, "torn clocks were read");
    // A reader gives up (None) only when seq stays on one odd number for
    // 20 ms, which here means the writer thread was descheduled mid-write on
    // a busy machine. That is a missed read, never a torn one, so it isn't
    // counted against the clock.
    // The test only means something if the writer kept writing while the
    // readers read: each reader must have seen the clock move many times.
    assert!(
        changes > (READERS * 1000) as u64,
        "the writer barely raced the readers"
    );
}

/// Every slot of entry k holds levels made from k, so a copy that mixes two
/// entries shows.
fn levels(k: u64, slot: usize) -> [f32; 4] {
    let v = (k % 1_000_000) as f32 + slot as f32 / 1000.0;
    [v, v + 0.25, v + 0.5, v + 0.75]
}

#[test]
fn no_reader_sees_a_torn_meter_entry_under_a_writer_at_full_rate() {
    let shm = Arc::new(Shm::create_for_app().unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let start = Arc::new(Barrier::new(2));
    let writer = {
        let (shm, stop, start) = (shm.clone(), stop.clone(), start.clone());
        thread::spawn(move || {
            let mut k = 0u64;
            let mut slots = vec![[0.0f32; 4]; 40];
            start.wait();
            while !stop.load(Ordering::Relaxed) {
                k += 1;
                for (s, v) in slots.iter_mut().enumerate() {
                    *v = levels(k, s);
                }
                shm.region()
                    .write_meters(k, (k % 100) as f32 / 100.0, k as u32, &slots);
            }
        })
    };
    let (mut torn, mut read, mut changes, mut last) = (0, 0, 0, 0);
    start.wait();
    // Count only reads that found an entry, so a writer slow to start on a
    // busy machine can't leave the test with nothing to check.
    while read < 200_000 {
        let Some(m) = shm.region().newest_meters() else {
            continue;
        };
        read += 1;
        let k = m.callback;
        let whole = m.dropouts == k as u32
            && m.dsp_load == (k % 100) as f32 / 100.0
            && m.slots.len() == 40
            && m.slots.iter().enumerate().all(|(s, v)| *v == levels(k, s));
        if !whole {
            torn += 1;
        }
        if k != last {
            changes += 1;
            last = k;
        }
    }
    stop.store(true, Ordering::Relaxed);
    writer.join().unwrap();
    eprintln!("{read} meter reads, {torn} torn, {changes} saw a new entry");
    assert_eq!(torn, 0, "torn meter entries were read");
    assert!(changes > 1000, "the writer barely raced the reader");
}

#[test]
fn a_takes_frames_and_peaks_read_back_until_the_ring_comes_round() {
    use wwav_wire::shm::{TakeFields, INPUT_FRAMES, PEAK_ENTRIES};
    let shm = Shm::create_for_app().unwrap();
    let (peaks, input) = (shm.peaks().unwrap(), shm.input().unwrap());
    let take = TakeFields {
        at: 44_100,
        sample_rate: 48_000,
        channels: 2,
        frames_per_peak: 256,
    };
    peaks.begin(&take);
    input.begin(&take);
    assert_eq!(peaks.read(0), Some(vec![]));
    let frames: Vec<f32> = (0..2000).map(|i| i as f32).collect();
    input.push(&frames);
    assert_eq!(input.read(0).unwrap(), frames);
    assert_eq!(input.read(990).unwrap(), frames[1980..]);
    assert_eq!(input.read(1001), None, "frames that aren't written yet");
    peaks.push([-0.5, 0.25, -1.0, 1.0]);
    assert_eq!(peaks.read(0).unwrap(), vec![[-0.5, 0.25, -1.0, 1.0]]);
    assert_eq!(
        (
            peaks.at.load(std::sync::atomic::Ordering::Relaxed),
            peaks.take.load(std::sync::atomic::Ordering::Relaxed)
        ),
        (44_100, 1)
    );
    // Round the ring: the oldest frames and peaks are gone, and a reader is told.
    for _ in 0..INPUT_FRAMES / 1000 {
        input.push(&frames);
    }
    assert_eq!(input.read(0), None);
    let newest = input
        .read(input.write.load(std::sync::atomic::Ordering::Relaxed) - 1000)
        .unwrap();
    assert_eq!(newest, frames);
    for _ in 0..PEAK_ENTRIES {
        peaks.push([0.0; 4]);
    }
    assert_eq!(peaks.read(0), None);
    assert_eq!(peaks.read(1).unwrap().len(), PEAK_ENTRIES);
    // A new take starts both over.
    input.begin(&take);
    assert_eq!(input.read(0), Some(vec![]));
}
