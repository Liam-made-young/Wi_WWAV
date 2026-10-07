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
        } else {
            format!(
                "Stems don't sum to the master: {} dBFS apart, from {cause}.",
                signed(self.peak_dbfs)
            )
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
pub fn stem_peaks(stems: [&[f32]; 4]) -> StemPeaks {
    StemPeaks {
        dbfs: stems.map(|s| dbfs(s.iter().fold(0.0f64, |p, &x| p.max((x as f64).abs())))),
    }
}

impl StemPeaks {
    /// Peaks in dBFS, in file order; −∞ for a silent stem.
    pub fn dbfs(&self) -> [f64; 4] {
        self.dbfs
    }

    /// The whole dB to lower all four stems by so none is over 0 dBFS: the
    /// loudest peak, rounded up. 0 when none is over.
    pub fn cut_db(&self) -> u32 {
        let loudest = self.dbfs.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        if loudest > 0.0 {
            loudest.ceil() as u32
        } else {
            0
        }
    }

    /// The sheet's two lines when a stem is over 0 dBFS, naming every one:
    /// "drums peaks at +1.8 dBFS. Lower all four stems by 2 dB." and what
    /// that means for a player that sums them. None when all fit.
    pub fn lines(&self) -> Option<[String; 2]> {
        let over: Vec<String> = STEM_NAMES
            .iter()
            .zip(self.dbfs)
            .filter(|(_, db)| *db > 0.0)
            .enumerate()
            .map(|(i, (name, db))| {
                let verb = if i == 0 { " peaks" } else { "" };
                format!("{name}{verb} at {} dBFS", signed(db))
            })
            .collect();
        let (last, rest) = over.split_last()?;
        let named = if rest.is_empty() {
            last.clone()
        } else {
            format!("{} and {last}", rest.join(", "))
        };
        let cut = self.cut_db();
        Some([
            format!("{named}. Lower all four stems by {cut} dB."),
            format!(
                "The master is unchanged. A player summing the stems will play {cut} dB quieter."
            ),
        ])
    }
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
