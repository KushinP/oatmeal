# Spec: Speaker labels (You / Them)

Status: Approved
Date: 2026-11-23
Branch: kush/speaker-labels

Context: see `HANDOFF-speaker-labels.md`. The earlier speaker-model version (`ee569fc`) was removed in
`69df8db` together with the paragraph-grouping change; the commit gives no technical reason and there are
no upstream issues about it.

## 1. Problem

- Every transcript reads as one voice. In Kush's words, the app "can't tell people apart." He can't tell
  what he said from what the other people said.
- Felt on every call. The cost is rereading to work out who said what, and the AI notes can't say "they
  asked for X". Granola already names the other person on 1-on-1 calls.
- Evidence: Kush's own complaint. We haven't measured how often labels would be wrong.

## 2. Outcome

- Every transcript line says whether Kush said it or the other side did, and paragraphs break when the
  speaker changes.
- **Number:** share of lines labelled correctly on 3 real recordings labelled by hand. Target: at least
  95% for You vs Them.
- **Question for Kush:** after a week, do you still have to reread to work out who said something?

## 3. Scope

**In:** label each line **You** or **Them** by comparing how loud the mic is with how loud the computer
audio is during that line. Labels are added automatically when a meeting stops. The transcript view shows
the label and starts a new paragraph when it changes.

- No new library, no model download, milliseconds rather than the old pass's ~12 minutes per hour.
- It works because the app already records the mic (`mic.wav`) and computer audio (`system.wav`) to
  separate files and only mixes them for Whisper.

**Out:**

- Telling several remote people apart ("Speaker 2", "Speaker 3"): needs the speaker-model library back (a
  new dependency) and the slow pass that probably got it removed.
- Labels in the live panel during the meeting: its timing is a separate system; stopping is when labels
  get added.
- Names from calendar invites: needs new EventKit code and a permissions check.
- Telling apart several people in one room on the mic: one mic can't do that without the speaker model.

**Later:**

- Speaker 2/3 on the computer-audio side only, reusing the speaker-model code from `ee569fc`.
- Naming the other person from the invite on 1-on-1 calls, as Granola does.
- Rename a speaker once and apply it to the whole meeting.
- A "label past meetings" action (their audio is still on disk).

## 4. How it works

1. The meeting stops. `session::finish` or `finish_from_blocks` produces the timed segments as today.
2. **New:** `speaker::attribute(mic, sys, segments)`. For each segment, measure loudness (RMS) on each lane
   over the segment's span. **Them** when the computer audio is clearly louder, **You** when the mic is
   clearly louder, unlabelled when too close to call or both silent. `finish_from_blocks` does not load
   the lane WAVs today, so it will need to.
3. `write_transcript_md` writes `**[0:12 · You]** text`. `library.rs` `transcript_lines` already parses
   this, and `strip_transcript_markup` already keeps the label away from the AI and search.
4. UI: `app.js` `renderTranscript` / `groupTurns` show the label and start a new paragraph when it changes.

**Data:** no database. The only format change is the optional `· Label` inside the existing timestamp
marker, which old builds already parse.

**Files:** new `app/src-tauri/src/speaker.rs` (with unit tests). Changed: `session.rs`, `lib.rs` (module
only), `app.js`, possibly `base.css`.

## 5. Edge cases and failure modes

- **One lane missing** (permission denied, no computer audio): all You, or all Them. Don't guess the
  missing side.
- **No headphones** (call audio bleeds into the mic): the remote voice also reaches the mic, more quietly.
  The comparison must weigh the computer audio more. Most likely to be wrong; one acceptance recording must
  be made this way.
- **Crosstalk within one segment:** leave it unlabelled.
- **Continued meetings:** each take has its own lane files; the timing offset must be right.
- **Silent failure:** labels look plausible but are all "You" because audio and segments are misaligned.
  Catch it with a per-meeting log line of You/Them/unlabelled counts, and a test that a recording with only
  computer audio comes out 100% Them.
- **If labelling errors:** write the transcript without labels, exactly as today. Labelling must never
  block or lose a transcript.

## 6. Tests

Automated (`cargo test --lib`):

- Normal: given mic speech at 0–5 s and computer-audio speech at 5–10 s, when two segments span those
  times, then the labels are You and Them.
- Edge: given an empty computer-audio lane, then every line is You.
- Edge: given equal loudness on both lanes in one segment, then that segment is unlabelled.
- Edge: given a continued meeting (take 2 with an offset), then take 2's lines are labelled from take 2's
  lane files.
- Failure: given an unreadable lane file, then the transcript is written without labels and finishing
  succeeds.
- Control: given an old transcript with no labels, it renders and groups exactly as today (`npm test`),
  and the existing `transcript_lines` tests still pass.

By hand (Kush): 3 real recordings (1-on-1 with headphones, 1-on-1 on laptop speakers, a group call),
labelled by hand; check at least 95%.

## 7. Checkpoints

- Installing Rust (`rustup`) on this Mac before anything builds.
- No new library in this version. Speaker 2/3 later would need approval.
- No database changes, logins, money, messages, or deploys.

## 8. Risks and undo

- Wrong labels make transcripts look worse than none. Mitigation: too-close-to-call lines stay
  unlabelled; possibly a setting to turn labels off.
- Paragraphs break more often, changing how long transcripts read.
- Undo: revert the branch. Labelled transcripts still render in old builds, which parse and ignore the
  label.

## 9. Open questions

Approved without answers; these defaults apply until Kush says otherwise:

1. You vs Them is enough for v1 (no Speaker 2/3). — default: yes.
2. The other side is labelled "Them", not a calendar name. — default: "Them".
3. Test recordings: none on this Mac. — answered: they are on Kush's other Mac; the hand check runs there.

## Changes

- 2026-11-23: Open question 3 answered (recordings are on the other Mac). Technical plan in
  `2026-11-23-speaker-labels-plan.md`.
