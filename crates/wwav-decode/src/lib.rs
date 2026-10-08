//! Decoding the common audio files, and converting one to a WAV the engine
//! plays.
//!
//! The engine plays WAV and `.wwav` directly. Every other file, and any file
//! whose rate differs from the session's, is converted once to a 32-bit float
//! WAV at the session's rate ([`to_wav`]) and played like any other WAV.
//! [`probe`] says what a file is without decoding it all, and [`Decoder`]
//! hands out its frames a packet at a time.
//!
//! The decoders are symphonia's, all in Rust: WAV and AIFF (PCM, ADPCM,
//! A-law, mu-law), CAF, FLAC, MP3, AAC and ALAC in MP4, Vorbis in Ogg, and
//! those codecs in Matroska. Integer samples come out divided by their full
//! scale (a 16-bit `v` is exactly `v / 32768.0`) and float samples as they
//! are. The resampler is `wwav_dsp`'s, so a conversion gives the same bytes
//! on every run.
//!
//! A clip has to start and end where its file says, to the frame, so that it
//! sits on the grid with the others. Symphonia 0.5 leaves three things a
//! container says unused: an MP4's AAC encoder delay, the end of an AIFF's
//! sound, and the end of a short Ogg stream. `Decoder` applies them (see
//! `container`), and gives a length only where the container states one.

mod container;
mod wav;

use std::fs::{self, File};
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{
    CodecType, DecoderOptions, CODEC_TYPE_AAC, CODEC_TYPE_MP1, CODEC_TYPE_MP2, CODEC_TYPE_MP3,
    CODEC_TYPE_NULL, CODEC_TYPE_OPUS, CODEC_TYPE_WMA,
};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use container::{Container, Edit};
pub use wav::{to_wav, Converted};

/// What went wrong, as a sentence fit to show.
#[derive(Debug)]
pub enum Error {
    /// Nothing is at the path: "No file at /x/y.mp3."
    NoSuchFile(String),
    /// A format or a layout this doesn't read: "song.wma is not a format this
    /// reads.", "x.flac has 6 channels; clips are mono or stereo."
    Unsupported(String),
    /// The file is damaged: "x.mp3 doesn't decode: " and the decoder's reason.
    Bad(String),
    Io(io::Error),
    Cancelled,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NoSuchFile(s) | Error::Unsupported(s) | Error::Bad(s) => f.write_str(s),
            Error::Io(e) => write!(f, "The file couldn't be read or written: {e}."),
            Error::Cancelled => f.write_str("The conversion was cancelled."),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// What a file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    pub rate: u32,
    /// 1 or 2. A file with more is refused as [`Error::Unsupported`].
    pub channels: u16,
    /// The file's length in frames when its container says, after any
    /// encoder delay and padding the file declares are taken off.
    pub frames: Option<u64>,
    /// "pcm", "flac", "mp3", "aac", "alac", "vorbis", "adpcm", "alaw", …
    pub codec: String,
}

/// What `path` holds, from its headers and its first packet.
pub fn probe(path: &Path) -> Result<Info, Error> {
    Decoder::open(path).map(|d| d.info)
}

/// A file being decoded from its start to its end.
pub struct Decoder {
    format: Box<dyn FormatReader>,
    codec: Box<dyn symphonia::core::codecs::Decoder>,
    track: u32,
    info: Info,
    /// The file's name, for error sentences.
    name: String,
    samples: Option<SampleBuffer<f32>>,
    /// The rate and channel count of the first packet decoded, which every
    /// later one must share.
    spec: Option<(u32, usize)>,
    /// Frames `open` decoded to learn the spec, kept for the first `read`.
    ahead: Vec<f32>,
    /// Frames still to drop from the front: an MP4's encoder delay.
    skip: u64,
    /// Frames still to give, when the container says where the sound ends
    /// and symphonia would read past it.
    left: Option<u64>,
    /// Why the last packet skipped wouldn't decode, in case none does.
    skipped: Option<&'static str>,
    over: bool,
}

