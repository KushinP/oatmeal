//! Who said it: you, or the other side of the call.
//!
//! The mic and the computer's audio are recorded into separate lanes and only
//! mixed for Whisper, so the recording already knows which side every sound came
//! from. A line is labelled by comparing how loud each lane was while it was
//! being said — no speaker model, no download, and it costs milliseconds.
//!
//! It cannot tell two people on the same lane apart: everyone on the call is
//! "Them", everyone near the mic is "You".

use std::path::Path;

use crate::session::segment_paths;
use crate::transcribe::Segment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    You,
    Them,
}

impl Speaker {
    pub fn label(self) -> &'static str {
        match self {
            Speaker::You => "You",
            Speaker::Them => "Them",
        }
    }
}

/// Loudness is measured over 50 ms frames, i.e. 5 centiseconds — the unit the
/// transcript's timestamps are in.
const FRAME_CS: i64 = 5;

/// RMS below this (-40 dBFS) is room tone, not speech.
const SILENCE: f32 = 0.01;

/// When the call is playing, a frame still counts as you if the mic is this much
/// louder (12 dB). Without headphones the call plays into the mic too, but it
/// arrives there quieter than the digital system lane, so a mic quieter than the
/// call is the call. Between the two (mic level with the call, up to 12 dB above
/// it) it could be either — crosstalk, or loud speakers — and the frame does not
/// vote.
const OVERRIDE_RATIO: f32 = 4.0;

/// Share of voting frames that must agree before a line gets a label. Below it
/// the line is crosstalk or too short to call, and stays unlabelled.
const AGREE: f32 = 0.7;

/// A line needs at least this many voting frames (150 ms) to be labelled.
const MIN_VOTES: usize = 3;

/// RMS per 50 ms frame of a lane WAV, at the file's own sample rate.
///
/// Streams the file rather than loading it: an hour of one lane at 16 kHz is
/// ~230 MB of f32, and this only needs 72k numbers. A missing file is a silent
/// lane, the same as `session::load_optional` treats it.
pub fn frame_rms(path: &Path) -> Result<Vec<f32>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("open wav {}: {e}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let frame_len = ((spec.sample_rate as i64 * FRAME_CS) / 100).max(1) as usize;

    let mut out = Vec::new();
    let mut acc = 0f64;
    let mut n = 0usize;
    let mut chan = 0usize;
    let mut mono = 0f32;
    let mut push = |v: f32| {
        mono += v;
        chan += 1;
        if chan == channels {
            let s = (mono / channels as f32) as f64;
            acc += s * s;
            n += 1;
            mono = 0.0;
            chan = 0;
            if n == frame_len {
                out.push((acc / n as f64).sqrt() as f32);
                acc = 0.0;
                n = 0;
            }
        }
    };
    match spec.sample_format {
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample - 1)) as f32;
            for s in reader.samples::<i32>() {
                push(s.map_err(|e| e.to_string())? as f32 / max);
            }
        }
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                push(s.map_err(|e| e.to_string())?);
            }
        }
    }
    if n > 0 {
        out.push((acc / n as f64).sqrt() as f32);
    }
    Ok(out)
}

/// Who said the line spanning `start_cs..end_cs`, or `None` when it can't be
/// told: nobody audible, too short, or both sides talking at once.
pub fn classify(mic: &[f32], sys: &[f32], start_cs: i64, end_cs: i64) -> Option<Speaker> {
    let first = (start_cs.max(0) / FRAME_CS) as usize;
    let last = ((end_cs.max(start_cs) + FRAME_CS - 1) / FRAME_CS) as usize;
    let (mut you, mut them) = (0usize, 0usize);
    for i in first..last.max(first + 1) {
        let m = mic.get(i).copied().unwrap_or(0.0);
        let s = sys.get(i).copied().unwrap_or(0.0);
        if s >= SILENCE {
            if m >= s * OVERRIDE_RATIO {
                you += 1;
            } else if m < s {
                them += 1;
            }
        } else if m >= SILENCE {
            you += 1;
        }
    }
    let votes = you + them;
    if votes < MIN_VOTES {
        return None;
    }
    if you as f32 >= votes as f32 * AGREE {
        Some(Speaker::You)
    } else if them as f32 >= votes as f32 * AGREE {
        Some(Speaker::Them)
    } else {
        None
    }
}

