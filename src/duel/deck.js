// Her own deck (duel.rs duelDeck), to look over and put right: what Claude
// read off the deck editor, a row a card (its name, how many; the unsure in
// yellow until she marks them), beside the picture it read them from. Kept
// as plain lines the helper's prompt reads as they are:
//
//   主題：巳劍＋Ryzeal
//   【主要牌組】
//   灰流麗 ×3
//   天叢雲之巳劍 ×2 ？        (？: not sure yet)
//   【額外牌組】
//   …
;(function () {
  const { t } = window.I18n
  let lang = 'zh-TW'
  let deck = { theme: '', main: [], extra: [] }
  let zoom = false
  const T = (key, vars) => t(lang, key, vars)
  const $ = id => document.getElementById(id)

  function el(tag, cls, ...kids) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    for (const kid of kids.flat()) if (kid != null && kid !== false && kid !== '') node.append(kid)
    return node
  }

  function parse(text) {
    const out = { theme: '', main: [], extra: [] }
    let part = 'main'
    for (const raw of String(text || '').split('\n')) {
      const line = raw.trim()
      if (!line) continue
      if (/^主題[:：]/.test(line)) out.theme = line.replace(/^主題[:：]\s*/, '')
      else if (/額外/.test(line) && /^[【\[]/.test(line)) part = 'extra'
      else if (/主要/.test(line) && /^[【\[]/.test(line)) part = 'main'
      else {
        const m = /^(.*?)\s*[×xX＊*]\s*(\d+)\s*([？?])?$/.exec(line)
        if (m) out[part].push({ name: m[1].trim(), n: Math.max(1, Math.min(3, Number(m[2]))), sure: !m[3] })
        else out[part].push({ name: line.replace(/[？?]$/, '').trim(), n: 1, sure: !/[？?]$/.test(line) })
      }
    }
    return out
  }

  function text() {
    const rows = list => list.filter(c => c.name.trim()).map(c => `${c.name.trim()} ×${c.n}${c.sure ? '' : ' ？'}`)
    return [deck.theme.trim() ? `主題：${deck.theme.trim()}` : '', '【主要牌組】', ...rows(deck.main), '【額外牌組】', ...rows(deck.extra)].filter(Boolean).join('\n')
  }

  const count = list => list.reduce((s, c) => s + (c.name.trim() ? c.n : 0), 0)

  function row(list, i) {
    const c = list[i]
    const name = el('input', 'name')
    name.value = c.name
    name.placeholder = T('deck.namePh')
    name.addEventListener('input', () => {
      c.name = name.value
      drawFoot()
    })
    const n = el('span', 'n', String(c.n))
    const less = el('button', 'step', '−')
    less.onclick = () => ((c.n = Math.max(1, c.n - 1)), draw())
    const more = el('button', 'step', '+')
    more.onclick = () => ((c.n = Math.min(3, c.n + 1)), draw())
    const ok = c.sure ? null : el('button', 'ok', T('deck.ok'))
    if (ok) ok.onclick = () => ((c.sure = true), draw())
    const del = el('button', 'del', '✕')
    del.title = T('deck.remove')
    del.onclick = () => (list.splice(i, 1), draw())
    return el('div', `row${c.sure ? '' : ' unsure'}`, name, less, n, more, ok, del)
  }

  function section(key, list) {
    const add = el('button', 'add', T('deck.add'))
    add.onclick = () => {
      list.push({ name: '', n: 1, sure: true })
      draw()
      const inputs = $('body').querySelectorAll(`[data-part="${key}"] input.name`)
      inputs[inputs.length - 1]?.focus()
    }
    const box = el('section', '', el('h2', '', T(key === 'main' ? 'deck.main' : 'deck.extra', { n: count(list) })), list.map((_, i) => row(list, i)), add)
    box.dataset.part = key
    return box
  }

  function drawFoot() {
    const unsure = [...deck.main, ...deck.extra].filter(c => !c.sure && c.name.trim()).length
    $('foot').textContent = unsure ? T('deck.unsure', { n: unsure }) : T('deck.allSure')
  }

  function draw() {
    const theme = el('input', 'theme')
    theme.value = deck.theme
    theme.placeholder = T('deck.themePh')
    theme.addEventListener('input', () => (deck.theme = theme.value))
    $('body').replaceChildren(el('label', 'th', T('deck.theme')), theme, section('main', deck.main), section('extra', deck.extra))
    drawFoot()
  }

  function drawPic(url) {
    const pic = $('pic')
    if (!url) {
      pic.replaceChildren(el('p', 'muted', T('deck.noPic')))
      return
    }
    const img = el('img')
    img.src = url
    img.title = T('deck.zoom')
    img.onclick = () => {
      zoom = !zoom
      pic.classList.toggle('zoom', zoom)
    }
    pic.replaceChildren(img, el('figcaption', 'muted', T('deck.picNote')))
  }

  $('close').onclick = () => window.pet.duelDeckClose()
  $('cancel').onclick = () => window.pet.duelDeckClose()
  $('save').onclick = () => window.pet.duelDeckSave(text())
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') window.pet.duelDeckClose()
  })

  window.pet.onDuelDeck(p => {
    lang = p.lang || 'zh-TW'
    document.documentElement.lang = lang
    $('label').textContent = T('deck.label')
    $('head').textContent = T('deck.title')
    $('cancel').textContent = T('deck.cancel')
    $('save').textContent = T('deck.save')
    deck = parse(p.deck)
    drawPic(p.picture)
    draw()
  })
})()
