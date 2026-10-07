//! The shared-memory region (`docs/ENGINE.md` §4), mirrored from
//! `engine/include/wwav_shm.h`. A test compiles the header as C and as C++
//! and compares every size and offset with the structs here.
//!
//! The app creates the region and zeroes it; the engine maps it, writes the
//! header, then writes the clock, crumb and meters from its audio thread
//! without locks. Both processes touch every field through atomics, so the
//! mirror's fields are atomics of the C types' sizes (an `f64` or `f32` lives
//! in an `AtomicU64` or `AtomicU32` as its bits).

use std::sync::atomic::{fence, AtomicI64, AtomicU32, AtomicU64, AtomicU8, Ordering};

pub const MAGIC: [u8; 4] = *b"WWAV";
pub const LAYOUT: u32 = 1;

/// Tracks, then the four stem buses, then the master.
pub const METER_SLOTS: usize = 260;
/// Entries in the meter ring.
pub const METER_RING: usize = 64;
pub const METER_ENTRY_HEADER: usize = 32;
pub const METER_SLOT_BYTES: usize = 16;
pub const METER_ENTRY_BYTES: usize = METER_ENTRY_HEADER + METER_SLOTS * METER_SLOT_BYTES;

pub const STATE_STOPPED: u32 = 0;
pub const STATE_PLAYING: u32 = 1;
pub const STATE_RECORDING: u32 = 2;

pub const RING_OFFSET: usize = 256;
pub const PEAKS_OFFSET: usize = RING_OFFSET + METER_RING * METER_ENTRY_BYTES;
pub const FIXED_BYTES: usize = PEAKS_OFFSET;
/// The peaks ring, 64 KiB.
pub const PEAKS_BYTES: usize = 64 * 1024;
/// The input ring: 2 s of interleaved f32 stereo at up to 96 kHz.
pub const INPUT_BYTES: usize = 2 * 96_000 * 2 * 4;
pub const TOTAL_BYTES: usize = FIXED_BYTES + PEAKS_BYTES + INPUT_BYTES;

/// `wwav_shm_header`, at offset 0.
#[repr(C)]
pub struct Header {
    pub magic: [AtomicU8; 4],
    pub layout: AtomicU32,
    pub sample_rate: AtomicU32,
    pub block_size: AtomicU32,
    pub meter_slots: AtomicU32,
    pub meter_ring: AtomicU32,
    pub peaks_bytes: AtomicU32,
    pub input_bytes: AtomicU32,
    pub engine_pid: AtomicU64,
    pub engine_start_ns: AtomicU64,
    pub reserved: [u8; 16],
}

/// `wwav_shm_clock`, at offset 64: a seqlock.
#[repr(C)]
pub struct Clock {
    pub seq: AtomicU64,
    pub sample_pos: AtomicI64,
    pub host_time_ns: AtomicU64,
    /// An `f64`'s bits.
    pub rate: AtomicU64,
    pub state: AtomicU32,
    pub dropouts: AtomicU32,
    pub callbacks: AtomicU64,
    pub reserved: [u8; 16],
}

/// `wwav_shm_crumb`, at offset 128.
#[repr(C)]
pub struct Crumb {
    pub crumb: AtomicU64,
    pub crumb_seq: AtomicU64,
    pub reserved: [u8; 48],
}

/// `wwav_shm_meter_index`, at offset 192.
#[repr(C)]
pub struct MeterIndex {
    pub meter_write: AtomicU64,
    pub reserved: [u8; 56],
}

/// `wwav_shm_meter_slot`: four `f32`s' bits, linear.
#[repr(C)]
pub struct MeterSlot {
    pub peak_l: AtomicU32,
    pub peak_r: AtomicU32,
    pub rms_l: AtomicU32,
    pub rms_r: AtomicU32,
}

/// `wwav_shm_meter_entry`.
#[repr(C)]
pub struct MeterEntry {
    pub callback: AtomicU64,
    /// An `f32`'s bits.
    pub dsp_load: AtomicU32,
    pub dropouts: AtomicU32,
    pub slots_used: AtomicU32,
    pub reserved: [u8; 12],
    pub slots: [MeterSlot; METER_SLOTS],
}