impl Decoder {
    pub fn open(path: &Path) -> Result<Decoder, Error> {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let file = match fs::metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(Error::NoSuchFile(format!("No file at {}.", path.display())));
            }
            Err(e) => return Err(Error::Io(e)),
            Ok(m) if m.is_dir() => {
                return Err(Error::Unsupported(format!(
                    "{name} is a folder, not a file."
                )));
            }
            Ok(_) => File::open(path)?,
        };
        // A decoder that panics on a damaged file is a damaged file, not a
        // reason to take the engine down with it.
        let opened = catch_unwind(AssertUnwindSafe(|| {
            let container = container::sniff(path);
            let (mut decoder, said) = Decoder::start(file, path, &name, container)?;
            decoder.settle(said)?;
            Ok(decoder)
        }));
        opened.unwrap_or_else(|_| Err(gave_up(&name)))
    }

    pub fn info(&self) -> &Info {
        &self.info
    }

    /// The next frames, interleaved at the file's own channel count and rate,
    /// appended to `out`. Returns the frames appended, and 0 at the end.
    pub fn read(&mut self, out: &mut Vec<f32>) -> Result<usize, Error> {
        if !self.ahead.is_empty() {
            let frames = self.ahead.len() / self.info.channels as usize;
            out.append(&mut self.ahead);
            return Ok(frames);
        }
        let before = out.len();
        // Past the end the container gives, the rest of the file is still
        // read through and dropped, so that what is wrong with it (a second
        // stream chained on) is said and not passed over.
        let read = catch_unwind(AssertUnwindSafe(|| {
            while self.packet(out)?.is_some() {
                let kept = self.trim(out, before);
                if kept > 0 {
                    return Ok(kept);
                }
            }
            Ok(0)
        }));
        read.unwrap_or_else(|_| {
            out.truncate(before);
            self.over = true;
            Err(gave_up(&self.name))
        })
    }

    /// Finds the format, the first track with sound and its decoder.
    fn start(
        file: File,
        path: &Path,
        name: &str,
        container: Container,
    ) -> Result<(Decoder, Said), Error> {
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        // Gapless: the reader marks the encoder's delay and padding on the
        // packets and the decoder leaves them out. The MP3 and Ogg readers
        // do this; the MP4 reader doesn't, hence `trim`.
        let options = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let not_a_format = || Error::Unsupported(format!("{name} is not a format this reads."));
        let mut format = match symphonia::default::get_probe().format(
            &hint,
            stream,
            &options,
            &MetadataOptions::default(),
        ) {
            Ok(probed) => probed.format,
            // The probe runs off the end of a short file that is nothing it
            // knows.
            Err(SymError::Unsupported(_)) => return Err(not_a_format()),
            Err(SymError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(not_a_format());
            }
            Err(e) => return Err(broken(name, e)),
        };

        let Some(track) = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        else {
            return Err(Error::Unsupported(format!("{name} has no sound in it.")));
        };
        let track_id = track.id;
        let params = track.codec_params.clone();
        let (codec_name, known) = codec_name(params.codec);
        if let Some(count) = params.channels.map(|c| c.count()) {
            check_channels(name, count)?;
        }
        let codec = match symphonia::default::get_codecs().make(&params, &DecoderOptions::default())
        {
            Ok(codec) => codec,
            // A codec this has no decoder for, or one whose decoder can't
            // work from what this container gives it (AAC in CAF, PCM in
            // Matroska).
            Err(SymError::Unsupported(_)) if known => {
                return Err(Error::Unsupported(format!(
                    "{name} holds {codec_name} sound in a form this doesn't read."
                )));
            }
            Err(SymError::Unsupported(_)) if codec_name == UNNAMED => {
                return Err(Error::Unsupported(format!(
                    "{name} holds sound in a codec this doesn't read."
                )));
            }
            Err(SymError::Unsupported(_)) => {
                return Err(Error::Unsupported(format!(
                    "{name} holds {codec_name} sound, which this doesn't read."
                )));
            }
            Err(e) => return Err(broken(name, e)),
        };

        let rate = params.sample_rate.unwrap_or(0);
        // Some readers only estimate `n_frames`: an MP3 with no LAME header
        // and a raw AAC stream are sized from their bitrate. Those are
        // lengths the container doesn't say.
        let frames = match (&container, params.codec) {
            (Container::Aiff { frames }, _) => Some(*frames),
            (Container::Mp4 { .. }, _) => params.n_frames.filter(|&n| n > 0),
            (Container::Other, CODEC_TYPE_AAC) => None,
            (Container::Other, CODEC_TYPE_MP1 | CODEC_TYPE_MP2 | CODEC_TYPE_MP3) => {
                params.n_frames.filter(|_| params.delay.is_some())
            }
            (Container::Ogg | Container::Other, _) => params.n_frames,
        };
        let trim = match container {
            // Where symphonia would read past the end the container gives.
            Container::Aiff { .. } | Container::Ogg => frames.map(|n| Edit {
                start: 0,
                frames: Some(n),
                timescale: rate,
            }),
            Container::Mp4 { edit } if params.codec == CODEC_TYPE_AAC => {
                edit.or_else(|| itunes_edit(format.as_mut(), rate))
            }
            _ => None,
        };
        let said = Said {
            trim,
            ticks: params.time_base.filter(|t| t.numer == 1).map(|t| t.denom),
        };
        let info = Info {
            rate,
            channels: params.channels.map_or(0, |c| c.count() as u16),
            frames,
            codec: codec_name.to_string(),
        };
        let decoder = Decoder {
            format,
            codec,
            track: track_id,
            info,
            name: name.to_string(),
            samples: None,
            spec: None,
            ahead: Vec::new(),
            skip: 0,
            left: None,
            skipped: None,
            over: false,
        };
        Ok((decoder, said))
    }

    /// Decodes the first packet, because the rate and channels a decoder
    /// gives are the ones `read` returns and a container's header can say
    /// otherwise (symphonia's MP4 reader calls every ALAC track 44.1 kHz).
    /// A file with no packets keeps what its header said.
    fn settle(&mut self, said: Said) -> Result<(), Error> {
        let mut first = Vec::new();
        while let Some(0) = self.packet(&mut first)? {}
        if let Some((rate, channels)) = self.spec {
            self.info.rate = rate;
            self.info.channels = channels as u16;
        } else if let Some(reason) = self.skipped {
            return Err(Error::Bad(format!(
                "{} doesn't decode: {reason}.",
                self.name
            )));
        }
        if self.info.rate == 0 || self.info.channels == 0 {
            return Err(Error::Bad(format!(
                "{} doesn't decode: it has no sound and doesn't say its rate.",
                self.name
            )));
        }
        check_channels(&self.name, self.info.channels as usize)?;

        // The length counts in the track's time base, which is frames in most
        // containers and milliseconds in Matroska.
        if said.ticks != Some(self.info.rate) {
            self.info.frames = None;
        }
        // An edit counts in the track's timescale too.
        if let Some(edit) = said.trim.filter(|e| e.timescale == self.info.rate) {
            let rest = self.info.frames.map(|n| n.saturating_sub(edit.start));
            self.skip = edit.start;
            self.left = match (edit.frames, rest) {
                (Some(n), Some(rest)) => Some(n.min(rest)),
                (n, rest) => n.or(rest),
            };
            self.info.frames = self.left;
        }
        self.trim(&mut first, 0);
        self.ahead = first;
        Ok(())
    }

    /// Decodes one packet onto `out`, as the decoder gives it. `None` at the
    /// end of the file.
    fn packet(&mut self, out: &mut Vec<f32>) -> Result<Option<usize>, Error> {
        if self.over {
            return Ok(None);
        }
        let packet = loop {
            match self.format.next_packet() {
                Ok(p) if p.track_id() == self.track => break p,
                Ok(_) => continue,
                // How symphonia says a file has ended; a file cut short ends
                // the same way.
                Err(SymError::IoError(e)) if e.kind() == io::ErrorKind::UnexpectedEof => {
                    self.over = true;
                    return Ok(None);
                }
                Err(SymError::ResetRequired) => {
                    self.over = true;
                    return Err(Error::Unsupported(format!(
                        "{} is several streams one after another; this reads a single stream.",
                        self.name
                    )));
                }
                Err(e) => {
                    self.over = true;
                    return Err(broken(&self.name, e));
                }
            }
        };
        let decoded = match self.codec.decode(&packet) {
            Ok(decoded) => decoded,
            // One packet that won't decode is skipped and the rest plays.
            Err(SymError::DecodeError(reason)) => {
                self.skipped = Some(reason);
                return Ok(Some(0));
            }
            Err(e) => {
                self.over = true;
                return Err(broken(&self.name, e));
            }
        };

        let spec = *decoded.spec();
        let now = (spec.rate, spec.channels.count());
        if *self.spec.get_or_insert(now) != now {
            self.over = true;
            return Err(Error::Unsupported(format!(
                "{} changes its rate or its channels part-way through.",
                self.name
            )));
        }
        if now.1 == 0 || decoded.frames() == 0 {
            return Ok(Some(0));
        }
        let room = decoded.capacity();
        let samples = match &mut self.samples {
            Some(s) if s.capacity() >= room * now.1 => s,
            slot => slot.insert(SampleBuffer::new(room as u64, spec)),
        };
        samples.copy_interleaved_ref(decoded);
        out.extend_from_slice(samples.samples());
        Ok(Some(samples.len() / now.1))
    }

    /// Takes the container's trim off the frames in `out[from..]`, and
    /// returns how many are kept.
    fn trim(&mut self, out: &mut Vec<f32>, from: usize) -> usize {
        let channels = self.info.channels as usize;
        let mut frames = ((out.len() - from) / channels) as u64;
        let drop = self.skip.min(frames);
        out.drain(from..from + drop as usize * channels);
        self.skip -= drop;
        frames -= drop;
        if let Some(left) = &mut self.left {
            frames = frames.min(*left);
            *left -= frames;
            out.truncate(from + frames as usize * channels);
        }
        frames as usize
    }
}

