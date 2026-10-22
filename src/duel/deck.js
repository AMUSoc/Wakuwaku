// Her own deck (duel.rs, deckscan.rs), as the game shows it: a tile a card,
// cut out of the deck editor's picture, what it was read as under it, the
// unsure ones in yellow with a "?" until she marks them right. A tile
// clicked opens on the right, large, to rename, mark right, put the same
// name on the others read alike, or remove. A theme from her ("雷熱祀劍")
// has the cards read again among that theme's, the ones marked right kept.
;(function () {
  const { t } = window.I18n
  let lang = 'zh-TW'
  let deck = { theme: '', cards: [] }
  let pictures = {}
  let scanning = null
  let hasPicture = false
  let error = null
  let picked = null
  let timer = 0
  const T = (key, vars) => t(lang, key, vars)
  const $ = id => document.getElementById(id)

  function el(tag, cls, ...kids) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    for (const kid of kids.flat()) if (kid != null && kid !== false && kid !== '') node.append(kid)
    return node
  }

  const part = c => (c.slot.startsWith('e') ? 'e' : 'm')
  const named = list => list.filter(c => c.name.trim())

  function tile(c) {
    const box = el('button', `tile${c.sure ? '' : ' unsure'}${picked === c.slot ? ' picked' : ''}`)
    const url = pictures[c.slot]
    box.append(url ? Object.assign(el('img'), { src: url, alt: '' }) : el('div', 'blank', '＋'))
    if (!c.sure) box.append(el('span', 'q', '?'))
    box.append(el('span', 'nm', c.name.trim() || T('deck.unnamed')))
    box.onclick = () => {
      picked = c.slot
      draw()
      setTimeout(() => $('pickName')?.focus(), 0)
    }
    return box
  }

  function section(key) {
    const list = deck.cards.filter(c => part(c) === key)
    const add = el('button', 'tile addtile', el('div', 'blank', '＋'), el('span', 'nm', T('deck.addOne')))
    add.onclick = () => {
      const n = deck.cards.filter(c => part(c) === key && c.slot.includes('+')).length
      const c = { slot: `${key}+${Date.now() % 100000}${n}`, name: '', sure: true }
      deck.cards.push(c)
      picked = c.slot
      draw()
      setTimeout(() => $('pickName')?.focus(), 0)
    }
    return el('section', '', el('h2', '', T(key === 'm' ? 'deck.main' : 'deck.extra', { n: named(list).length })), el('div', 'grid', list.map(tile), add))
  }

  // The card picked, large, to put right.
  function side() {
    const c = deck.cards.find(x => x.slot === picked)
    if (!c) {
      const unsure = deck.cards.filter(x => !x.sure && x.name.trim()).length
      return el('aside', 'side', el('h3', '', T('deck.howTitle')), el('p', 'muted', T('deck.how')), unsure ? el('p', 'warnline', T('deck.unsure', { n: unsure })) : el('p', 'okline', T('deck.allSure')))
    }
    const url = pictures[c.slot]
    const name = el('input', 'name')
    name.id = 'pickName'
    name.value = c.name
    name.placeholder = T('deck.namePh')
    const was = c.name
    name.addEventListener('input', () => {
      c.name = name.value
      c.sure = true
      const n = box.querySelector('.tile.picked .nm')
      if (n) n.textContent = c.name || T('deck.unnamed')
    })
    name.addEventListener('keydown', e => e.key === 'Enter' && ((picked = null), draw()))
    const alike = deck.cards.filter(x => x !== c && x.name.trim() && x.name.trim() === was.trim())
    const ok = el('button', 'act good', c.sure ? T('deck.markUnsure') : T('deck.ok'))
    ok.onclick = () => ((c.sure = !c.sure), draw())
    const same = alike.length ? el('button', 'act', T('deck.sameAll', { n: alike.length })) : null
    if (same)
      same.onclick = () => {
        for (const x of deck.cards) if (x.name.trim() === was.trim()) Object.assign(x, { name: c.name, sure: true })
        draw()
      }
    const del = el('button', 'act bad', T('deck.remove'))
    del.onclick = () => {
      deck.cards = deck.cards.filter(x => x !== c)
      picked = null
      draw()
    }
    const box = $('grids')
    return el('aside', 'side', url ? el('img', 'big') : el('div', 'bigblank', '＋'), el('label', 'th', T('deck.cardName')), name, el('div', 'acts', ok, same, del))
  }

  function draw() {
    clearInterval(timer)
    $('theme').value = deck.theme || ''
    $('rescan').disabled = !!scanning || !hasPicture
    const grids = $('grids')
    if (!deck.cards.length && !scanning) {
      grids.replaceChildren(el('div', 'empty', T(hasPicture ? 'deck.notRead' : 'deck.noPic')), section('m'), section('e'))
    } else {
      grids.replaceChildren(section('m'), section('e'))
    }
    const s = side()
    const big = s.querySelector('img.big')
    if (big) big.src = pictures[picked]
    $('side').replaceChildren(s)
    $('status').textContent = scanning ? '' : error ? T('deck.failed', { why: error }) : ''
    $('scan').hidden = !scanning
    if (scanning) {
      const tick = () => ($('scanText').textContent = T('deck.scanning', { s: Math.max(0, Math.round((Date.now() - scanning) / 1000)) }))
      tick()
      timer = setInterval(tick, 1000)
    }
    const unsure = deck.cards.filter(c => !c.sure && c.name.trim()).length
    $('foot').textContent = unsure ? T('deck.unsure', { n: unsure }) : T('deck.allSure')
  }

  $('close').onclick = () => window.pet.duelDeckClose()
  $('cancel').onclick = () => window.pet.duelDeckClose()
  $('save').onclick = () => window.pet.duelDeckSave({ theme: $('theme').value.trim(), cards: deck.cards.map(c => ({ slot: c.slot, name: c.name, sure: c.sure })) })
  $('allok').onclick = () => {
    for (const c of deck.cards) c.sure = true
    draw()
  }
  $('rescan').onclick = () => {
    deck.theme = $('theme').value.trim()
    const kept = deck.cards.filter(c => c.sure && c.name.trim() && !c.slot.includes('+')).map(c => [c.slot, c.name.trim()])
    scanning = Date.now()
    error = null
    draw()
    window.pet.duelDeckRescan(deck.theme, kept)
  }
  $('theme').addEventListener('input', e => (deck.theme = e.target.value))
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') picked ? ((picked = null), draw()) : window.pet.duelDeckClose()
  })

  window.pet.onDuelDeck(p => {
    lang = p.lang || 'zh-TW'
    document.documentElement.lang = lang
    for (const [id, key] of [['label', 'deck.label'], ['head', 'deck.title'], ['cancel', 'deck.cancel'], ['save', 'deck.save'], ['allok', 'deck.allOk'], ['rescan', 'deck.rescan'], ['themeLabel', 'deck.theme']]) $(id).textContent = T(key)
    $('theme').placeholder = T('deck.themePh')
    const d = p.deck || {}
    deck = { theme: d.theme || '', cards: (d.cards || []).map(c => ({ slot: String(c.slot), name: String(c.name || ''), sure: c.sure !== false })) }
    pictures = p.pictures || {}
    scanning = p.scanning || null
    hasPicture = !!p.hasPicture
    error = p.error || null
    if (picked && !deck.cards.some(c => c.slot === picked)) picked = null
    draw()
  })
})()
