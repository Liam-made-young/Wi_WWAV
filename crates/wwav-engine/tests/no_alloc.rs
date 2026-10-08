//! "The audio thread never allocates" (`docs/SPEC.md` 9.4), held as a
//! number: this test is the audio thread, with an allocator that counts
//! what that thread asks for while it runs blocks.
//!
//! The blocks do everything a block can: a clip held whole and one streamed
//! from disk, a loop's wrap, parameter changes and transport commands
//! arriving, a graph swapped in under it, a take being recorded.

use serde_json::json;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use wwav_engine::audio::{Audio, Kind, Output};
use wwav_engine::build::{self, Sources};
use wwav_engine::graph::Param;
use wwav_engine::media::{Failure, HeldCache, Reader};
use wwav_wire::shm::Shm;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static WATCHED: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: every call goes straight to the system allocator.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHED.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCHED.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if WATCHED.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, size)
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const RATE: u32 = 44_100;
const BLOCK: usize = 256;

/// A 16-bit stereo WAV of a 441 Hz sine, `seconds` long.
fn wav(path: &std::path::Path, seconds: usize) {
    let frames = seconds * RATE as usize;
    let data = (frames * 4) as u32;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    let cycle: Vec<i16> = (0..100)
        .map(|n| (8000.0 * (2.0 * std::f64::consts::PI * n as f64 / 100.0).sin()) as i16)
        .collect();
    for n in 0..frames {
        let s = cycle[n % 100].to_le_bytes();
        bytes.extend_from_slice(&[s[0], s[1], s[0], s[1]]);
    }
    std::fs::write(path, bytes).unwrap();
}

fn graph(gen: u64, held: &mut HeldCache, small: &str, big: &str) -> Box<wwav_engine::graph::Graph> {
    let clip = |path: &str, len: u32| json!([{"id": "C", "path": path, "source": "master", "at": 0, "in": 0, "len": len}]);
    let args = json!({"graph": {"sample_rate": RATE, "tracks": [
        {"id": "held", "kind": "audio", "role": "vocals", "clips": clip(small, 2 * RATE)},
        {"id": "streamed", "kind": "audio", "role": "bass", "pan": -0.5, "clips": clip(big, 200 * RATE)},
    ]}});
    let mut sources = Sources {
        held,
        convert: &mut |_, _, _, _| Err(Failure::new("unsupported", "nothing to convert here.")),
    };
    Box::new(
        build::build(args.as_object().unwrap(), RATE, gen, &mut sources).expect("the graph builds"),
    )
}

#[test]
fn the_audio_thread_allocates_nothing() {
    // The instrument first: an allocation on a watched thread is counted.
    WATCHED.with(|w| w.set(true));
    drop(std::hint::black_box(Box::new(7u64)));
    WATCHED.with(|w| w.set(false));
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 2, "the counter counts");

    let dir = tempfile::tempdir().unwrap();
    let (small, big) = (dir.path().join("small.wav"), dir.path().join("big.wav"));
    wav(&small, 2);
    wav(&big, 200); // 35 MB: over what is held whole, so it streams
    let (small, big) = (small.to_str().unwrap(), big.to_str().unwrap());

    let (mut audio, mut params) = Audio::new(Shm::create_for_app().unwrap(), RATE, BLOCK as u32);
    let token = audio.open_manual(RATE, BLOCK as u32);
    let shared = audio.shared().clone();
    let reader = Reader::new();
    let mut held = HeldCache::default();
    let first = graph(1, &mut held, small, big);
    assert_eq!(first.streams().iter().count(), 1, "the big file streams");
    first.streams().prefill(0);
    for s in first.streams().iter() {
        reader.add(s.clone());
    }
    let second = graph(2, &mut held, small, big);
    second.streams().prefill(0);
    for s in second.streams().iter() {
        reader.add(s.clone());
    }
    audio.swap(Some(first));

    // Everything that will arrive while the blocks run is queued or made
    // now, on this thread, before it starts counting.
    audio.post(Kind::Loop {
        on: true,
        start: 1000,
        end: 1000 + 10 * BLOCK as i64 + 7,
    });
    audio.post(Kind::Play);
    audio.post(Kind::Record(true));
    shared.capture().begin(2, true);
    let change = |node, param, value, gen| {
        Kind::SetParam {
            node,
            param,
            value,
            gen,
        }
        .into()
    };
    let mut out = vec![0.0f32; 2 * BLOCK];
    let mut block = |n: usize| {
        for _ in 0..n {
            shared.callback(
                token,
                BLOCK,
                Output::Interleaved {
                    data: &mut out,
                    channels: 2,
                },
                None,
            );
        }
    };

    ALLOCATIONS.store(0, Ordering::Relaxed);
    WATCHED.with(|w| w.set(true));
    block(40); // plays, wraps the loop three times, feeds the loopback
    WATCHED.with(|w| w.set(false));
    params.push(change(0, Param::Mute, 1.0, 1)).unwrap();
    params.push(change(4, Param::GainDb, -6.0, 1)).unwrap();
    audio.post(Kind::Locate(50 * RATE as i64)); // out of the streamed window: silence, not a wait
    WATCHED.with(|w| w.set(true));
    block(40);
    WATCHED.with(|w| w.set(false));

    // A swap happens on the worker; the audio thread only finds a new pointer.
    let swapper = std::thread::spawn(move || {
        let old = audio.swap(Some(second));
        audio.post(Kind::Stop);
        (audio, old)
    });
    WATCHED.with(|w| w.set(true));
    block(200);
    WATCHED.with(|w| w.set(false));
    let (audio, old) = swapper.join().unwrap();
    assert!(
        old.is_some(),
        "the first graph came back to be freed off the audio thread"
    );
    WATCHED.with(|w| w.set(true));
    block(10);
    WATCHED.with(|w| w.set(false));

    assert_eq!(
        ALLOCATIONS.load(Ordering::Relaxed),
        0,
        "the audio thread allocated or freed memory"
    );
    // And the blocks really ran: the clock counted them, and sound came out.
    let clock = shared.shm().region().clock.read().unwrap();
    assert_eq!(clock.callbacks, 290);
    assert!(shared.capture().frames() > 0, "the loopback was fed");
    drop(old);
    drop(audio);
}
