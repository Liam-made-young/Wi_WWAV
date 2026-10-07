//! The two checks the export sheet runs on the four stem buses (6.6): that
//! they sum to the master, and that none is too loud for 16 bits.

/// File order: PRANA's faders, Wi's faders, and every stem lane (6.1).
pub const STEM_NAMES: [&str; 4] = ["vocals", "drums", "other", "bass"];

/// The fold check passes only under this.
pub const FOLD_LIMIT_DBFS: f64 = -80.0;

/// How far the master is from the sum of the four stems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FoldCheck {
    peak_dbfs: f64,
}

/// Subtracts the summed stems from the master, both as rendered with the
/// master chain bypassed, and keeps the largest difference. All five are the
/// same length and layout (interleaved stereo, as the engine renders them).
pub fn fold_check(stems: [&[f32]; 4], master: &[f32]) -> FoldCheck {
    for s in stems {
        assert_eq!(s.len(), master.len(), "stems and master differ in length");
    }
    let mut peak = 0.0f64;
    for (i, &m) in master.iter().enumerate() {
        let sum: f64 = stems.iter().map(|s| s[i] as f64).sum();
        let d = (m as f64 - sum).abs();
        // A NaN from a misbehaving plugin is as far apart as it gets.
        peak = peak.max(if d.is_nan() { f64::INFINITY } else { d });
    }
    FoldCheck {
        peak_dbfs: dbfs(peak),
    }
}

impl FoldCheck {
    /// The largest difference, in dB against full scale.
    pub fn peak_dbfs(&self) -> f64 {
        self.peak_dbfs
    }

    pub fn passes(&self) -> bool {
        self.peak_dbfs < FOLD_LIMIT_DBFS
    }

    /// The sheet's line. `cause` names what keeps the stems from summing,
    /// usually a non-linear plugin on a return: "Tape Sat on the drums
    /// reverb return".
    pub fn line(&self, cause: &str) -> String {
        if self.passes() {
            "Stems sum to the master.".to_string()
        } else if self.peak_dbfs.is_finite() {
            format!(
                "Stems don't sum to the master: {} dBFS apart, from {cause}.",
                signed(self.peak_dbfs)
            )
        } else {
            format!("Stems don't sum to the master: some samples aren't numbers, from {cause}.")
        }
    }
}

/// Each stem's sample peak.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StemPeaks {
    dbfs: [f64; 4],
}

/// Measures the stems as they will be written: after resampling and before
/// dither, since resampling can raise a peak.
///
/// Only a peak over 0 dBFS is named (6.6). A stem at or just under it can
/// have dither push a sample past 32,767, where `to_16_bit` clamps it; that
/// sample is still within 1 LSB of its value, inside the 1.5 LSB any
/// dithered sample moves, so the sheet says nothing of it.
pub fn stem_peaks(stems: [&[f32]; 4]) -> StemPeaks {
    StemPeaks {
        dbfs: stems.map(|s| {
            dbfs(s.iter().fold(0.0f64, |p, &x| {
                let a = (x as f64).abs();
                // A NaN would slip past `max`; it counts as infinite.
                if a.is_nan() {
                    f64::INFINITY
                } else {
                    p.max(a)
                }
            }))
        }),
    }
}

impl StemPeaks {
    /// Peaks in dBFS, in file order; −∞ for a silent stem and +∞ for one
    /// with samples that aren't numbers (NaN or infinite).
    pub fn dbfs(&self) -> [f64; 4] {
        self.dbfs
    }

    /// The whole dB to lower all four stems by so none is over 0 dBFS: the
    /// loudest peak as `lines` shows it, rounded up. 0 when none is over.
    /// Stems with samples that aren't numbers don't count, since no cut
    /// fixes them.
    pub fn cut_db(&self) -> u32 {
        let loudest = self
            .dbfs
            .iter()
            .filter(|db| db.is_finite())
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        if loudest > 0.0 {
            tenths_up(loudest).div_ceil(10) as u32
        } else {
            0
        }
    }

    /// The sheet's two lines when a stem is over 0 dBFS, naming every one:
    /// "drums peaks at +1.8 dBFS. Lower all four stems by 2 dB." and what
    /// that means for a player that sums them. A peak shows rounded up to a
    /// tenth, so it never reads lower than it is, and the cut is that,
    /// rounded up to a whole dB. Stems with samples that aren't numbers are
    /// named first and alone, since they can't be written at any level.
    /// None when all fit.
    pub fn lines(&self) -> Option<[String; 2]> {
        let broken: Vec<String> = STEM_NAMES
            .iter()
            .zip(self.dbfs)
            .filter(|(_, db)| *db == f64::INFINITY)
            .map(|(name, _)| name.to_string())
            .collect();
        if let Some(named) = and_list(&broken) {
            let verb = if broken.len() == 1 { "has" } else { "have" };
            return Some([
                format!("{named} {verb} samples that aren't numbers."),
                "At 16 bits they would be written as silence or full scale.".to_string(),
            ]);
        }

        let over: Vec<String> = STEM_NAMES
            .iter()
            .zip(self.dbfs)
            .filter(|(_, db)| *db > 0.0)
            .enumerate()
            .map(|(i, (name, db))| {
                let verb = if i == 0 { " peaks" } else { "" };
                let t = tenths_up(db);
                format!("{name}{verb} at +{}.{} dBFS", t / 10, t % 10)
            })
            .collect();
        let named = and_list(&over)?;
        let cut = self.cut_db();
        Some([
            format!("{named}. Lower all four stems by {cut} dB."),
            format!(
                "The master is unchanged. A player summing the stems will play {cut} dB quieter."
            ),
        ])
    }
}

/// "a", "a and b", "a, b and c"; None for none.
fn and_list(items: &[String]) -> Option<String> {
    let (last, rest) = items.split_last()?;
    Some(if rest.is_empty() {
        last.clone()
    } else {
        format!("{} and {last}", rest.join(", "))
    })
}

/// A positive level in tenths of a dB, rounded up: 1.84 → 19, 0.01 → 1.
fn tenths_up(db: f64) -> u64 {
    (db * 10.0).ceil() as u64
}

fn dbfs(peak: f64) -> f64 {
    20.0 * libm::log10(peak)
}

/// A level to one decimal, signed as the spec sets it: "+1.8", "−78.4".
fn signed(db: f64) -> String {
    let s = format!("{:.1}", db.abs());
    if s == "0.0" {
        s
    } else if db < 0.0 {
        format!("−{s}")
    } else {
        format!("+{s}")
    }
}
