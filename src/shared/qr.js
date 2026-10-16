// A QR code of a short text (the phone page's address, settings.js): byte
// mode, error correction level M, versions 1 to 10 (up to 213 bytes). Loaded
// by the page as a script (window.QR) and required by the tests.
//
//   QR.encode(text, mask?)  rows of booleans (true: dark), size 21 to 57
//   QR.svg(text)            the same as an SVG, a quiet zone of 4 round it
;(function (root) {
  // Level M, by version: error correction codewords per block, and blocks.
  const ECC_PER_BLOCK = [0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26]
  const BLOCKS = [0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5]
  const MAX_VERSION = 10

  // The modules a version has for data and its correction, in bits.
  function rawModules(ver) {
    let n = (16 * ver + 128) * ver + 64
    if (ver >= 2) {
      const align = Math.floor(ver / 7) + 2
      n -= (25 * align - 10) * align - 55
      if (ver >= 7) n -= 36
    }
    return n
  }

  const dataCodewords = ver => Math.floor(rawModules(ver) / 8) - ECC_PER_BLOCK[ver] * BLOCKS[ver]

  // --- Reed-Solomon over GF(256), x^8 + x^4 + x^3 + x^2 + 1 ---------------------

  function mul(x, y) {
    let z = 0
    for (let i = 7; i >= 0; i--) {
      z = (z << 1) ^ ((z >>> 7) * 0x11d)
      z ^= ((y >>> i) & 1) * x
    }
    return z
  }

  function divisor(degree) {
    const out = new Array(degree).fill(0)
    out[degree - 1] = 1
    let r = 1
    for (let i = 0; i < degree; i++) {
      for (let j = 0; j < out.length; j++) {
        out[j] = mul(out[j], r)
        if (j + 1 < out.length) out[j] ^= out[j + 1]
      }
      r = mul(r, 0x02)
    }
    return out
  }

  function remainder(data, div) {
    const out = div.map(() => 0)
    for (const b of data) {
      const factor = b ^ out.shift()
      out.push(0)
      div.forEach((coef, i) => (out[i] ^= mul(coef, factor)))
    }
    return out
  }

  // --- The codewords --------------------------------------------------------------

  function utf8(text) {
    return Array.from(new TextEncoder().encode(String(text)))
  }

  // The data, padded, split into blocks with their correction, interleaved.
  function codewords(bytes, ver) {
    const bits = []
    const put = (value, n) => {
      for (let i = n - 1; i >= 0; i--) bits.push((value >>> i) & 1)
    }
    put(0b0100, 4)
    put(bytes.length, ver <= 9 ? 8 : 16)
    for (const b of bytes) put(b, 8)
    const capacity = dataCodewords(ver) * 8
    put(0, Math.min(4, capacity - bits.length))
    put(0, (8 - (bits.length % 8)) % 8)
    for (let pad = 0xec; bits.length < capacity; pad ^= 0xec ^ 0x11) put(pad, 8)
    const data = []
    for (let i = 0; i < bits.length; i += 8) data.push(bits.slice(i, i + 8).reduce((a, b) => (a << 1) | b, 0))

    const blocks = BLOCKS[ver]
    const eccLen = ECC_PER_BLOCK[ver]
    const raw = Math.floor(rawModules(ver) / 8)
    const short = blocks - (raw % blocks)
    const shortLen = Math.floor(raw / blocks)
    const div = divisor(eccLen)
    const all = []
    for (let i = 0, k = 0; i < blocks; i++) {
      const dat = data.slice(k, k + shortLen - eccLen + (i < short ? 0 : 1))
      k += dat.length
      const ecc = remainder(dat, div)
      if (i < short) dat.push(0)
      all.push(dat.concat(ecc))
    }
    const out = []
    for (let i = 0; i < all[0].length; i++) {
      all.forEach((block, j) => {
        if (i !== shortLen - eccLen || j >= short) out.push(block[i])
      })
    }
    return out
  }

  // --- The matrix -----------------------------------------------------------------

  function alignments(ver) {
    if (ver === 1) return []
    const n = Math.floor(ver / 7) + 2
    const step = Math.ceil((ver * 4 + 4) / (n * 2 - 2)) * 2
    const out = [6]
    for (let pos = ver * 4 + 10; out.length < n; pos -= step) out.splice(1, 0, pos)
    return out
  }

  const MASKS = [
    (x, y) => (x + y) % 2 === 0,
    (x, y) => y % 2 === 0,
    (x, y) => x % 3 === 0,
    (x, y) => (x + y) % 3 === 0,
    (x, y) => (Math.floor(x / 3) + Math.floor(y / 2)) % 2 === 0,
    (x, y) => ((x * y) % 2) + ((x * y) % 3) === 0,
    (x, y) => (((x * y) % 2) + ((x * y) % 3)) % 2 === 0,
    (x, y) => (((x + y) % 2) + ((x * y) % 3)) % 2 === 0,
  ]

  function draw(ver, data, mask) {
    const size = ver * 4 + 17
    const dark = Array.from({ length: size }, () => new Array(size).fill(false))
    const fixed = Array.from({ length: size }, () => new Array(size).fill(false))
    const set = (x, y, on) => {
      dark[y][x] = on
      fixed[y][x] = true
    }
    for (let i = 0; i < size; i++) {
      set(6, i, i % 2 === 0)
      set(i, 6, i % 2 === 0)
    }
    for (const [cx, cy] of [[3, 3], [size - 4, 3], [3, size - 4]]) {
      for (let dy = -4; dy <= 4; dy++) {
        for (let dx = -4; dx <= 4; dx++) {
          const d = Math.max(Math.abs(dx), Math.abs(dy))
          const x = cx + dx
          const y = cy + dy
          if (x >= 0 && x < size && y >= 0 && y < size) set(x, y, d !== 2 && d !== 4)
        }
      }
    }
    const at = alignments(ver)
    const last = at.length - 1
    at.forEach((ax, i) =>
      at.forEach((ay, j) => {
        if ((i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0)) return
        for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) set(ax + dx, ay + dy, Math.max(Math.abs(dx), Math.abs(dy)) !== 1)
      }),
    )
    // Format: level M (00) and the mask, with its BCH bits.
    const format = mask
    let rem = format
    for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537)
    const fbits = ((format << 10) | rem) ^ 0x5412
    const fb = i => ((fbits >>> i) & 1) !== 0
    for (let i = 0; i <= 5; i++) set(8, i, fb(i))
    set(8, 7, fb(6))
    set(8, 8, fb(7))
    set(7, 8, fb(8))
    for (let i = 9; i < 15; i++) set(14 - i, 8, fb(i))
    for (let i = 0; i < 8; i++) set(size - 1 - i, 8, fb(i))
    for (let i = 8; i < 15; i++) set(8, size - 15 + i, fb(i))
    set(8, size - 8, true)
    if (ver >= 7) {
      let r = ver
      for (let i = 0; i < 12; i++) r = (r << 1) ^ ((r >>> 11) * 0x1f25)
      const vbits = (ver << 12) | r
      for (let i = 0; i < 18; i++) {
        const on = ((vbits >>> i) & 1) !== 0
        const a = size - 11 + (i % 3)
        const b = Math.floor(i / 3)
        set(a, b, on)
        set(b, a, on)
      }
    }
    // The data, two columns at a time from the right, up and down by turns.
    let i = 0
    for (let right = size - 1; right >= 1; right -= 2) {
      if (right === 6) right = 5
      for (let v = 0; v < size; v++) {
        for (let j = 0; j < 2; j++) {
          const x = right - j
          const y = ((right + 1) & 2) === 0 ? size - 1 - v : v
          if (fixed[y][x]) continue
          dark[y][x] = i < data.length * 8 && ((data[i >>> 3] >>> (7 - (i & 7))) & 1) !== 0
          i++
        }
      }
    }
    for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) if (!fixed[y][x] && MASKS[mask](x, y)) dark[y][x] = !dark[y][x]
    return dark
  }

  // How hard a matrix is to read (the standard's four rules): the mask with
  // the least is used.
  function penalty(m) {
    const size = m.length
    let score = 0
    const lines = []
    for (let i = 0; i < size; i++) {
      lines.push(m[i])
      lines.push(m.map(row => row[i]))
    }
    for (const line of lines) {
      for (let i = 0, run = 1; i < size; i++) {
        if (i + 1 < size && line[i + 1] === line[i]) {
          run++
          continue
        }
        if (run >= 5) score += run - 2
        run = 1
      }
      const s = line.map(d => (d ? '1' : '0')).join('')
      for (let at = s.indexOf('1011101'); at !== -1; at = s.indexOf('1011101', at + 1)) {
        const before = s.slice(Math.max(0, at - 4), at)
        const after = s.slice(at + 7, at + 11)
        if ((at < 4 ? '0'.repeat(4 - at) + before : before) === '0000' || (after + '0'.repeat(4 - after.length)) === '0000') score += 40
      }
    }
    for (let y = 0; y + 1 < size; y++) {
      for (let x = 0; x + 1 < size; x++) {
        const c = m[y][x]
        if (c === m[y][x + 1] && c === m[y + 1][x] && c === m[y + 1][x + 1]) score += 3
      }
    }
    const darkCount = m.reduce((n, row) => n + row.filter(Boolean).length, 0)
    const total = size * size
    score += (Math.ceil(Math.abs(darkCount * 20 - total * 10) / total) - 1) * 10
    return score
  }

  function encode(text, mask) {
    const bytes = utf8(text)
    let ver = 1
    while (ver <= MAX_VERSION && 4 + (ver <= 9 ? 8 : 16) + bytes.length * 8 > dataCodewords(ver) * 8) ver++
    if (ver > MAX_VERSION) throw new Error('too long for a QR code here')
    const data = codewords(bytes, ver)
    if (mask !== undefined) return draw(ver, data, mask)
    let best = null
    for (let m = 0; m < 8; m++) {
      const drawn = draw(ver, data, m)
      const score = penalty(drawn)
      if (!best || score < best.score) best = { drawn, score }
    }
    return best.drawn
  }

  function svg(text) {
    const m = encode(text)
    const n = m.length + 8
    let path = ''
    m.forEach((row, y) => row.forEach((on, x) => on && (path += `M${x + 4} ${y + 4}h1v1h-1z`)))
    return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${n} ${n}" shape-rendering="crispEdges"><rect width="${n}" height="${n}" fill="#fff"/><path d="${path}" fill="#000"/></svg>`
  }

  const api = { encode, svg }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = api
  } else {
    root.QR = api
  }
})(this)