/// Label every segment of take `segment` from that take's own lane files.
///
/// Segment times must be relative to the start of the take (before any
/// continuation offset). Never fails: if a lane can't be read, every line comes
/// back unlabelled and the transcript is written exactly as it was before
/// speaker labels existed.
pub fn attribute(dir: &Path, segment: u32, segments: &[Segment]) -> Vec<Option<Speaker>> {
    let (mic_path, sys_path) = segment_paths(dir, segment);
    let lanes = frame_rms(&mic_path).and_then(|m| frame_rms(&sys_path).map(|s| (m, s)));
    let (mic, sys) = match lanes {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[oatmeal] speakers: skipped, {e}");
            return vec![None; segments.len()];
        }
    };
    let labels: Vec<Option<Speaker>> = segments
        .iter()
        .map(|s| classify(&mic, &sys, s.start_cs, s.end_cs))
        .collect();
    let count = |want: Option<Speaker>| labels.iter().filter(|l| **l == want).count();
    eprintln!(
        "[oatmeal] speakers: {} You, {} Them, {} unlabelled",
        count(Some(Speaker::You)),
        count(Some(Speaker::Them)),
        count(None)
    );
    labels
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;

    const RATE: u32 = 16_000;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oatmeal-speaker-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A mono 16-bit WAV of `secs` seconds, a 220 Hz tone at `amp` inside each
    /// `(from, to)` second range and silence elsewhere.
    pub(crate) fn wav(path: &Path, secs: f32, tone: &[(f32, f32, f32)]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..(secs * RATE as f32) as u32 {
            let t = i as f32 / RATE as f32;
            let amp = tone
                .iter()
                .find(|(a, b, _)| t >= *a && t < *b)
                .map(|(_, _, amp)| *amp)
                .unwrap_or(0.0);
            let v = amp * (t * 220.0 * std::f32::consts::TAU).sin();
            w.write_sample((v * i16::MAX as f32) as i16).unwrap();
        }
        w.finalize().unwrap();
    }

    fn seg(from_s: f32, to_s: f32) -> Segment {
        Segment {
            start_cs: (from_s * 100.0) as i64,
            end_cs: (to_s * 100.0) as i64,
            text: "x".into(),
        }
    }

    #[test]
    fn mic_then_call_is_you_then_them() {
        let dir = tmp("normal");
        wav(&dir.join("mic.wav"), 10.0, &[(0.0, 5.0, 0.3)]);
        wav(&dir.join("system.wav"), 10.0, &[(5.0, 10.0, 0.3)]);
        let labels = attribute(&dir, 1, &[seg(0.0, 5.0), seg(5.0, 10.0)]);
        assert_eq!(labels, vec![Some(Speaker::You), Some(Speaker::Them)]);
    }

    #[test]
    fn no_system_lane_means_every_line_is_you() {
        let dir = tmp("nosys");
        wav(&dir.join("mic.wav"), 6.0, &[(0.0, 6.0, 0.3)]);
        let labels = attribute(&dir, 1, &[seg(0.0, 2.0), seg(2.0, 4.0), seg(4.0, 6.0)]);
        assert!(labels.iter().all(|l| *l == Some(Speaker::You)), "{labels:?}");
    }

    #[test]
    fn only_system_audio_is_all_them() {
        let dir = tmp("onlysys");
        wav(&dir.join("system.wav"), 6.0, &[(0.0, 6.0, 0.3)]);
        let labels = attribute(&dir, 1, &[seg(0.0, 3.0), seg(3.0, 6.0)]);
        assert!(labels.iter().all(|l| *l == Some(Speaker::Them)), "{labels:?}");
    }

    #[test]
    fn call_bleeding_into_the_mic_is_still_them() {
        // Laptop speakers: the remote voice reaches the mic, quieter.
        let dir = tmp("bleed");
        wav(&dir.join("mic.wav"), 4.0, &[(0.0, 4.0, 0.15)]);
        wav(&dir.join("system.wav"), 4.0, &[(0.0, 4.0, 0.3)]);
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 4.0)]), vec![Some(Speaker::Them)]);
    }

    #[test]
    fn talking_over_the_call_is_you() {
        let dir = tmp("over");
        wav(&dir.join("mic.wav"), 4.0, &[(0.0, 4.0, 0.6)]);
        wav(&dir.join("system.wav"), 4.0, &[(0.0, 4.0, 0.05)]);
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 4.0)]), vec![Some(Speaker::You)]);
    }

    #[test]
    fn equal_loudness_on_both_lanes_is_unlabelled() {
        let dir = tmp("equal");
        wav(&dir.join("mic.wav"), 4.0, &[(0.0, 4.0, 0.3)]);
        wav(&dir.join("system.wav"), 4.0, &[(0.0, 4.0, 0.3)]);
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 4.0)]), vec![None]);
    }

    #[test]
    fn half_and_half_is_unlabelled() {
        let dir = tmp("cross");
        wav(&dir.join("mic.wav"), 4.0, &[(0.0, 2.0, 0.3)]);
        wav(&dir.join("system.wav"), 4.0, &[(2.0, 4.0, 0.3)]);
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 4.0)]), vec![None]);
    }

    #[test]
    fn silence_and_blips_are_unlabelled() {
        let dir = tmp("quiet");
        wav(&dir.join("mic.wav"), 4.0, &[(1.0, 1.05, 0.3)]);
        wav(&dir.join("system.wav"), 4.0, &[]);
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 1.0), seg(1.0, 2.0)]), vec![None, None]);
    }

    #[test]
    fn a_continuation_reads_its_own_lanes() {
        let dir = tmp("cont");
        wav(&dir.join("mic.wav"), 4.0, &[(0.0, 4.0, 0.3)]);
        wav(&dir.join("system.002.wav"), 4.0, &[(0.0, 4.0, 0.3)]);
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 4.0)]), vec![Some(Speaker::You)]);
        assert_eq!(attribute(&dir, 2, &[seg(0.0, 4.0)]), vec![Some(Speaker::Them)]);
    }

    #[test]
    fn an_unreadable_lane_leaves_everything_unlabelled() {
        let dir = tmp("broken");
        wav(&dir.join("mic.wav"), 4.0, &[(0.0, 4.0, 0.3)]);
        std::fs::write(dir.join("system.wav"), b"not a wav").unwrap();
        assert_eq!(attribute(&dir, 1, &[seg(0.0, 2.0), seg(2.0, 4.0)]), vec![None, None]);
    }

    #[test]
    fn stereo_and_float_lanes_are_read() {
        let dir = tmp("stereo");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(dir.join("system.wav"), spec).unwrap();
        for i in 0..48_000 * 2 {
            let v = 0.3 * (i as f32 / 48_000.0 * 220.0 * std::f32::consts::TAU).sin();
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        let frames = frame_rms(&dir.join("system.wav")).unwrap();
        assert_eq!(frames.len(), 40);
        assert!(frames.iter().all(|r| (*r - 0.212).abs() < 0.01), "{frames:?}");
    }
}
