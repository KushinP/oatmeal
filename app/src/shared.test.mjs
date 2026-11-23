import test from 'node:test'
import assert from 'node:assert/strict'
import { atSecs, groupTurns } from './shared.js'

const line = (at, text, speaker) => (speaker ? { at, text, speaker } : { at, text, speaker: null })

test('unlabelled lines group by time exactly as before speakers', () => {
  const lines = [line('0:00', 'Bye.'), line('0:04', 'Alright.'), line('0:29', 'So again.'), line('0:31', 'Next.')]
  const turns = groupTurns(lines, 30)
  assert.deepEqual(
    turns.map((t) => t.lines.map((l) => l.text)),
    [['Bye.', 'Alright.', 'So again.'], ['Next.']],
  )
  assert.ok(turns.every((t) => t.speaker === null))
})

test('a change of speaker starts a new paragraph', () => {
  const lines = [line('0:00', 'Morning.', 'You'), line('0:02', 'Hi.', 'Them'), line('0:03', 'How are you?', 'Them')]
  const turns = groupTurns(lines, 30)
  assert.deepEqual(
    turns.map((t) => [t.speaker, t.lines.length]),
    [
      ['You', 1],
      ['Them', 2],
    ],
  )
})

test('one speaker still breaks once the paragraph runs past the limit', () => {
  const lines = [line('0:00', 'a', 'Them'), line('0:20', 'b', 'Them'), line('0:31', 'c', 'Them')]
  assert.equal(groupTurns(lines, 30).length, 2)
})

test('an unlabelled line between two of the same speaker splits them', () => {
  const lines = [line('0:00', 'a', 'You'), line('0:01', 'mm'), line('0:02', 'b', 'You')]
  assert.deepEqual(
    groupTurns(lines, 30).map((t) => t.speaker),
    ['You', null, 'You'],
  )
})

test('timestamps past an hour read as seconds', () => {
  assert.equal(atSecs('1:01:16'), 3676)
  assert.equal(atSecs('2:05'), 125)
})