/// `wwav_shm`: the fixed part of the region. The peaks and input rings
/// follow it, at the sizes in the header.
#[repr(C)]
pub struct Region {
    pub header: Header,
    pub clock: Clock,
    pub crumb: Crumb,
    pub meters: MeterIndex,
    pub ring: [MeterEntry; METER_RING],
}

/// The header fields the engine chooses; the rest are the layout's constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeaderFields {
    pub sample_rate: u32,
    pub block_size: u32,
    pub engine_pid: u64,
    pub engine_start_ns: u64,
}

impl Header {
    /// The engine's side, once after it maps the region. The magic goes in
    /// last, after a release fence, so a reader that sees it sees the rest.
    pub fn write(&self, f: &HeaderFields) {
        let r = Ordering::Relaxed;
        self.layout.store(LAYOUT, r);
        self.sample_rate.store(f.sample_rate, r);
        self.block_size.store(f.block_size, r);
        self.meter_slots.store(METER_SLOTS as u32, r);
        self.meter_ring.store(METER_RING as u32, r);
        self.peaks_bytes.store(PEAKS_BYTES as u32, r);
        self.input_bytes.store(INPUT_BYTES as u32, r);
        self.engine_pid.store(f.engine_pid, r);
        self.engine_start_ns.store(f.engine_start_ns, r);
        fence(Ordering::Release);
        for (m, b) in self.magic.iter().zip(MAGIC) {
            m.store(b, r);
        }
    }

    /// After `device.open` changes the rate or block.
    pub fn set_format(&self, sample_rate: u32, block_size: u32) {
        self.sample_rate.store(sample_rate, Ordering::Relaxed);
        self.block_size.store(block_size, Ordering::Relaxed);
    }

    /// The header, if an engine of this layout has written it.
    pub fn read(&self) -> Result<HeaderFields, String> {
        let r = Ordering::Relaxed;
        let magic: Vec<u8> = self.magic.iter().map(|m| m.load(r)).collect();
        fence(Ordering::Acquire);
        if magic != MAGIC {
            return Err("No engine has written the shared memory's header.".into());
        }
        let layout = self.layout.load(r);
        let (slots, ring) = (self.meter_slots.load(r), self.meter_ring.load(r));
        if layout != LAYOUT || slots as usize != METER_SLOTS || ring as usize != METER_RING {
            return Err(format!(
                "The engine wrote shared-memory layout {layout} ({slots} meter slots, a ring of {ring}); \
                 the app reads layout {LAYOUT}."
            ));
        }
        Ok(HeaderFields {
            sample_rate: self.sample_rate.load(r),
            block_size: self.block_size.load(r),
            engine_pid: self.engine_pid.load(r),
            engine_start_ns: self.engine_start_ns.load(r),
        })
    }
}

/// The clock's fields, as the audio thread publishes them every block.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClockFields {
    /// Where the playhead will be when this block reaches the speaker, with
    /// output latency and plugin delay already subtracted.
    pub sample_pos: i64,
    /// The monotonic time (`monotonic_ns`) at which `sample_pos` is at the speaker.
    pub host_time_ns: u64,
    /// Samples per second of playhead travel; 0 when stopped.
    pub rate: f64,
    pub state: u32,
    pub dropouts: u32,
    /// Blocks processed since the engine started.
    pub callbacks: u64,
}

impl ClockFields {
    /// Where the cursor is at `now_ns`: `sample_pos + (now - host_time_ns) × rate / 1e9`.
    pub fn playhead_at(&self, now_ns: u64) -> f64 {
        let dt = now_ns as i128 - self.host_time_ns as i128;
        self.sample_pos as f64 + dt as f64 * self.rate / 1e9
    }
}

/// How long a reader waits on one odd `seq` before it decides the writer
/// died halfway through a write. A live writer holds it odd for nanoseconds;
/// this allows for one descheduled at the wrong moment.
const CLOCK_STUCK: std::time::Duration = std::time::Duration::from_millis(20);

