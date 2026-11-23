# Handoff: speaker labels for Oatmeal

Written 2026-11-23 so the work can continue on another computer.

## Goal

Kush's meeting app (this repo, forked from `Cujoqt/oatmeal`, installed version 1.12.0) can't tell people apart.
Goal: label who is speaking, all on the Mac, with no cloud. "Speaker identification" here means telling
different voices apart ("diarization") and putting a label such as "Speaker 2" on each line.

Kush chose this app over two alternatives:
- `VedSoni-dev/oatmeal`: a near-copy of this repo with the same You/Room split. It doesn't fix anything.
- oatmealapp.com's Oatmeal: it does have speaker labels (v0.6.0), but Kush finds its look unacceptable.

Granola (free) is what Kush uses today. On a 1-on-1 call it names the other person from the calendar
invite. It's unclear how it handles several people on the other end.

## Where things are

- Fork: https://github.com/KushinP/oatmeal (`origin`). `upstream` is `Cujoqt/oatmeal`.
- Branch: `kush/speaker-labels`. Only this file has been added so far. No app code has changed.
- Process (from Kush's global CLAUDE.md): run `/spec` in plan mode. Gate 1 is the product spec and Gate 2 the
  technical plan. Commit both to `docs/specs/` before any code. Adding the speaker-model library is a
  new dependency, so it needs approval. Finish with `/test-gate`.

## What the code does today (verified 2026-11-23)

- **Recording:** the mic and the computer's audio are recorded into **separate** files, `mic.wav` and
  `system.wav`, under `~/Library/Application Support/dev.oatmeal.app/recordings/<id>/`. They are kept
  forever, so old meetings could be labelled after the fact.
- **Transcription mixes both lanes into one track before Whisper runs** (`session.rs`, `live.rs`). So the
  desktop app doesn't even label lines You or Room. That label exists only in the old browser recorder
  under `capture/`.
- **Transcript format:** `transcript.md` stores one `**[M:SS]** text` line per segment
  (`session.rs` `write_transcript_md`). The parser already accepts `**[0:12 · Speaker 2]** text`
  (`library.rs` `transcript_lines`), and `strip_transcript_markup` keeps labels away from the AI and from search.
- **The UI ignores the `speaker` field.** `app.js` `renderTranscript`/`groupTurns` and `transcript.js` group
  lines into paragraphs by time only.
- **Calendar** (`apple_calendar.rs`) does not read attendees, so naming speakers would need new EventKit
  code. The `EKParticipant` Cargo feature is already enabled.
- **Tests:** `npm test` (JS), plus `cargo test --lib` in `app/src-tauri`. The end-to-end tests in
  `app/src-tauri/tests/` are `#[ignore]` and download models.

## Important history: this was built once, then removed

- `ee569fc` (2026-08-18, v1.9.0) added speaker labels. It used `sherpa-rs` (sherpa-onnx), pyannote
  segmentation and TitaNet embeddings, about 46 MB of models downloaded on first use, in `diarize.rs`
  (261 lines). Users started it with an "identify speakers" button. It ran at about one fifth of real
  time (roughly 12 minutes for an hour-long meeting), then rewrote `transcript.md` with
  `· Speaker N` labels.
- `69df8db` (the next day) removed all of it: the chip, the four commands, `diarize.rs` and the model
  download. **It gives no reason.** Before reviving it, find out why. Check the upstream issues and PRs,
  or ask the author. Possible reasons: poor accuracy, slowness, or a build conflict between `sherpa-rs`
  and the `ggml` copy that `whisper-rs` and `llama-cpp-2` share (see the comment on the pinned
  `llama-cpp-2 = "=0.1.141"` in `Cargo.toml`).
- Look at it with `git show ee569fc:app/src-tauri/src/diarize.rs` and
  `git show ee569fc:app/src-tauri/tests/e2e_diarize.rs`.

## Product questions for the spec (Gate 1)

1. Label only the other side (the system-audio lane) and always call the mic "You"? That is probably
   more accurate than diarizing the mix.
2. Label automatically when a meeting stops, or with a button? Stopping is already slow.
3. Generic "Speaker 2", or names? Names could come from calendar attendees, or from Kush renaming
   a speaker once in a meeting.
4. Label past meetings? Their audio is still on disk.
5. What does "good enough" mean? Pick 2 or 3 real multi-person recordings as acceptance tests.

## Setup needed on the desktop Mac

- Xcode (full app, not just the command-line tools), Node 20+, and **Rust** (`rustup`). Rust is not
  installed on the laptop either.
- Then: `cd app && npm ci && npm run tauri dev` (check `app/README.md` for the exact commands).
- macOS will ask the dev build for Microphone and Screen & System Audio Recording permission.

## Next step

On the desktop: clone the fork, `git switch kush/speaker-labels`, open Claude Code in the folder, and say
"read docs/specs/HANDOFF-speaker-labels.md and start /spec".
