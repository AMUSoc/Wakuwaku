const assert = require('node:assert/strict')
const crypto = require('node:crypto')
const { test } = require('node:test')

const QR = require('../src/shared/qr')

const URL = 'http://100.90.183.86:47213/mobile/?t=0123456789abcdef0123456789abcdef'
const bits = m => m.map(r => r.map(Number).join('')).join('\n')

// The finder at a corner: a dark ring, a light ring, a dark 3x3.
function finderAt(m, x0, y0) {
  for (let y = 0; y < 7; y++) {
    for (let x = 0; x < 7; x++) {
      const d = Math.max(Math.abs(x - 3), Math.abs(y - 3))
      if (m[y0 + y][x0 + x] !== (d !== 2)) return false
    }
  }
  return true
}

test('the smallest version that holds the text, square, with its three finders and the dark module', () => {
  for (const [text, size] of [['hi', 21], [URL, 37], ['x'.repeat(150), 49], ['z'.repeat(200), 57]]) {
    const m = QR.encode(text)
    assert.equal(m.length, size, text.length)
    assert.ok(m.every(r => r.length === size))
    assert.ok(finderAt(m, 0, 0) && finderAt(m, size - 7, 0) && finderAt(m, 0, size - 7))
    assert.equal(m[size - 8][8], true)
    // The timing rows between the finders.
    for (let i = 8; i < size - 8; i++) assert.equal(m[6][i], i % 2 === 0)
  }
})

// Checked module by module against segno (every mask, versions 1 to 10)
// and read back by OpenCV's decoder, 2026-10-17; kept so it stays so.
test('the phone page address comes out as it was checked', () => {
  const hash = crypto.createHash('sha256').update(bits(QR.encode(URL))).digest('hex')
  assert.equal(hash, '5c38a760c6f7fb8062f8a97a81114d06dd23456dfa69b6cd9e67a3e8ccca782e')
})

test('every mask draws the same text differently, and the same each time', () => {
  const all = new Set([...Array(8).keys()].map(mask => bits(QR.encode(URL, mask))))
  assert.equal(all.size, 8)
  assert.equal(bits(QR.encode(URL)), bits(QR.encode(URL)))
})

test('too long for version 10 is said, not drawn wrong', () => {
  assert.throws(() => QR.encode('a'.repeat(214)))
  assert.doesNotThrow(() => QR.encode('a'.repeat(213)))
})

test('as an SVG: a quiet zone round it, the dark modules as one path', () => {
  const svg = QR.svg('hi')
  assert.match(svg, /^<svg [^>]*viewBox="0 0 29 29"/)
  assert.equal((svg.match(/<path /g) || []).length, 1)
})