impl Clock {
    /// The writer's side, from one thread only (the audio thread).
    ///
    /// The ordering is the seqlock's (§4.2), spelled out: `seq` goes odd, a
    /// release fence keeps the field stores from being seen before it, the
    /// fields are stored, and `seq` goes even with a release store, which
    /// keeps them from being seen after it.
    pub fn write(&self, f: &ClockFields) {
        let r = Ordering::Relaxed;
        let mut seq = self.seq.load(r);
        // An engine killed between the two halves of a write leaves seq odd;
        // start from the next even number so no reader takes it as done.
        seq += seq & 1;
        self.seq.store(seq + 1, r);
        fence(Ordering::Release);
        self.sample_pos.store(f.sample_pos, r);
        self.host_time_ns.store(f.host_time_ns, r);
        self.rate.store(f.rate.to_bits(), r);
        self.state.store(f.state, r);
        self.dropouts.store(f.dropouts, r);
        self.callbacks.store(f.callbacks, r);
        self.seq.store(seq + 2, Ordering::Release);
    }

    /// The reader's side, from any thread or process: load `seq` (acquire),
    /// read the fields, an acquire fence, load `seq` again, and retry if it
    /// changed or is odd. `None` means `seq` stayed on one odd number: the
    /// writer died mid-write.
    pub fn read(&self) -> Option<ClockFields> {
        let r = Ordering::Relaxed;
        let mut stuck: Option<(u64, std::time::Instant)> = None;
        loop {
            let before = self.seq.load(Ordering::Acquire);
            if before & 1 == 0 {
                let f = ClockFields {
                    sample_pos: self.sample_pos.load(r),
                    host_time_ns: self.host_time_ns.load(r),
                    rate: f64::from_bits(self.rate.load(r)),
                    state: self.state.load(r),
                    dropouts: self.dropouts.load(r),
                    callbacks: self.callbacks.load(r),
                };
                fence(Ordering::Acquire);
                if self.seq.load(r) == before {
                    return Some(f);
                }
            } else {
                match stuck {
                    Some((seq, since)) if seq == before => {
                        if since.elapsed() > CLOCK_STUCK {
                            return None;
                        }
                    }
                    _ => stuck = Some((before, std::time::Instant::now())),
                }
            }
            std::hint::spin_loop();
        }
    }
}

/// The crumb of a graph node: FNV-1a 64 of its id (`wwav_ids::fnv1a64`).
pub fn crumb_hash(node_id: &str) -> u64 {
    wwav_ids::fnv1a64(node_id.as_bytes())
}

/// Which of `ids` a crumb names, if any: after a crash, the app hashes each
/// device id in its session to find the one that was running.
pub fn crumb_node<'a>(crumb: u64, ids: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    if crumb == 0 {
        return None;
    }
    ids.into_iter().find(|id| crumb_hash(id) == crumb)
}

impl Crumb {
    /// The audio thread, before every call into a device node.
    pub fn set(&self, hash: u64) {
        self.crumb.store(hash, Ordering::Relaxed);
        self.crumb_seq.fetch_add(1, Ordering::Release);
    }

    /// The audio thread, after the call returns.
    pub fn clear(&self) {
        self.set(0);
    }

    pub fn get(&self) -> u64 {
        self.crumb.load(Ordering::Acquire)
    }

    pub fn seq(&self) -> u64 {
        self.crumb_seq.load(Ordering::Acquire)
    }
}

/// One meter entry as a reader copied it.
#[derive(Debug, Clone, PartialEq)]
pub struct MeterFrame {
    pub callback: u64,
    pub dsp_load: f32,
    pub dropouts: u32,
    /// `slots_used` slots, each peak L, peak R, RMS L, RMS R (linear).
    pub slots: Vec<[f32; 4]>,
}

impl Region {
    /// The writer's side, from the audio thread once per block. Entry `i`
    /// goes in ring slot `i mod MR`, then `meter_write` becomes `i + 1`.
    pub fn write_meters(&self, callback: u64, dsp_load: f32, dropouts: u32, slots: &[[f32; 4]]) {
        let r = Ordering::Relaxed;
        let i = self.meters.meter_write.load(r);
        let e = &self.ring[(i % METER_RING as u64) as usize];
        // As with the clock: a reader that sees any of this entry's new
        // values must also see meter_write at i, so it knows it was lapped.
        fence(Ordering::Release);
        let used = slots.len().min(METER_SLOTS);
        e.callback.store(callback, r);
        e.dsp_load.store(dsp_load.to_bits(), r);
        e.dropouts.store(dropouts, r);
        e.slots_used.store(used as u32, r);
        for (dst, src) in e.slots.iter().zip(&slots[..used]) {
            dst.peak_l.store(src[0].to_bits(), r);
            dst.peak_r.store(src[1].to_bits(), r);
            dst.rms_l.store(src[2].to_bits(), r);
            dst.rms_r.store(src[3].to_bits(), r);
        }
        self.meters.meter_write.store(i + 1, Ordering::Release);
    }

