//! The command line (`docs/ENGINE.md` 1).

use crate::media::MAX_BLOCK;
use std::path::PathBuf;

pub const USAGE: &str = "usage: wwav-engine --socket <path> --shm <name> [--device <name|null>]
                   [--rate 48000] [--block 128] [--cache <dir>] [--test]

The app starts the engine with a pipe on its stdin; when the pipe closes, the
engine stops its audio and exits. See docs/ENGINE.md.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub socket: PathBuf,
    pub shm: String,
    /// A device's name, "null" for the timer, or `None` for the system's
    /// default output.
    pub device: Option<String>,
    pub rate: u32,
    pub block: u32,
    /// Where files decoded or resampled for a session are kept.
    pub cache: Option<PathBuf>,
    pub test: bool,
}

pub fn parse(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut socket, mut shm) = (None, None);
    let mut args = Args {
        socket: PathBuf::new(),
        shm: String::new(),
        device: None,
        rate: 48000,
        block: 128,
        cache: None,
        test: false,
    };
    let number =
        |v: String, lo: u32, hi: u32| v.parse::<u32>().ok().filter(|n| (lo..=hi).contains(n));
    while let Some(flag) = it.next() {
        if flag == "--test" {
            args.test = true;
            continue;
        }
        if !matches!(
            flag.as_str(),
            "--socket" | "--shm" | "--device" | "--rate" | "--block" | "--cache"
        ) {
            return Err(format!("Unknown flag {flag}."));
        }
        let value = it.next().ok_or(format!("{flag} needs a value."))?;
        match flag.as_str() {
            "--socket" => socket = Some(PathBuf::from(value)),
            "--shm" => shm = Some(value),
            "--device" => args.device = Some(value),
            "--cache" => args.cache = Some(PathBuf::from(value)),
            "--rate" => {
                args.rate = number(value, 8000, 384_000)
                    .ok_or("--rate is a sample rate from 8000 to 384000.")?
            }
            _ => {
                args.block = number(value, 16, MAX_BLOCK as u32)
                    .ok_or(format!("--block is from 16 to {MAX_BLOCK} frames."))?
            }
        }
    }
    args.socket = socket.ok_or("--socket and --shm are required.")?;
    args.shm = shm.ok_or("--socket and --shm are required.")?;
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Result<Args, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn flags_parse_with_the_contracts_defaults() {
        let a = args("--socket /tmp/e.sock --shm /wwav-1-0").unwrap();
        assert_eq!(
            (a.device, a.rate, a.block, a.test),
            (None, 48000, 128, false)
        );
        let a = args("--socket /s --shm /m --device null --rate 44100 --block 256 --test").unwrap();
        assert_eq!(
            (a.device.as_deref(), a.rate, a.block, a.test),
            (Some("null"), 44100, 256, true)
        );
        assert!(args("--shm /m").is_err());
        assert!(args("--socket /s --shm /m --rate fast").is_err());
        assert!(args("--socket /s --shm /m --block 8").is_err());
        assert!(args("--socket /s --shm /m --colour blue").is_err());
    }
}
