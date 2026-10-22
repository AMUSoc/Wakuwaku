// Her own deck (duel.rs, deckscan.rs), as the game shows it: a tile a card,
// cut out of the deck editor's picture, what it was read as under it, the
// unsure ones in yellow with a "?" until she marks them right. A tile
// clicked opens on the right, large, to rename, mark right, put the same
// name on the others read alike, or remove. A theme from her ("雷熱祀劍")
// has the cards read again among that theme's, the ones marked right kept.
// Each card's ATK, DEF, level, attribute, type and kind from the card
// database (cards.rs), to sort and filter by as Master Duel does, its text
// when picked, and the deck's numbers when none is (she asked for the
// game's own sorting, 2026-10-22). The window moves by its top.
;(function () {
  const { t } = window.I18n
  let lang = 'zh-TW'
  let deck = { theme: '', name: '', lib: null, cards: [] }
  // Her saved decks ({ id, name, main }); the one in the window is deck.lib.
  let decks = []
  let dropping = 0
  // The cards she put right herself since loading (their slots): only these
  // may change what a card's look is known as (duel.rs save_deck).
  const taught = new Set()
  // The deck as last loaded or saved: anything else is not saved yet.
  let kept = ''
  const asSaved = () => JSON.stringify({ theme: (deck.theme || '').trim(), name: (deck.name || '').trim(), cards: deck.cards.map(c => [c.slot, c.name.trim(), c.sure, c.id || null]) })
  // Changed since, or never kept among her decks at all.
  const unsaved = () => (asSaved() !== kept || !deck.lib) && deck.cards.some(c => c.name.trim())
  let pictures = {}
  // Slot → the slots that look like it (copies of the card, duel.rs).
  let alike = {}
  let alsoChanged = 0
  let scanning = null
  let hasPicture = false
  let error = null
  let picked = null
  let timer = 0
  // The database's word on each card, by "id|name"; what is sorted and kept.
  const infos = new Map()
  let sortBy = 'game'
  const filter = { type: '', attr: '', race: '', kind: '', level: '', q: '' }
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

  // YGOPro's numbers (cards.rs): the type bits, attributes and types one bit each.
  const MONSTER = 0x1
  const SPELL = 0x2
  const TRAP = 0x4
  const KINDS = [0x10, 0x20, 0x80, 0x40, 0x2000, 0x800000, 0x4000000, 0x1000000, 0x1000, 0x200000, 0x200, 0x400, 0x800, 0x400000]
  const SPELL_KINDS = [0, 0x10000, 0x20000, 0x40000, 0x80000, 0x80]
  const TRAP_KINDS = [0, 0x20000, 0x100000]
  const words = key => T(key).split(',')
  const bitIndex = n => (n > 0 ? Math.round(Math.log2(n & -n)) : -1)
  const infoOf = c => infos.get(`${c.id || ''}|${c.name.trim()}`) || null
  const subOf = (i, list) => list.slice(1).find(b => i.kind & b) || 0

  // What a card is, in a line: "光・炎族・超量／效果" or "速攻魔法".
  function whatIs(i) {
    if (i.kind & MONSTER) {
      const kinds = KINDS.filter(b => i.kind & b).map(b => words('deckc.kinds')[KINDS.indexOf(b)])
      return [words('deckc.attrs')[bitIndex(i.attr)], words('deckc.races')[bitIndex(i.race)], kinds.join('／')].filter(Boolean).join('・')
    }
    if (i.kind & SPELL) return T('deckc.spellOf', { k: words('deckc.spellKinds')[SPELL_KINDS.indexOf(subOf(i, SPELL_KINDS))] })
    return T('deckc.trapOf', { k: words('deckc.trapKinds')[TRAP_KINDS.indexOf(subOf(i, TRAP_KINDS))] })
  }

  // Its level, rank or link and its ATK / DEF, for a monster.
  function stars(i) {
    if (!(i.kind & MONSTER)) return ''
    const link = i.kind & 0x4000000
    const lv = link ? `LINK-${i.level}` : `${i.kind & 0x800000 ? '☆' : '★'}${i.level}`
    const n = v => (v < 0 ? '?' : v)
    return link ? `${lv}  ${n(i.atk)}` : `${lv}  ${n(i.atk)}/${n(i.def)}`
  }

  function passes(c) {
    const i = infoOf(c)
    const f = filter
    if (!f.type && !f.attr && !f.race && !f.kind && !f.level && !f.q) return true
    if (!i) return false
    if (f.type === 'monster' && !(i.kind & MONSTER)) return false
    if (f.type === 'spell' && !(i.kind & SPELL)) return false
    if (f.type === 'trap' && !(i.kind & TRAP)) return false
    if (f.type === 'hand' && !i.hand) return false
    if (f.attr && !(i.kind & MONSTER && i.attr === Number(f.attr))) return false
    if (f.race && !(i.kind & MONSTER && i.race === Number(f.race))) return false
    if (f.kind && !(i.kind & MONSTER && i.kind & Number(f.kind))) return false
    if (f.level && !(i.kind & MONSTER && i.level === Number(f.level))) return false
    const q = f.q.trim().toLowerCase()
    if (q && ![i.name, i.jp, i.en, i.desc].join('\n').toLowerCase().includes(q)) return false
    return true
  }

  // As the game orders a deck: monsters, then spells, then traps; the
  // extra deck fusion, synchro, Xyz, link; each by level high to low, then
  // name. Or by level, ATK, DEF, name, or where it was read.
  function rank(i) {
    if (!i) return 9
    if (i.kind & MONSTER) {
      const extra = [0x40, 0x2000, 0x800000, 0x4000000].findIndex(b => i.kind & b)
      return extra >= 0 ? 1 + extra : i.kind & 0x80 ? 1 : 0
    }
    return i.kind & SPELL ? 5 + SPELL_KINDS.indexOf(subOf(i, SPELL_KINDS)) / 10 : 7 + TRAP_KINDS.indexOf(subOf(i, TRAP_KINDS)) / 10
  }

  function sorted(list) {
    if (sortBy === 'slot') return list
    const by = {
      game: (a, b) => rank(a) - rank(b) || (b?.level || 0) - (a?.level || 0),
      level: (a, b) => (b?.kind & MONSTER ? b.level : -1) - (a?.kind & MONSTER ? a.level : -1),
      atk: (a, b) => (b?.kind & MONSTER ? b.atk : -2) - (a?.kind & MONSTER ? a.atk : -2),
      def: (a, b) => (b?.kind & MONSTER && !(b.kind & 0x4000000) ? b.def : -2) - (a?.kind & MONSTER && !(a.kind & 0x4000000) ? a.def : -2),
      name: () => 0,
    }[sortBy]
    return list
      .map((c, n) => ({ c, i: infoOf(c), n }))
      .sort((a, b) => by(a.i, b.i) || (a.i?.name || a.c.name).localeCompare(b.i?.name || b.c.name, lang) || a.n - b.n)
      .map(x => x.c)
  }

  function tile(c) {
    const i = infoOf(c)
    const box = el('button', `tile${c.sure ? '' : ' unsure'}${picked === c.slot ? ' picked' : ''}${passes(c) ? '' : ' dim'}${i?.hand ? ' hand' : ''}`)
    const url = pictures[c.slot] || (c.slot.includes('+') && i ? i.img : null)
    box.append(url ? Object.assign(el('img'), { src: url, alt: '' }) : el('div', 'blank', '＋'))
    if (!c.sure) box.append(el('span', 'q', '?'))
    if (c.remembered && !c.sure) box.append(el('span', 'mem', T('deck.memBadge')))
    if (i?.hand) box.append(el('span', 'ht', T('deckc.handTrap')))
    box.append(el('span', 'nm', c.name.trim() || T('deck.unnamed')))
    if (i) box.append(el('span', 'st', i.kind & MONSTER ? stars(i) : whatIs(i)))
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
    return el('section', '', el('h2', '', T(key === 'm' ? 'deck.main' : 'deck.extra', { n: named(list).length })), el('div', 'grid', sorted(list).map(tile), add))
  }

  // The card picked, large, to put right: its picture beside the official
  // one of the card chosen, the name typed with the cards it may be listed
  // under it (the card database, cards.rs); one picked is the name.
  let asking = 0
  function side() {
    const c = deck.cards.find(x => x.slot === picked)
    if (!c) {
      const unsure = deck.cards.filter(x => !x.sure && x.name.trim()).length
      return el('aside', 'side', stats(), el('h3', '', T('deck.howTitle')), el('p', 'muted', T('deck.how')), unsure ? el('p', 'warnline', T('deck.unsure', { n: unsure })) : el('p', 'okline', T('deck.allSure')))
    }
    const url = pictures[c.slot]
    const pics = el('div', 'pics', el('figure', '', url ? Object.assign(el('img', 'big'), { src: url }) : el('div', 'bigblank', '＋'), el('figcaption', '', T('deck.scanned'))))
    let arts = null
    if (c.id) {
      const official = Object.assign(el('img', 'big'), { src: `https://images.ygoprodeck.com/images/cards_small/${c.id}.jpg` })
      pics.append(el('figure', '', official, el('figcaption', '', T('deck.official'))))
      // Its other arts (alternate ones, Master Duel's too where listed): a click shows one.
      arts = el('div', 'arts')
      const id = c.id
      window.pet.duelCardArts(id).then(list => {
        if (!list || list.length < 2 || picked !== c.slot) return
        arts.replaceChildren(
          el('span', 'th', T('deck.arts', { n: list.length })),
          ...list.map(src => {
            const b = el('button', `art${src === official.src ? ' on' : ''}`, Object.assign(el('img'), { src, alt: '', loading: 'lazy' }))
            b.onclick = () => {
              official.src = src
              arts.querySelectorAll('.art').forEach(x => x.classList.toggle('on', x === b))
            }
            return b
          }),
        )
      })
    }
    const name = el('input', 'name')
    name.id = 'pickName'
    name.value = c.name
    name.placeholder = T('deck.namePh')
    name.autocomplete = 'off'
    const list = el('div', 'picks')
    let found = []
    let at = -1
    const was = c.name
    // Chosen: the copies that look like it (alike) take the name too.
    const choose = card => {
      alsoChanged = 0
      for (const x of deck.cards)
        if (x === c || (alike[c.slot] || []).includes(x.slot)) {
          if (x !== c && (x.name !== card.name || !x.sure)) alsoChanged++
          Object.assign(x, { name: card.name, id: card.id, sure: true, remembered: false })
          taught.add(x.slot)
        }
      draw()
    }
    const showFound = () => {
      list.replaceChildren(
        ...found.map((card, i) => {
          const row = el('button', `pick${i === at ? ' on' : ''}`, Object.assign(el('img'), { src: card.img, alt: '', loading: 'lazy' }), el('span', 'pn', el('b', '', card.name), el('small', '', [card.jp, card.en].filter(Boolean).join(' · '))))
          row.onmousedown = e => (e.preventDefault(), choose(card))
          return row
        }),
      )
    }
    name.addEventListener('input', () => {
      taught.add(c.slot)
      c.name = name.value
      c.id = null
      const n = $('grids').querySelector('.tile.picked .nm')
      if (n) n.textContent = c.name || T('deck.unnamed')
      const q = name.value.trim()
      const mine = ++asking
      if (!q) return ((found = []), showFound())
      setTimeout(() => {
        if (mine !== asking) return
        window.pet.duelCards(q).then(got => {
          if (mine !== asking || !got) return
          found = got.cards || []
          at = found.length ? 0 : -1
          if (got.loading && !found.length) list.replaceChildren(el('p', 'muted', T('deck.dbLoading')))
          else if (!found.length) list.replaceChildren(el('p', 'muted', T('deck.noMatch')))
          else showFound()
        })
      }, 160)
    })
    name.addEventListener('keydown', e => {
      if (e.key === 'ArrowDown' && found.length) (e.preventDefault(), (at = (at + 1) % found.length), showFound())
      else if (e.key === 'ArrowUp' && found.length) (e.preventDefault(), (at = (at - 1 + found.length) % found.length), showFound())
      else if (e.key === 'Enter') {
        e.preventDefault()
        if (found[at]) choose(found[at])
        else ((c.sure = true), (picked = null), draw())
      }
    })
    const namedAlike = deck.cards.filter(x => x !== c && x.name.trim() && x.name.trim() === was.trim())
    const ok = el('button', 'act good', c.sure ? T('deck.markUnsure') : T('deck.ok'))
    ok.onclick = () => ((c.sure = !c.sure), (c.remembered = false), taught.add(c.slot), draw())
    const same = namedAlike.length ? el('button', 'act', T('deck.sameAll', { n: namedAlike.length })) : null
    if (same)
      same.onclick = () => {
        for (const x of deck.cards) if (x.name.trim() === was.trim()) Object.assign(x, { name: c.name, id: c.id, sure: true }), taught.add(x.slot)
        draw()
      }
    const del = el('button', 'act bad', T('deck.remove'))
    del.onclick = () => {
      deck.cards = deck.cards.filter(x => x !== c)
      picked = null
      draw()
    }
    const note = c.remembered && !c.sure ? el('p', 'memline', T('deck.remembered')) : alsoChanged ? el('p', 'okline', T('deck.alsoChanged', { n: alsoChanged })) : null
    alsoChanged = 0
    const i = infoOf(c)
    const about = i
      ? el('div', 'about', el('b', '', i.name), el('span', 'what', whatIs(i)), i.kind & MONSTER ? el('span', 'nums', stars(i).replace('  ', '　ATK ').replace('/', ' / DEF ')) : null, i.scale ? el('span', 'what', T('deckc.scale', { n: i.scale })) : null, el('p', 'desc', i.desc))
      : null
    return el('aside', 'side', pics, note, arts, about, el('label', 'th', T('deck.cardName')), name, list, el('p', 'hint', T('deck.typeHint')), el('div', 'acts', ok, same, del))
  }

  // The deck's numbers: how many of each, the levels, and the chance of a
  // hand trap in the opening hand (5 cards first, 6 second).
  function stats() {
    const main = deck.cards.filter(c => part(c) === 'm' && c.name.trim())
    const extra = deck.cards.filter(c => part(c) === 'e' && c.name.trim())
    const of = list => list.map(infoOf).filter(Boolean)
    const mi = of(main)
    const count = f => mi.filter(f).length
    const hands = count(i => i.hand)
    const chance = n => {
      const N = main.length
      if (!hands || N < n) return 0
      let none = 1
      for (let k = 0; k < n; k++) none *= (N - hands - k) / (N - k)
      return Math.round((1 - Math.max(0, none)) * 100)
    }
    const levels = new Map()
    for (const i of mi) if (i.kind & MONSTER) levels.set(i.level, (levels.get(i.level) || 0) + 1)
    const most = Math.max(1, ...levels.values())
    const bars = el(
      'div',
      'bars',
      [...levels.entries()].sort((a, b) => a[0] - b[0]).map(([lv, n]) => el('div', 'bar', el('i', '', Object.assign(el('span'), { style: `height:${(n / most) * 100}%` })), el('small', '', `★${lv}`), el('em', '', String(n)))),
    )
    const ex = of(extra)
    const exKinds = [0x40, 0x2000, 0x800000, 0x4000000].map(b => [words('deckc.kinds')[KINDS.indexOf(b)], ex.filter(i => i.kind & b).length]).filter(x => x[1])
    const missing = main.length + extra.length - mi.length - ex.length
    return el(
      'div',
      'stats',
      el('h3', '', T('deckc.statsTitle')),
      el('p', '', T('deckc.split', { m: count(i => i.kind & MONSTER), s: count(i => i.kind & SPELL), t: count(i => i.kind & TRAP) })),
      el('p', '', T('deckc.hands', { n: hands }), '　', T('deckc.tuners', { n: count(i => i.kind & 0x1000) })),
      hands ? el('p', 'good', T('deckc.open', { p: chance(5), q: chance(6) })) : null,
      levels.size ? el('div', '', el('span', 'th', T('deckc.levels')), bars) : null,
      exKinds.length ? el('p', '', exKinds.map(([k, n]) => `${k} ${n}`).join('・')) : null,
      missing ? el('p', 'muted', T('deckc.noInfo', { n: missing })) : null,
    )
  }

  // The sort and the filters, as the game's deck editor has them.
  function tools() {
    const select = (key, options, label) => {
      const s = el('select', '', el('option', '', label), ...options.map(([v, text]) => Object.assign(el('option', '', text), { value: String(v) })))
      s.firstChild.value = ''
      s.value = filter[key]
      s.onchange = () => ((filter[key] = s.value), draw())
      if (filter[key]) s.classList.add('on')
      return s
    }
    const sort = el('select', 'sort', ...['game', 'level', 'atk', 'def', 'name', 'slot'].map(v => Object.assign(el('option', '', T(`deckc.sort.${v}`)), { value: v })))
    sort.value = sortBy
    sort.onchange = () => ((sortBy = sort.value), draw())
    const types = el(
      'div',
      'chips',
      [['', T('deckc.all')], ['monster', T('deckc.monster')], ['spell', T('deckc.spell')], ['trap', T('deckc.trap')], ['hand', T('deckc.handTrap')]].map(([v, text]) => {
        const b = el('button', `chip${filter.type === v ? ' on' : ''}`, text)
        b.onclick = () => ((filter.type = v), draw())
        return b
      }),
    )
    const q = el('input', 'q')
    q.id = 'filterQ'
    q.placeholder = T('deckc.search')
    q.value = filter.q
    q.oninput = () => {
      filter.q = q.value
      draw()
      const again = $('filterQ')
      again.focus()
      again.setSelectionRange(again.value.length, again.value.length)
    }
    const shown = deck.cards.filter(c => c.name.trim())
    const on = Object.values(filter).some(Boolean)
    const clear = on ? el('button', 'chip', T('deckc.clear')) : null
    if (clear) clear.onclick = () => (Object.keys(filter).forEach(k => (filter[k] = '')), draw())
    return el(
      'div',
      'tools',
      el('label', 'th', T('deckc.sort')),
      sort,
      types,
      select('attr', words('deckc.attrs').slice(0, 6).map((w, n) => [1 << n, w]), T('deckc.attr')),
      select('race', words('deckc.races').map((w, n) => [1 << n, w]).filter((_, n) => n !== 21 && n !== 22), T('deckc.race')),
      select('kind', KINDS.map((b, n) => [b, words('deckc.kinds')[n]]), T('deckc.kind')),
      select('level', Array.from({ length: 13 }, (_, n) => [n, `★${n}`]).slice(1), T('deckc.level')),
      q,
      on ? el('span', 'match', T('deckc.matching', { n: shown.filter(passes).length })) : null,
      clear,
    )
  }

  // The database's word on the deck's cards, asked for those not yet known.
  let infoAsked = 0
  function learn() {
    const want = deck.cards.filter(c => c.name.trim() && !infos.has(`${c.id || ''}|${c.name.trim()}`))
    if (!want.length) return
    const mine = ++infoAsked
    window.pet.duelCardInfo(want.map(c => [c.id || null, c.name.trim()])).then(got => {
      if (!got || mine !== infoAsked) return
      if (got.loading) return void setTimeout(learn, 3000)
      want.forEach((c, n) => infos.set(`${c.id || ''}|${c.name.trim()}`, got.cards[n] || null))
      draw()
    })
  }

  // The saved decks: one picked is taken up; a name; a new one; one thrown away (pressed twice).
  function drawLib() {
    const sel = $('decks')
    const options = decks.map(d => Object.assign(el('option', '', T('deck.libItem', { name: d.name, n: d.main })), { value: d.id }))
    if (!deck.lib) options.unshift(Object.assign(el('option', '', T('deck.unsaved')), { value: '' }))
    sel.replaceChildren(...options)
    sel.value = deck.lib || ''
    if (document.activeElement !== $('deckName')) $('deckName').value = deck.name || ''
    $('deckName').placeholder = T('deck.namePh2')
    $('newDeck').textContent = T('deck.newDeck')
    $('dropDeck').textContent = dropping ? T('deck.dropAgain') : T('deck.dropDeck')
    $('dropDeck').disabled = !deck.lib
  }

  function draw() {
    clearInterval(timer)
    drawLib()
    learn()
    $('tools').replaceChildren(tools())
    $('theme').value = deck.theme || ''
    $('rescan').disabled = !!scanning || !hasPicture
    const grids = $('grids')
    const scroll = grids.scrollTop
    if (!deck.cards.length && !scanning) {
      // Nothing yet: read it from the game, the big way in.
      const go = el('button', 'primary bigscan', T('deck.scanGame'))
      go.onclick = scanGame
      grids.replaceChildren(el('div', 'empty', el('p', '', T('deck.emptyHow')), go), section('m'), section('e'))
    } else {
      grids.replaceChildren(section('m'), section('e'))
    }
    grids.scrollTop = scroll
    $('side').replaceChildren(side())
    $('status').textContent = scanning ? '' : error === 'noGame' ? T('deck.noGame') : error === 'picture' ? T('deck.noShot') : error ? T('deck.failed', { why: error }) : ''

    $('scan').hidden = !scanning
    if (scanning) {
      const tick = () => ($('scanText').textContent = T('deck.scanning', { s: Math.max(0, Math.round((Date.now() - scanning) / 1000)) }))
      tick()
      timer = setInterval(tick, 1000)
    }
    const unsure = deck.cards.filter(c => !c.sure && c.name.trim()).length
    $('foot').textContent = unsure ? T('deck.unsure', { n: unsure }) : T('deck.allSure')
  }

  // Before what would lose what is not saved: save it, leave it, or stay.
  function leaving(then) {
    if (!unsaved()) return then()
    $('askText').textContent = T('deck.unsavedAsk', { name: (deck.name || '').trim() || T('deck.unsaved') })
    $('askSave').textContent = T('deck.save')
    $('askDrop').textContent = T('deck.dontSave')
    $('askBack').textContent = T('deck.back')
    $('ask').hidden = false
    const done = () => ($('ask').hidden = true)
    $('askSave').onclick = () => (done(), save(), then())
    $('askDrop').onclick = () => (done(), then())
    $('askBack').onclick = () => (done(), drawLib())
  }

  let toastTimer = 0
  function toast(text) {
    $('toast').textContent = text
    $('toast').hidden = false
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => ($('toast').hidden = true), 2600)
  }

  function save() {
    deck.name = $('deckName').value.trim()
    window.pet.duelDeckSave({ theme: $('theme').value.trim(), name: deck.name, taught: [...taught], cards: deck.cards.map(c => ({ slot: c.slot, name: c.name, sure: c.sure, id: c.id || null, remembered: !!c.remembered })) })
  }

  // From the game: its deck editor pictured and read, a new deck.
  function scanGame() {
    leaving(() => {
      scanning = Date.now()
      error = null
      draw()
      window.pet.duelScan()
    })
  }

  const close = () => leaving(() => window.pet.duelDeckClose())
  $('close').onclick = close
  $('cancel').onclick = close
  $('decks').onchange = e => {
    const id = e.target.value
    if (id) leaving(() => window.pet.duelDeckUse(id))
  }
  $('deckName').addEventListener('input', e => (deck.name = e.target.value))
  $('newDeck').onclick = () => leaving(() => window.pet.duelDeckNew())
  $('scanGame').onclick = scanGame
  $('dropDeck').onclick = () => {
    if (!deck.lib) return
    if (!dropping) {
      dropping = setTimeout(() => ((dropping = 0), drawLib()), 3000)
      return drawLib()
    }
    clearTimeout(dropping)
    dropping = 0
    window.pet.duelDeckDrop(deck.lib)
  }
  $('save').onclick = save
  $('allok').onclick = () => {
    for (const c of deck.cards) (c.sure = true), (c.remembered = false)
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
    if (e.key !== 'Escape') return
    if (!$('ask').hidden) return ($('ask').hidden = true)
    picked ? ((picked = null), draw()) : close()
  })
  // Dragged by its top (Tauri's drag region: on the header and what is
  // written in it, not its buttons and boxes).
  $('label').setAttribute('data-tauri-drag-region', '')
  document.querySelector('header').setAttribute('data-tauri-drag-region', '')
  document.querySelector('.title').setAttribute('data-tauri-drag-region', '')

  window.pet.onDuelDeck(p => {
    lang = p.lang || 'zh-TW'
    document.documentElement.lang = lang
    for (const [id, key] of [['label', 'deck.label'], ['cancel', 'deck.cancel'], ['save', 'deck.save'], ['allok', 'deck.allOk'], ['rescan', 'deck.rescan'], ['themeLabel', 'deck.theme']]) $(id).textContent = T(key)
    $('theme').placeholder = T('deck.themePh')
    $('scanGame').textContent = T('deck.scanGameShort')
    $('scanGame').title = T('deck.emptyHow')
    const d = p.deck || {}
    decks = p.decks || []
    deck = { theme: d.theme || '', name: d.name || '', lib: d.lib || null, cards: (d.cards || []).map(c => ({ slot: String(c.slot), name: String(c.name || ''), sure: c.sure !== false, id: c.id || null, remembered: !!c.remembered })) }
    pictures = p.pictures || {}
    alike = p.alike || {}
    scanning = p.scanning || null
    hasPicture = !!p.hasPicture
    error = p.error || null
    if (picked && !deck.cards.some(c => c.slot === picked)) picked = null
    kept = asSaved()
    if (p.saved) toast(T('deck.savedAs', { name: p.saved }))
    taught.clear()
    draw()
  })
})()