    /// The newest complete entry, or `None` before the first. The copy is
    /// checked afterwards: if the writer came round the ring to this entry
    /// while it was being copied, the copy is thrown away and taken again.
    pub fn newest_meters(&self) -> Option<MeterFrame> {
        let r = Ordering::Relaxed;
        for _ in 0..8 {
            let n = self.meters.meter_write.load(Ordering::Acquire);
            if n == 0 {
                return None;
            }
            let e = &self.ring[((n - 1) % METER_RING as u64) as usize];
            let used = (e.slots_used.load(r) as usize).min(METER_SLOTS);
            let frame = MeterFrame {
                callback: e.callback.load(r),
                dsp_load: f32::from_bits(e.dsp_load.load(r)),
                dropouts: e.dropouts.load(r),
                slots: e.slots[..used]
                    .iter()
                    .map(|s| {
                        [
                            s.peak_l.load(r),
                            s.peak_r.load(r),
                            s.rms_l.load(r),
                            s.rms_r.load(r),
                        ]
                        .map(f32::from_bits)
                    })
                    .collect(),
            };
            fence(Ordering::Acquire);
            // Entry n-1 is overwritten by entry n-1+MR, which the writer
            // starts once meter_write reaches n-1+MR.
            if self.meters.meter_write.load(r) < n - 1 + METER_RING as u64 {
                return Some(frame);
            }
        }
        None
    }
}

