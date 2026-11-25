# Technical plan: Speaker labels (You / Them)

Status: Approved (Kush: "Do whatever needs to be done", 2026-11-23)
Date: 2026-11-23
Branch: kush/speaker-labels
Spec: `2026-11-23-speaker-labels.md`

## Approach

Label each Whisper segment by comparing loudness of the two capture lanes over the segment's time span.
No new dependency (`hound` is already used to read the WAVs).

### `app/src-tauri/src/speaker.rs` (new)

- `Speaker { You, Them }`, rendered as `"You"` / `"Them"`.
- `frame_rms(path) -> Result<Vec<f32>, String>`: stream a lane WAV with `hound`, downmix to mono, and
  return RMS per 50 ms frame (5 cs) at the file's native rate. Streaming keeps memory at ~72k floats per
  hour instead of loading the whole lane (the full 16 kHz load is ~230 MB per lane per hour). A missing
  file is an empty vec (that lane is silent), matching `session::load_optional`.
- `classify(mic, sys, start_cs, end_cs) -> Option<Speaker>`: per frame in the span,
  - sys above `SILENCE` (-40 dBFS): mic quieter than sys → vote Them (laptop-speaker bleed reaches the
    mic quieter than the digital system lane); mic at least 12 dB louder than sys → vote You (Kush talking
    over the call); in between → no vote (could be crosstalk or loud speakers; spec says unlabelled).
  - else mic above `SILENCE` → vote You.
  - else silent, no vote.
  - Result: the label with at least `AGREE` (70%) of votes, given at least `MIN_VOTES` (3 frames = 150 ms).
    Otherwise `None` (unlabelled). Whisper's timestamps are loose; voting over frames tolerates that.
- `attribute(dir, segment, &[Segment]) -> Vec<Option<Speaker>>`: load both lanes' frames via
  `session::segment_paths`; on any read error log it and return all `None` (transcript is written
  unlabelled, never blocked). Logs `[oatmeal] speakers: N You, N Them, N unlabelled`.

Thresholds are constants and will be tuned on Kush's 3 recordings (on the other Mac).

### `session.rs`

- `finish` and `finish_from_blocks` call `speaker::attribute(dir, segment, &transcript.segments)` before
  writing. Segment times are relative to the take's own lane files, so labels are computed before the
  continuation offset is applied.
- `write_transcript_md` takes `labels: &[Option<Speaker>]` and writes `**[M:SS · You]** text` when a
  label is present, `**[M:SS]** text` otherwise. `Segment` itself is unchanged (it is built in many places
  and serialized to the live panel).

### UI (`app/src`)

- Move `groupTurns` / `atSecs` from `app.js` into `shared.js` (exported) so they can be unit-tested.
- `groupTurns` also starts a new paragraph when the speaker changes; turns carry `speaker`.
- `renderTranscript` puts the speaker name in the existing `.who` header next to the time.
- Onboarding copy in `index.html` ("won't know who is speaking") changes to say it tells you apart from
  the other side.

### Tests

- Rust unit tests in `speaker.rs`: the spec's normal, edge (empty sys lane, equal loudness, bleed), and
  failure cases using synthetic WAVs in a temp dir; `session.rs` test that a continued take is labelled
  from its own lane files and that an unreadable lane writes an unlabelled transcript.
- JS test `shared.test.mjs`: unlabelled lines group exactly as before (control), and a speaker change
  breaks a paragraph.
- Existing `library.rs` `transcript_lines` tests must still pass.

## Not changing

Live panel, `library.rs` parsing, `strip_transcript_markup`, the AI note pipeline, settings.