/// What a file's headers say about its length, to be held against the rate
/// its first packet decodes at.
struct Said {
    trim: Option<Edit>,
    /// Ticks a second the length is counted in.
    ticks: Option<u32>,
}

/// Apple's encoders say an AAC track's delay, padding and true length in an
/// `iTunSMPB` tag, as hex numbers: a zero, the delay, the padding, the length.
fn itunes_edit(format: &mut dyn FormatReader, rate: u32) -> Option<Edit> {
    let metadata = format.metadata();
    let tag = metadata
        .current()?
        .tags()
        .iter()
        .find(|t| t.key.ends_with(":iTunSMPB"))?;
    let text = tag.value.to_string();
    let mut numbers = text
        .split_whitespace()
        .map(|n| u64::from_str_radix(n, 16).ok());
    let (_, start, _, frames) = (
        numbers.next()??,
        numbers.next()??,
        numbers.next()??,
        numbers.next()??,
    );
    Some(Edit {
        start,
        frames: (frames > 0).then_some(frames),
        timescale: rate,
    })
}

fn check_channels(name: &str, count: usize) -> Result<(), Error> {
    if count > 2 {
        return Err(Error::Unsupported(format!(
            "{name} has {count} channels; clips are mono or stereo."
        )));
    }
    Ok(())
}