/// The monotonic clock both processes stamp the clock with: `CLOCK_MONOTONIC`
/// on Linux, and on macOS `CLOCK_UPTIME_RAW`, which is `mach_absolute_time`,
/// the host time CoreAudio stamps its buffers with.
#[cfg(unix)]
pub fn monotonic_ns() -> u64 {
    #[cfg(target_vendor = "apple")]
    let id = libc::CLOCK_UPTIME_RAW;
    #[cfg(not(target_vendor = "apple"))]
    let id = libc::CLOCK_MONOTONIC;
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: ts is a valid timespec to write to; the clock id exists here.
    unsafe { libc::clock_gettime(id, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

#[cfg(unix)]
pub use mapping::Shm;

// Windows (Stage 6): TODO a named file mapping (CreateFileMappingW and
// MapViewOfFile) behind the same `Shm`, owner-only through its security
// descriptor, and QueryPerformanceCounter behind `monotonic_ns`.
#[cfg(unix)]
mod mapping {
    use super::{Region, FIXED_BYTES, TOTAL_BYTES};
    use std::ffi::CString;
    use std::io;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A mapping of the region. The app's (`create`) unlinks the name when
    /// it is dropped; the engine's (`open`) only unmaps.
    pub struct Shm {
        ptr: NonNull<u8>,
        len: usize,
        name: String,
        owner: bool,
    }

    // SAFETY: the mapping is plain shared memory, and every access to it
    // goes through the atomics in `Region`.
    unsafe impl Send for Shm {}
    unsafe impl Sync for Shm {}

    static NEXT: AtomicU32 = AtomicU32::new(0);

    impl Shm {
        /// A new zeroed region named `/wwav-<app pid>-<n>` (§1), mode 0600.
        pub fn create_for_app() -> io::Result<Shm> {
            let name = format!(
                "/wwav-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            match Shm::create(&name) {
                // Left by an earlier process that had this pid and died:
                // nothing alive can be using a name with our pid in it.
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let c = CString::new(name.as_str()).map_err(io::Error::other)?;
                    // SAFETY: c is a valid C string.
                    unsafe { libc::shm_unlink(c.as_ptr()) };
                    Shm::create(&name)
                }
                other => other,
            }
        }

        /// Creates `name` (it must not exist), owner-only, zeroed and
        /// `TOTAL_BYTES` long, and maps it.
        pub fn create(name: &str) -> io::Result<Shm> {
            let c = CString::new(name).map_err(io::Error::other)?;
            // SAFETY: c is a valid C string; the fd is closed below on every path.
            let fd = unsafe {
                libc::shm_open(
                    c.as_ptr(),
                    libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: fd is open; ftruncate zero-fills the new region.
            let sized = unsafe { libc::ftruncate(fd, TOTAL_BYTES as libc::off_t) } == 0;
            let mapped = if sized {
                map(fd, TOTAL_BYTES)
            } else {
                Err(io::Error::last_os_error())
            };
            // SAFETY: fd is open and no longer needed once mapped.
            unsafe { libc::close(fd) };
            match mapped {
                Ok(ptr) => Ok(Shm {
                    ptr,
                    len: TOTAL_BYTES,
                    name: name.into(),
                    owner: true,
                }),
                Err(e) => {
                    // SAFETY: c is a valid C string.
                    unsafe { libc::shm_unlink(c.as_ptr()) };
                    Err(e)
                }
            }
        }

        /// Maps a region the app created: the engine's side.
        pub fn open(name: &str) -> io::Result<Shm> {
            let c = CString::new(name).map_err(io::Error::other)?;
            // SAFETY: c is a valid C string; the fd is closed below.
            let fd = unsafe { libc::shm_open(c.as_ptr(), libc::O_RDWR, 0) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: an all-zero stat is a valid out-parameter.
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            // SAFETY: fd is open and st is writable.
            let len = if unsafe { libc::fstat(fd, &mut st) } == 0 {
                st.st_size as usize
            } else {
                0
            };
            let mapped = if len < FIXED_BYTES {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{name} holds {len} bytes; layout {} needs {TOTAL_BYTES}",
                        super::LAYOUT
                    ),
                ))
            } else {
                map(fd, len)
            };
            // SAFETY: fd is open and no longer needed.
            unsafe { libc::close(fd) };
            Ok(Shm {
                ptr: mapped?,
                len,
                name: name.into(),
                owner: false,
            })
        }

        pub fn name(&self) -> &str {
            &self.name
        }

        pub fn len(&self) -> usize {
            self.len
        }

        pub fn is_empty(&self) -> bool {
            self.len == 0
        }

        pub fn region(&self) -> &Region {
            // SAFETY: the mapping is page-aligned, at least FIXED_BYTES long
            // (the size of Region), and lives as long as self. All-zero is a
            // valid Region, and other writers only ever use atomics.
            unsafe { &*(self.ptr.as_ptr() as *const Region) }
        }
    }

    fn map(fd: libc::c_int, len: usize) -> io::Result<NonNull<u8>> {
        // SAFETY: fd is an open shm object of at least len bytes.
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if p == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        NonNull::new(p as *mut u8).ok_or_else(|| io::Error::other("mmap returned null"))
    }

    impl Drop for Shm {
        fn drop(&mut self) {
            // SAFETY: ptr and len are the mapping made in map().
            unsafe { libc::munmap(self.ptr.as_ptr() as *mut libc::c_void, self.len) };
            if self.owner {
                if let Ok(c) = CString::new(self.name.as_str()) {
                    // SAFETY: c is a valid C string.
                    unsafe { libc::shm_unlink(c.as_ptr()) };
                }
            }
        }
    }
}

const _: () = assert!(std::mem::size_of::<Region>() == FIXED_BYTES);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::mem::{offset_of, size_of};
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::Ordering;

    fn field_size<T, F>(_: fn(&T) -> &F) -> usize {
        size_of::<F>()
    }

    /// (key, C expression, Rust value) for every struct size, every field's
    /// offset and size, and every constant in the header.
    macro_rules! layout {
        ($($c:literal $r:ident { $($f:ident)* })*) => {{
            let mut rows: Vec<(String, String, usize)> = Vec::new();
            $(
                rows.push((format!("sizeof {}", $c), format!("sizeof({})", $c), size_of::<$r>()));
                $(
                    rows.push((
                        format!("offsetof {}.{}", $c, stringify!($f)),
                        format!("offsetof({}, {})", $c, stringify!($f)),
                        offset_of!($r, $f),
                    ));
                    rows.push((
                        format!("sizeof {}.{}", $c, stringify!($f)),
                        format!("sizeof((({} *)0)->{})", $c, stringify!($f)),
                        field_size(|s: &$r| &s.$f),
                    ));
                )*
            )*
            rows
        }};
    }

    fn rows() -> Vec<(String, String, usize)> {
        let mut rows = layout! {
            "wwav_shm_header" Header {
                magic layout sample_rate block_size meter_slots meter_ring peaks_bytes input_bytes
                engine_pid engine_start_ns reserved
            }
            "wwav_shm_clock" Clock { seq sample_pos host_time_ns rate state dropouts callbacks reserved }
            "wwav_shm_crumb" Crumb { crumb crumb_seq reserved }
            "wwav_shm_meter_index" MeterIndex { meter_write reserved }
            "wwav_shm_meter_slot" MeterSlot { peak_l peak_r rms_l rms_r }
            "wwav_shm_meter_entry" MeterEntry { callback dsp_load dropouts slots_used reserved slots }
            "wwav_shm" Region { header clock crumb meters ring }
        };
        for (name, value) in [
            ("WWAV_SHM_LAYOUT", LAYOUT as usize),
            ("WWAV_PROTOCOL", crate::PROTOCOL as usize),
            ("WWAV_METER_SLOTS", METER_SLOTS),
            ("WWAV_METER_RING", METER_RING),
            ("WWAV_METER_ENTRY_HEADER", METER_ENTRY_HEADER),
            ("WWAV_METER_SLOT_BYTES", METER_SLOT_BYTES),
            ("WWAV_METER_ENTRY_BYTES", METER_ENTRY_BYTES),
            ("WWAV_STATE_STOPPED", STATE_STOPPED as usize),
            ("WWAV_STATE_PLAYING", STATE_PLAYING as usize),
            ("WWAV_STATE_RECORDING", STATE_RECORDING as usize),
            ("WWAV_SHM_RING_OFFSET", RING_OFFSET),
            ("WWAV_SHM_PEAKS_OFFSET", PEAKS_OFFSET),
            ("WWAV_SHM_FIXED_BYTES", FIXED_BYTES),
            ("WWAV_SHM_PEAKS_BYTES", PEAKS_BYTES),
            ("WWAV_SHM_INPUT_BYTES", INPUT_BYTES),
            ("WWAV_SHM_TOTAL_BYTES", TOTAL_BYTES),
            ("sizeof WWAV_SHM_MAGIC", MAGIC.len() + 1),
        ] {
            let expr = name
                .strip_prefix("sizeof ")
                .map_or(name.to_string(), |m| format!("sizeof({m})"));
            rows.push((name.to_string(), expr, value));
        }
        for (i, b) in MAGIC.iter().enumerate() {
            rows.push((
                format!("WWAV_SHM_MAGIC[{i}]"),
                format!("WWAV_SHM_MAGIC[{i}]"),
                *b as usize,
            ));
        }
        rows
    }

    /// Compiles a program against the real header and returns what it prints.
    fn c_layout(
        compiler: &str,
        args: &[&str],
        rows: &[(String, String, usize)],
    ) -> BTreeMap<String, usize> {
        let dir = tempfile::tempdir().unwrap();
        let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../engine/include");
        let mut src =
            String::from("#include <stdio.h>\n#include \"wwav_shm.h\"\nint main(void) {\n");
        for (key, expr, _) in rows {
            src +=
                &format!("  printf(\"%s\\t%llu\\n\", \"{key}\", (unsigned long long)({expr}));\n");
        }
        src += "  return 0;\n}\n";
        let c = dir.path().join("layout.c");
        std::fs::write(&c, src).unwrap();
        let exe = dir.path().join(format!("layout-{compiler}"));
        let out = Command::new(compiler)
            .args(args)
            .arg("-Wall")
            .arg("-Werror")
            .arg("-I")
            .arg(&include)
            .arg(&c)
            .arg("-o")
            .arg(&exe)
            .output()
            .unwrap_or_else(|e| panic!("can't run {compiler}: {e}"));
        assert!(
            out.status.success(),
            "{compiler} failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let run = Command::new(&exe).output().unwrap();
        assert!(run.status.success());
        String::from_utf8(run.stdout)
            .unwrap()
            .lines()
            .map(|l| {
                let (k, v) = l.split_once('\t').unwrap();
                (k.to_string(), v.parse().unwrap())
            })
            .collect()
    }

    fn assert_same_layout(compiler: &str, args: &[&str]) {
        let rows = rows();
        let c = c_layout(compiler, args, &rows);
        assert_eq!(c.len(), rows.len(), "every row printed once");
        let mismatches: Vec<String> = rows
            .iter()
            .filter(|(key, _, rust)| c[key] != *rust)
            .map(|(key, _, rust)| format!("{key}: C says {}, Rust says {rust}", c[key]))
            .collect();
        assert!(
            mismatches.is_empty(),
            "{compiler}: the layouts differ:\n{}",
            mismatches.join("\n")
        );
    }

    #[test]
    fn the_rust_mirror_matches_the_header_compiled_as_c() {
        assert_same_layout("cc", &["-std=c99"]);
    }

    #[test]
    fn the_rust_mirror_matches_the_header_compiled_as_cpp() {
        // The engine includes it from C++17, where its static_asserts run too.
        assert_same_layout("c++", &["-std=c++17", "-x", "c++"]);
    }

    #[test]
    fn the_layout_check_would_catch_a_moved_field() {
        // The check compares numbers it computes on both sides; make sure a
        // wrong Rust number shows up as a mismatch rather than passing.
        let mut rows = rows();
        let c = c_layout("cc", &["-std=c99"], &rows);
        let row = rows
            .iter_mut()
            .find(|r| r.0 == "offsetof wwav_shm_clock.rate")
            .unwrap();
        row.2 += 8;
        assert_ne!(c[&row.0], row.2);
    }

    fn region() -> Shm {
        Shm::create_for_app().unwrap()
    }

    #[test]
    fn the_header_is_written_magic_last_and_read_back() {
        let shm = region();
        let h = &shm.region().header;
        assert!(h.read().is_err(), "a zeroed region has no header yet");
        let fields = HeaderFields {
            sample_rate: 48000,
            block_size: 128,
            engine_pid: 4321,
            engine_start_ns: 99,
        };
        h.write(&fields);
        assert_eq!(h.read().unwrap(), fields);
        h.set_format(44100, 256);
        assert_eq!(h.read().unwrap().sample_rate, 44100);
        assert_eq!(h.read().unwrap().block_size, 256);
        assert_eq!(h.meter_slots.load(Ordering::Relaxed), METER_SLOTS as u32);
        assert_eq!(h.peaks_bytes.load(Ordering::Relaxed), PEAKS_BYTES as u32);
    }

    #[test]
    fn a_header_of_another_layout_is_refused() {
        let shm = region();
        let h = &shm.region().header;
        h.write(&HeaderFields {
            sample_rate: 48000,
            block_size: 128,
            engine_pid: 1,
            engine_start_ns: 1,
        });
        h.layout.store(2, Ordering::Relaxed);
        assert!(h.read().unwrap_err().contains("layout 2"));
    }

    fn clock(k: u64) -> ClockFields {
        ClockFields {
            sample_pos: k as i64 * 128 - 312,
            host_time_ns: 1_000 + k * 2_666_667,
            rate: 48000.0,
            state: STATE_PLAYING,
            dropouts: 2,
            callbacks: k,
        }
    }

    #[test]
    fn the_clock_round_trips_and_seq_stays_even() {
        let shm = region();
        let c = &shm.region().clock;
        assert_eq!(
            c.read(),
            Some(ClockFields::default()),
            "a zeroed clock reads as stopped at 0"
        );
        for k in 1..5 {
            c.write(&clock(k));
            assert_eq!(c.read(), Some(clock(k)));
            assert_eq!(c.seq.load(Ordering::Relaxed), 2 * k);
        }
    }

    #[test]
    fn a_writer_killed_mid_write_leaves_no_half_clock() {
        let shm = region();
        let c = &shm.region().clock;
        c.write(&clock(1));
        // An engine killed between the two halves of a write.
        c.seq.store(3, Ordering::Relaxed);
        assert_eq!(c.read(), None);
        // The next engine's first write makes it readable again.
        c.write(&clock(2));
        assert_eq!(c.read(), Some(clock(2)));
        assert_eq!(c.seq.load(Ordering::Relaxed) % 2, 0);
    }

    #[test]
    fn the_cursor_extrapolates_from_the_clock() {
        let f = ClockFields {
            sample_pos: 48_000,
            host_time_ns: 1_000_000_000,
            rate: 48_000.0,
            ..Default::default()
        };
        assert_eq!(f.playhead_at(1_500_000_000), 72_000.0);
        assert_eq!(f.playhead_at(500_000_000), 24_000.0);
        let stopped = ClockFields { rate: 0.0, ..f };
        assert_eq!(stopped.playhead_at(9_000_000_000), 48_000.0);
    }

    #[test]
    fn the_crumb_names_a_node() {
        let shm = region();
        let crumb = &shm.region().crumb;
        assert_eq!(crumb.get(), 0);
        crumb.set(crumb_hash("01JC5T0000000000000000TAPE"));
        crumb.clear();
        crumb.set(crumb_hash("01JC5T0000000000000000ECHO"));
        assert_eq!(crumb.seq(), 3, "every write counts");
        let ids = ["01JC5T0000000000000000TAPE", "01JC5T0000000000000000ECHO"];
        assert_eq!(
            crumb_node(crumb.get(), ids),
            Some("01JC5T0000000000000000ECHO")
        );
        crumb.clear();
        assert_eq!(crumb_node(crumb.get(), ids), None);
        assert_eq!(
            crumb_hash("a"),
            0xaf63dc4c8601ec8c,
            "FNV-1a 64, as wwav_ids::fnv1a64"
        );
    }

    #[test]
    fn the_meter_ring_gives_the_newest_entry_by_slot() {
        let shm = region();
        let r = shm.region();
        assert!(r.newest_meters().is_none(), "nothing written yet");
        for k in 0..(METER_RING as u64 * 2 + 3) {
            let level = k as f32 / 1000.0;
            r.write_meters(
                k,
                0.25,
                1,
                &[
                    [level, level / 2.0, level / 3.0, level / 4.0],
                    [1.0, 1.0, 0.5, 0.5],
                ],
            );
        }
        let k = METER_RING as u64 * 2 + 2;
        let m = r.newest_meters().unwrap();
        assert_eq!(m.callback, k);
        assert_eq!(m.dsp_load, 0.25);
        assert_eq!(m.dropouts, 1);
        assert_eq!(m.slots.len(), 2);
        let level = k as f32 / 1000.0;
        assert_eq!(m.slots[0], [level, level / 2.0, level / 3.0, level / 4.0]);
        assert_eq!(m.slots[1], [1.0, 1.0, 0.5, 0.5]);
        assert_eq!(r.meters.meter_write.load(Ordering::Relaxed), k + 1);
        // The entry sits where the contract says: 256 + (i mod MR) × entry bytes.
        let base = r as *const Region as usize;
        let entry = &r.ring[(k as usize) % METER_RING] as *const MeterEntry as usize;
        assert_eq!(
            entry - base,
            256 + (k as usize % METER_RING) * METER_ENTRY_BYTES
        );
    }

    #[test]
    fn too_many_slots_are_cut_to_the_ring_width() {
        let shm = region();
        let r = shm.region();
        r.write_meters(1, 0.0, 0, &vec![[0.5; 4]; METER_SLOTS + 10]);
        assert_eq!(r.newest_meters().unwrap().slots.len(), METER_SLOTS);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_region_is_owner_only_and_unlinked_by_its_creator() {
        use std::os::unix::fs::PermissionsExt;
        let shm = region();
        let path = format!("/dev/shm{}", shm.name());
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(&path).unwrap().len() as usize,
            TOTAL_BYTES
        );
        // The engine's mapping doesn't unlink it; the app's does.
        let engine = Shm::open(shm.name()).unwrap();
        drop(engine);
        assert!(Path::new(&path).exists());
        drop(shm);
        assert!(!Path::new(&path).exists());
    }

    #[test]
    fn both_sides_see_the_same_memory() {
        let app = region();
        let engine = Shm::open(app.name()).unwrap();
        engine.region().clock.write(&clock(7));
        assert_eq!(app.region().clock.read(), Some(clock(7)));
        assert!(Shm::open("/wwav-no-such-region").is_err());
    }

    #[test]
    fn names_follow_the_contract() {
        let a = region();
        let b = region();
        let prefix = format!("/wwav-{}-", std::process::id());
        assert!(a.name().starts_with(&prefix), "{}", a.name());
        assert_ne!(a.name(), b.name());
        // macOS caps POSIX shm names at 31 bytes.
        assert!(a.name().len() <= 31);
    }

    #[test]
    fn monotonic_time_moves_forward() {
        let a = monotonic_ns();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = monotonic_ns();
        assert!(b - a >= 5_000_000, "{a} {b}");
    }
}
