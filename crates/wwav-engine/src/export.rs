//! Writing a `.wwav` (`docs/ENGINE.md` 3.11; `docs/SPEC.md` 6.1, 6.6, 6.7):
//! the session's four stem buses and its master, rendered by the graph that
//! plays them, brought to 44.1 kHz, taken to 16 bits once at the last step,
//! and laid out by `wwav-formats`, which owns the format.

use crate::graph::{Graph, ROLES};
use crate::wav::to_s16;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use wwav_dsp::dither::to_16_bit;
use wwav_dsp::fold::FOLD_LIMIT_DBFS;
use wwav_dsp::resample::{resampled_len, Resampler};
use wwav_formats::meta::{Lineage, SongMeta};
use wwav_formats::writer::WwavWriter;
use wwav_formats::wwav::RATE;

pub struct Written {
    /// Frames in the file, at 44.1 kHz.
    pub frames: u64,
    pub bytes: u64,
    /// How far the master is from the sum of the four stems at its worst, in
    /// dB against full scale (`-inf` when they are the same), before either
    /// was taken to 16 bits.
    pub fold_dbfs: f64,
    /// The loudest sample of any stem or the master: over 1.0 was clipped.
    pub peak: f32,
}

impl Written {
    pub fn folds(&self) -> bool {
        self.fold_dbfs < FOLD_LIMIT_DBFS
    }
}

/// One of the five things a `.wwav` holds, on its way to 16 bits.
struct Lane {
    resampler: Option<Resampler>,
    /// Interleaved frames at 44.1 kHz not yet written.
    ready: Vec<f32>,
    seed: u64,
}

impl Lane {
    fn feed(&mut self, l: &[f32], r: &[f32], scratch: &mut Vec<f32>) {
        scratch.clear();
        for (a, b) in l.iter().zip(r) {
            scratch.extend_from_slice(&[*a, *b]);
        }
        match &mut self.resampler {
            Some(rs) => rs.process(scratch, &mut self.ready),
            None => self.ready.extend_from_slice(scratch),
        }
    }

    fn finish(&mut self) {
        if let Some(rs) = self.resampler.take() {
            rs.finish(&mut self.ready);
        }
    }

    /// The first `samples` of what is ready, as 16-bit. Each call dithers
    /// with a seed of its own, so every lane's noise is its own and a file
    /// written twice is the same bytes.
    fn take(&mut self, samples: usize, dither: bool) -> Vec<i16> {
        let chunk: Vec<f32> = self.ready.drain(..samples).collect();
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        if dither {
            to_16_bit(&chunk, self.seed).into_samples()
        } else {
            chunk.iter().map(|x| to_s16(*x)).collect()
        }
    }
}

/// Renders session samples `start..start + len` of `g` into a `.wwav` at
/// `path`. `dither` off rounds instead, which gives 16-bit sources at unity
/// back bit for bit. `progress` hears how many session frames are done.
#[allow(clippy::too_many_arguments)]
pub fn write_wwav(
    g: &mut Graph,
    block: usize,
    start: i64,
    len: i64,
    path: &Path,
    meta: &SongMeta,
    lineage: &Lineage,
    dither: bool,
    stopping: &AtomicBool,
    progress: &mut dyn FnMut(i64),
) -> Result<Written, String> {
    let rate = g.index.sample_rate;
    let frames = if rate == RATE {
        len as u64
    } else {
        resampled_len(len as u64, rate, RATE)
    };
    let mut file =
        WwavWriter::create(path, frames, meta, lineage, None).map_err(|e| e.to_string())?;
    // The master, then the stems in file order. Seeds differ by lane and by
    // song, so two songs' noise isn't the same noise.
    let song = meta.song_id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let mut lanes: Vec<Lane> = (0..=ROLES as u64)
        .map(|i| Lane {
            resampler: (rate != RATE).then(|| Resampler::new(rate, RATE, 2)),
            ready: Vec::new(),
            seed: song ^ (i + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15),
        })
        .collect();
    let (mut scratch, mut worst, mut peak) = (Vec::new(), 0.0f64, 0.0f32);
    let mut written = 0u64;
    let mut flush =
        |lanes: &mut Vec<Lane>, file: &mut WwavWriter, last: bool| -> Result<(), String> {
            // Every lane was fed the same frames, so they hold the same count.
            let mut samples = lanes.iter().map(|l| l.ready.len()).min().unwrap_or(0);
            if last {
                // Exactly the frames the header promised.
                samples = samples.min(((frames - written) * 2) as usize);
            }
            if samples == 0 {
                return Ok(());
            }
            for i in 0..samples {
                let sum: f64 = lanes[1..].iter().map(|l| l.ready[i] as f64).sum();
                let d = (lanes[0].ready[i] as f64 - sum).abs();
                worst = worst.max(if d.is_nan() { f64::INFINITY } else { d });
                for l in lanes.iter() {
                    peak = peak.max(l.ready[i].abs());
                }
            }
            let pcm: Vec<Vec<i16>> = lanes.iter_mut().map(|l| l.take(samples, dither)).collect();
            file.write_master(&pcm[0]).map_err(|e| e.to_string())?;
            file.write_stems([&pcm[1], &pcm[2], &pcm[3], &pcm[4]])
                .map_err(|e| e.to_string())?;
            written += samples as u64 / 2;
            Ok(())
        };

    g.settle();
    let mut done = 0i64;
    while done < len {
        if stopping.load(Ordering::Relaxed) {
            return Err("The engine stopped during the export.".into());
        }
        let n = (block as i64).min(len - done) as usize;
        g.begin_block(n);
        g.process(start + done, 0, n, true, false, None);
        g.end_block(n, None);
        let (l, r) = g.master_out();
        lanes[0].feed(&l[..n], &r[..n], &mut scratch);
        for role in 0..ROLES {
            let (l, r) = g.bus(role);
            lanes[1 + role].feed(&l[..n], &r[..n], &mut scratch);
        }
        flush(&mut lanes, &mut file, false)?;
        done += n as i64;
        progress(done);
    }
    for l in &mut lanes {
        l.finish();
    }
    flush(&mut lanes, &mut file, true)?;
    if written != frames {
        return Err(format!(
            "The export came to {written} frames, not the {frames} it was laid out for."
        ));
    }
    let bytes = file.finish().map_err(|e| e.to_string())?;
    Ok(Written {
        frames,
        bytes,
        fold_dbfs: if worst > 0.0 {
            20.0 * worst.log10()
        } else {
            f64::NEG_INFINITY
        },
        peak,
    })
}