/// Symphonia's errors start in lower case and name the part that raised
/// them ("mp3: invalid main_data_begin"), which reads well enough after a
/// colon.
fn broken(name: &str, e: SymError) -> Error {
    match e {
        SymError::IoError(e) if e.kind() != io::ErrorKind::UnexpectedEof => Error::Io(e),
        SymError::Unsupported(what) => {
            Error::Unsupported(format!("{name} uses something this doesn't read ({what})."))
        }
        e => Error::Bad(format!("{name} doesn't decode: {e}.")),
    }
}

fn gave_up(name: &str) -> Error {
    Error::Bad(format!("{name} doesn't decode: the decoder stopped on it."))
}

const UNNAMED: &str = "unknown";

/// A short name for a codec, and whether there is a decoder for it. The
/// registry's names say the sample layout too ("pcm_s16le", "adpcm_ms"); a
/// clip only needs the family.
fn codec_name(codec: CodecType) -> (&'static str, bool) {
    let Some(known) = symphonia::default::get_codecs().get_codec(codec) else {
        let name = match codec {
            CODEC_TYPE_OPUS => "opus",
            CODEC_TYPE_WMA => "wma",
            CODEC_TYPE_MP1 => "mp1",
            CODEC_TYPE_MP2 => "mp2",
            _ => UNNAMED,
        };
        return (name, false);
    };
    let name = match known.short_name {
        "pcm_alaw" => "alaw",
        "pcm_mulaw" => "mulaw",
        s if s.starts_with("pcm_") => "pcm",
        s if s.starts_with("adpcm_") => "adpcm",
        s => s,
    };
    (name, true)
}
