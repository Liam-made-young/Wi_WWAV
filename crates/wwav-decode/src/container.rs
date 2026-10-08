//! What a file's container says that symphonia 0.5's readers don't pass on.
//!
//! An AIFF's `COMM` chunk counts its frames; symphonia counts them from the
//! sound chunk's length, 8 bytes too long, and reads that far, so a chunk
//! after the sound comes out as a click. An MP4's edit list says how much
//! encoder delay an AAC track starts with and where its sound ends;
//! symphonia reads the list and doesn't use it. An Ogg stream's last page
//! says where its sound ends; symphonia reports that length and, when the
//! sound fits in one page (a second or two), decodes past it to the end of
//! the last block.
//!
//! Anything odd here means "the container doesn't say", never an error: the
//! file still decodes, as symphonia alone would have it.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

pub(crate) enum Container {
    /// An AIFF, and the frame count in its `COMM` chunk.
    Aiff {
        frames: u64,
    },
    /// An MP4, and its sound track's edit when it has one.
    Mp4 {
        edit: Option<Edit>,
    },
    Ogg,
    Other,
}

/// The part of a track that is its sound.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Edit {
    /// Frames to drop from the front.
    pub start: u64,
    /// Frames of sound after that, when the file says.
    pub frames: Option<u64>,
    /// Ticks a second for both counts: they are frames only when this is the
    /// rate the track decodes at.
    pub timescale: u32,
}

/// A `moov` box bigger than this is not read for its edit list.
const MAX_MOOV: u64 = 64 << 20;

pub(crate) fn sniff(path: &Path) -> Container {
    read(path).unwrap_or(Container::Other)
}

fn read(path: &Path) -> io::Result<Container> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut head = [0u8; 12];
    file.read_exact(&mut head)?;
    let mut at = 0;
    // An ID3v2 tag may sit in front of anything: 10 bytes, the last four its
    // length in 7-bit bytes.
    if &head[..3] == b"ID3" {
        let size = head[6..10]
            .iter()
            .fold(0u64, |n, &b| n << 7 | (b & 0x7f) as u64);
        at = 10 + size;
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut head)?;
    }
    if &head[..4] == b"FORM" && matches!(&head[8..12], b"AIFF" | b"AIFC") {
        return Ok(aiff_frames(&mut file, at + 12, len)?
            .map_or(Container::Other, |frames| Container::Aiff { frames }));
    }
    if &head[4..8] == b"ftyp" {
        let edit = moov(&mut file, at, len)?.and_then(|moov| edit(&moov));
        return Ok(Container::Mp4 { edit });
    }
    if &head[..4] == b"OggS" {
        return Ok(Container::Ogg);
    }
    Ok(Container::Other)
}

/// Walks an AIFF's chunks to `COMM`: channels, then the frame count.
fn aiff_frames(file: &mut File, mut at: u64, len: u64) -> io::Result<Option<u64>> {
    while at + 8 <= len {
        let mut head = [0u8; 8];
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut head)?;
        let size = be32(&head[4..]) as u64;
        if &head[..4] == b"COMM" {
            let mut comm = [0u8; 6];
            file.read_exact(&mut comm)?;
            return Ok(Some(be32(&comm[2..]) as u64));
        }
        at += 8 + size + (size & 1);
    }
    Ok(None)
}

/// An MP4's `moov` box, from wherever it is among the top-level boxes.
fn moov(file: &mut File, mut at: u64, len: u64) -> io::Result<Option<Vec<u8>>> {
    while at + 8 <= len {
        let mut head = [0u8; 16];
        file.seek(SeekFrom::Start(at))?;
        file.read_exact(&mut head[..8])?;
        // A size of 1 means a 64-bit size follows, and 0 the rest of the file.
        let (size, header) = match be32(&head) {
            1 => {
                file.read_exact(&mut head[8..])?;
                (u64::from_be_bytes(head[8..].try_into().unwrap()), 16)
            }
            0 => (len - at, 8),
            n => (n as u64, 8),
        };
        if size < header || at + size > len {
            break;
        }
        if &head[4..8] == b"moov" {
            if size - header > MAX_MOOV {
                break;
            }
            let mut body = vec![0u8; (size - header) as usize];
            file.read_exact(&mut body)?;
            return Ok(Some(body));
        }
        at += size;
    }
    Ok(None)
}

