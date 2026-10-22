// Frames laid on the game itself (duel.rs): a glowing box on each card to
// stop, numbered as in the panel, with what to stop it with. The window
// covers the game's inside exactly and lets every click through. The zones
// as parts of the game's width and height, measured on a real duel
// (2026-10-22, the board as Master Duel draws it at 16:9).
;(function () {
  // [left, top, right, bottom], 0..1 of the game's inside.
  const ZONES = {
    S1: [0.293, 0.117, 0.373, 0.226],
    S2: [0.375, 0.117, 0.457, 0.226],
    S3: [0.46, 0.117, 0.543, 0.226],
    S4: [0.545, 0.117, 0.627, 0.226],
    S5: [0.63, 0.117, 0.712, 0.226],
    M1: [0.286, 0.238, 0.37, 0.372],
    M2: [0.372, 0.238, 0.456, 0.372],
    M3: [0.458, 0.238, 0.543, 0.372],
    M4: [0.544, 0.238, 0.629, 0.372],
    M5: [0.63, 0.238, 0.713, 0.372],
    F: [0.719, 0.238, 0.78, 0.363],
    EL: [0.37, 0.384, 0.454, 0.535],
    ER: [0.545, 0.384, 0.63, 0.535],
  }
  const box = document.getElementById('frames')

  window.pet.onDuelFrames(({ zones } = {}) => {
    box.replaceChildren(
      ...(zones || [])
        .filter(z => ZONES[z.zone])
        .map(z => {
          const [l, t, r, b] = ZONES[z.zone]
          const d = document.createElement('div')
          d.className = 'zone'
          d.style.left = `${l * 100}%`
          d.style.top = `${t * 100}%`
          d.style.width = `${(r - l) * 100}%`
          d.style.height = `${(b - t) * 100}%`
          const n = document.createElement('span')
          n.className = 'n'
          n.textContent = String(z.n)
          d.append(n)
          if (z.with) {
            const tag = document.createElement('span')
            tag.className = 'tag'
            tag.textContent = z.with
            d.append(tag)
          }
          return d
        }),
    )
  })
})()