/// The edit of the one sound track in a `moov`. Two sound tracks, several
/// edits, or an edit that starts with a gap are left alone.
fn edit(moov: &[u8]) -> Option<Edit> {
    let movie = timescale(child(moov, b"mvhd")?)?;
    let mut sound = boxes(moov).filter(|(kind, body)| {
        *kind == b"trak"
            && child(body, b"mdia")
                .and_then(|mdia| child(mdia, b"hdlr"))
                .is_some_and(|hdlr| hdlr.get(8..12) == Some(b"soun"))
    });
    let (_, trak) = sound.next()?;
    if sound.next().is_some() {
        return None;
    }
    let media = timescale(child(child(trak, b"mdia")?, b"mdhd")?)?;
    let elst = child(child(trak, b"edts")?, b"elst")?;
    if be32(elst.get(4..8)?) != 1 {
        return None;
    }
    // Version 1 holds the same fields in 64 bits.
    let (length, start) = if *elst.first()? == 1 {
        (
            u64::from_be_bytes(elst.get(8..16)?.try_into().ok()?),
            i64::from_be_bytes(elst.get(16..24)?.try_into().ok()?),
        )
    } else {
        (
            be32(elst.get(8..12)?) as u64,
            be32(elst.get(12..16)?) as i32 as i64,
        )
    };
    Some(Edit {
        start: u64::try_from(start).ok()?,
        // The length counts in the movie's timescale. When that isn't the
        // track's it is rounded, and can't say where the last frame is.
        frames: (movie == media && length > 0).then_some(length),
        timescale: media,
    })
}

/// The boxes laid end to end in `bytes`, as (type, body).
fn boxes(bytes: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut rest = bytes;
    std::iter::from_fn(move || {
        let size = be32(rest.get(..4)?) as usize;
        let kind = rest.get(4..8)?;
        let (header, size) = match size {
            1 => (
                16,
                u64::from_be_bytes(rest.get(8..16)?.try_into().ok()?) as usize,
            ),
            0 => (8, rest.len()),
            n => (8, n),
        };
        let body = rest.get(header..size)?;
        rest = &rest[size..];
        Some((kind, body))
    })
}

fn child<'a>(bytes: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(bytes).find(|(k, _)| *k == kind).map(|(_, body)| body)
}

/// The timescale in an `mvhd` or `mdhd`: after two dates, which version 1
/// holds in 64 bits.
fn timescale(header: &[u8]) -> Option<u32> {
    let at = if *header.first()? == 1 { 20 } else { 12 };
    Some(be32(header.get(at..at + 4)?)).filter(|&t| t > 0)
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes[..4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }

    /// An `mvhd` or `mdhd` as far as its timescale.
    fn header(kind: &[u8; 4], timescale: u32) -> Vec<u8> {
        let mut body = vec![0u8; 12];
        body.extend_from_slice(&timescale.to_be_bytes());
        body.extend_from_slice(&[0; 4]);
        boxed(kind, &body)
    }

    fn hdlr(kind: &[u8; 4]) -> Vec<u8> {
        let mut body = vec![0u8; 8];
        body.extend_from_slice(kind);
        boxed(b"hdlr", &body)
    }

    fn elst(entries: &[(u32, i32)]) -> Vec<u8> {
        let mut body = vec![0u8; 4];
        body.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        for (length, start) in entries {
            body.extend_from_slice(&length.to_be_bytes());
            body.extend_from_slice(&start.to_be_bytes());
            body.extend_from_slice(&[0, 1, 0, 0]);
        }
        boxed(b"edts", &boxed(b"elst", &body))
    }

    fn trak(handler: &[u8; 4], media: u32, edits: &[(u32, i32)]) -> Vec<u8> {
        let mdia = [header(b"mdhd", media), hdlr(handler)].concat();
        boxed(b"trak", &[elst(edits), boxed(b"mdia", &mdia)].concat())
    }

    #[test]
    fn an_edit_gives_the_delay_and_the_length_in_frames() {
        let moov = [
            header(b"mvhd", 48_000),
            trak(b"vide", 600, &[(10, 0)]),
            trak(b"soun", 48_000, &[(96_123, 1024)]),
        ]
        .concat();
        let want = Edit {
            start: 1024,
            frames: Some(96_123),
            timescale: 48_000,
        };
        assert_eq!(edit(&moov), Some(want));
    }

    #[test]
    fn a_length_in_another_timescale_is_not_taken_for_frames() {
        let moov = [
            header(b"mvhd", 1000),
            trak(b"soun", 44_100, &[(2003, 2112)]),
        ]
        .concat();
        let want = Edit {
            start: 2112,
            frames: None,
            timescale: 44_100,
        };
        assert_eq!(edit(&moov), Some(want));
    }

    #[test]
    fn edits_this_cant_be_sure_of_are_left_alone() {
        let movie = header(b"mvhd", 48_000);
        let one = trak(b"soun", 48_000, &[(96_123, 1024)]);
        // Two sound tracks, a gap before the sound, and a cut-off box.
        let two = [movie.clone(), one.clone(), one.clone()].concat();
        assert_eq!(edit(&two), None);
        let gap = trak(b"soun", 48_000, &[(4800, -1), (96_123, 1024)]);
        assert_eq!(edit(&[movie.clone(), gap].concat()), None);
        let whole = [movie, one].concat();
        for cut in 0..whole.len() {
            let _ = edit(&whole[..cut]);
        }
        assert!(edit(&whole).is_some());
    }
}
