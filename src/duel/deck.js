// Her own deck (duel.rs, deckscan.rs), laid out as Master Duel's own deck
// editor (she wanted it to feel like the game's, 2026-10-23): the card's
// details on the left (its art, level, ATK/DEF, text, +1 / −1), the deck in
// the middle (主要牌組, 額外牌組, ordered as the game orders it), and the
// whole card list on the right (search by name or text, filters; nothing
// typed, her deck's cards, its theme's and the hand traps).
//
// Read off the game, each card is its scanned picture; the unsure ones carry
// a yellow "?", the ones known by their look a green 記得. A wrong one is put
// right as in the game: pick it in the deck, find the right card in the list,
// 換成這張 (copies that look alike change with it). Double-click a list card
// to add it, right-click a deck card to take one out. Saved decks are picked
// from the deck's title; nothing is lost unsaved without being asked.
;(function () {
  const { t } = window.I18n
  let lang = 'zh-TW'
  let deck = { theme: '', name: '', lib: null, cards: [] }
  let decks = []
  const taught = new Set()
  let kept = ''
  const asSaved = () => JSON.stringify({ theme: (deck.theme || '').trim(), name: (deck.name || '').trim(), cards: deck.cards.map(c => [c.slot, c.name.trim(), c.sure, c.id || null]) })
  const unsaved = () => (asSaved() !== kept || !deck.lib) && deck.cards.some(c => c.name.trim())
  let pictures = {}
  let alike = {}
  let alsoChanged = 0
  let scanning = null
  let hasPicture = false
  let error = null
  let timer = 0
  // What the left shows: a deck card (its slot), or a list card (its info).
  let picked = null
  let viewing = null
  // The database's word on each card, by "id|name".
  const infos = new Map()
  // The card list: what is typed, the filters, what came back.
  const filter = { type: '', attr: '', race: '', kind: '', level: '' }
  let query = ''
  let listed = []
  let listLoading = false
  let tab = 'list'
  const T = (key, vars) => t(lang, key, vars)
  const $ = id => document.getElementById(id)

  function el(tag, cls, ...kids) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    for (const kid of kids.flat()) if (kid != null && kid !== false && kid !== '') node.append(kid)
    return node
  }
  const img = (src, cls) => Object.assign(el('img', cls || ''), { src, alt: '', loading: 'lazy', draggable: false })

  const part = c => (c.slot.startsWith('e') ? 'e' : 'm')
  const named = list => list.filter(c => c.name.trim())

  // YGOPro's numbers (cards.rs).
  const MONSTER = 0x1
  const SPELL = 0x2
  const TRAP = 0x4
  const EXTRA = 0x40 | 0x2000 | 0x800000 | 0x4000000
  const KINDS = [0x10, 0x20, 0x80, 0x40, 0x2000, 0x800000, 0x4000000, 0x1000000, 0x1000, 0x200000, 0x200, 0x400, 0x800, 0x400000]
  const SPELL_KINDS = [0, 0x10000, 0x20000, 0x40000, 0x80000, 0x80]
  const TRAP_KINDS = [0, 0x20000, 0x100000]
  const words = key => T(key).split(',')
  const bitIndex = n => (n > 0 ? Math.round(Math.log2(n & -n)) : -1)
  const keyOf = (id, name) => `${id || ''}|${(name || '').trim()}`
  const infoOf = c => infos.get(keyOf(c.id, c.name)) || null
  const subOf = (i, list) => list.slice(1).find(b => i.kind & b) || 0
  const official = id => `https://images.ygoprodeck.com/images/cards_small/${id}.jpg`

  // What a card is: "【爬蟲類族／儀式／效果】" as the game writes it, or "速攻魔法".
  function typeLine(i) {
    if (i.kind & MONSTER) {
      const kinds = KINDS.filter(b => i.kind & b).map(b => words('deckc.kinds')[KINDS.indexOf(b)])
      return `【${[words('deckc.races')[bitIndex(i.race)], ...kinds].filter(Boolean).join('／')}】`
    }
    if (i.kind & SPELL) return `【${T('deckc.spellOf', { k: words('deckc.spellKinds')[SPELL_KINDS.indexOf(subOf(i, SPELL_KINDS))] })}】`
    return `【${T('deckc.trapOf', { k: words('deckc.trapKinds')[TRAP_KINDS.indexOf(subOf(i, TRAP_KINDS))] })}】`
  }
  const attrOf = i => (i.kind & MONSTER ? words('deckc.attrs')[bitIndex(i.attr)] || '' : i.kind & SPELL ? T('deckc.spell') : T('deckc.trap'))

  // As the game orders a deck: monsters (by kind), spells, traps; level high to low; name.
  function rank(i) {
    if (!i) return 9
    if (i.kind & MONSTER) {
      const extra = [0x40, 0x2000, 0x800000, 0x4000000].findIndex(b => i.kind & b)
      return extra >= 0 ? 1 + extra : i.kind & 0x80 ? 1 : 0
    }
    return i.kind & SPELL ? 5 + SPELL_KINDS.indexOf(subOf(i, SPELL_KINDS)) / 10 : 7 + TRAP_KINDS.indexOf(subOf(i, TRAP_KINDS)) / 10
  }
  const sorted = list =>
    list
      .map((c, n) => ({ c, i: infoOf(c), n }))
      .sort((a, b) => rank(a.i) - rank(b.i) || (b.i?.level || 0) - (a.i?.level || 0) || (a.i?.name || a.c.name).localeCompare(b.i?.name || b.c.name, lang) || a.n - b.n)
      .map(x => x.c)

  const copies = name => deck.cards.filter(c => c.name.trim() === (name || '').trim()).length

  // --- The deck (middle) ------------------------------------------------------------

  function tile(c) {
    const i = infoOf(c)
    const box = el('button', `tile${c.sure ? '' : ' unsure'}${picked === c.slot ? ' picked' : ''}`)
    const url = pictures[c.slot] || (i ? i.img : c.id ? official(c.id) : null)
    box.append(url ? img(url) : el('div', 'blank', '?'))
    if (!c.sure) box.append(el('span', 'q', '?'))
    if (c.remembered && !c.sure) box.append(el('span', 'mem', T('deck.memBadge')))
    box.append(el('span', 'nm', c.name.trim() || T('deck.unnamed')))
    box.title = c.name
    box.onclick = () => {
      picked = c.slot
      viewing = null
      draw()
    }
    // Right-click: one copy out, as the game's −1.
    box.oncontextmenu = e => {
      e.preventDefault()
      takeOut(c)
    }
    return box
  }

  function section(key) {
    const list = deck.cards.filter(c => part(c) === key)
    return el('section', `dsec ${key}`, el('h2', '', el('b', '', T(key === 'm' ? 'deck.mainHead' : 'deck.extraHead')), el('span', 'n', String(named(list).length))), el('div', 'grid', sorted(list).map(tile)))
  }

  function drawDeck() {
    const grids = $('grids')
    const scroll = grids.scrollTop
    if (!deck.cards.length && !scanning) {
      const go = el('button', 'savebtn bigscan', T('deck.scanGame'))
      go.onclick = scanGame
      grids.replaceChildren(el('div', 'empty', el('p', '', T('deck.emptyHow')), go, el('p', 'note', T('dk.orAdd'))), section('m'), section('e'))
    } else grids.replaceChildren(section('m'), section('e'))
    grids.scrollTop = scroll
  }

  // --- The card's details (left) ----------------------------------------------------

  function drawDetail() {
    const c = picked ? deck.cards.find(x => x.slot === picked) : null
    const i = viewing || (c ? infoOf(c) : null)
    const pane = $('detail')
    if (!c && !viewing) return pane.replaceChildren(el('div', 'hello', el('p', '', T('dk.pick')), el('p', 'note', T('dk.listHint'))))
    const out = []
    const name = i ? i.name : c.name || T('deck.unnamed')
    out.push(el('div', 'dname', el('b', '', name), i ? el('span', 'attr', attrOf(i)) : null))
    // Its art and numbers, as the game's left panel.
    // The official art, sharp (the scanned picture is compared below).
    const art = img(viewing ? viewing.img : (i && i.img) || (c && c.id ? official(c.id) : c && pictures[c.slot]) || '', 'art')
    const nums = el('div', 'nums')
    if (i && i.kind & MONSTER) {
      const link = i.kind & 0x4000000
      nums.append(el('div', 'lv', link ? `LINK-${i.level}` : `${i.kind & 0x800000 ? '☆' : '★'} ${i.level}`))
      nums.append(el('div', 'atk', el('small', '', 'ATK'), String(i.atk < 0 ? '?' : i.atk)))
      if (!link) nums.append(el('div', 'def', el('small', '', 'DEF'), String(i.def < 0 ? '?' : i.def)))
      if (i.scale) nums.append(el('div', 'note', T('deckc.scale', { n: i.scale })))
    }
    if (i && i.hand) nums.append(el('div', 'ht', T('deckc.handTrap')))
    const have = copies(name)
    if (have) nums.append(el('div', 'have', T('dk.inDeck', { n: have })))
    out.push(el('div', 'top2', art, nums))
    // Read off the game: the scanned picture beside the official one, to tell a misread.
    if (c && !viewing && pictures[c.slot] && (c.id || i)) {
      out.push(el('div', 'compare', el('figure', '', img(pictures[c.slot]), el('figcaption', '', T('deck.scanned'))), el('figure', '', img(official(c.id || i.id)), el('figcaption', '', T('deck.official')))))
    }
    if (c && !viewing && c.remembered && !c.sure) out.push(el('p', 'memline', T('deck.remembered')))
    if (alsoChanged) out.push(el('p', 'okline', T('deck.alsoChanged', { n: alsoChanged })))
    alsoChanged = 0
    if (i) out.push(el('div', 'type', typeLine(i)), el('p', 'desc', i.desc || ''))
    else out.push(el('p', 'note', T('deckc.noInfo', { n: 1 })))
    // The other arts of the card (alternate ones too), one clicked shown.
    const id = i ? i.id : c && c.id
    if (id) {
      const arts = el('div', 'arts')
      window.pet.duelCardArts(id).then(list => {
        if (!list || list.length < 2) return
        arts.replaceChildren(el('span', 'note', T('deck.arts', { n: list.length })), ...list.map(src => {
          const b = el('button', 'artb', img(src))
          b.onclick = () => (art.src = src)
          return b
        }))
      })
      out.push(arts)
    }
    // What can be done, at the bottom whatever the text's length: as the
    // game's +1 / −1; put a picked deck card right.
    const scroll = el('div', 'dscroll', out.splice(0))
    const acts = el('div', 'acts')
    const plus = el('button', 'act', '＋1')
    plus.onclick = () => add(i || { name, id: c && c.id, kind: 0 })
    const minus = el('button', 'act', '－1')
    minus.disabled = !have
    minus.onclick = () => (c && !viewing ? takeOut(c) : takeOutNamed(name))
    acts.append(plus, minus)
    out.push(acts)
    const target = picked && viewing ? deck.cards.find(x => x.slot === picked) : null
    if (target && viewing.name !== target.name) {
      const rep = el('button', 'savebtn wide', T('dk.replaceWhat', { name: target.name || T('deck.unnamed') }))
      rep.onclick = () => replace(target, viewing)
      out.push(rep)
    }
    if (c && !viewing) {
      const ok = el('button', `act wide ${c.sure ? '' : 'good'}`, c.sure ? T('deck.markUnsure') : T('deck.ok'))
      ok.onclick = () => ((c.sure = !c.sure), (c.remembered = false), taught.add(c.slot), draw())
      out.push(ok, el('p', 'note', T('dk.replaceHint')))
    }
    pane.replaceChildren(scroll, el('div', 'dfoot', out))
  }

  // --- Changing the deck --------------------------------------------------------------

  function add(i) {
    if (!i || !i.name) return
    const ex = (i.kind & EXTRA) !== 0
    const key = ex ? 'e' : 'm'
    if (copies(i.name) >= 3) return toast(T('dk.max3'))
    if (named(deck.cards.filter(c => part(c) === key)).length >= (ex ? 15 : 60)) return toast(T(ex ? 'dk.extraFull' : 'dk.mainFull'))
    const c = { slot: `${key}+${Date.now() % 1000000}${deck.cards.length}`, name: i.name, id: i.id || null, sure: true }
    if (i.id) infos.set(keyOf(i.id, i.name), i)
    deck.cards.push(c)
    taught.add(c.slot)
    draw()
  }

  function takeOut(c) {
    deck.cards = deck.cards.filter(x => x !== c)
    if (picked === c.slot) picked = null
    draw()
  }

  // One copy of a card out (the added ones first, then the read ones).
  function takeOutNamed(name) {
    const all = deck.cards.filter(c => c.name.trim() === name.trim())
    const c = all.find(x => x.slot.includes('+')) || all[all.length - 1]
    if (c) takeOut(c)
  }

  // A deck card put right: it, and the copies that look like it, become this card.
  function replace(target, card) {
    alsoChanged = 0
    for (const x of deck.cards)
      if (x === target || (alike[target.slot] || []).includes(x.slot)) {
        if (x !== target && (x.name !== card.name || !x.sure)) alsoChanged++
        Object.assign(x, { name: card.name, id: card.id, sure: true, remembered: false })
        taught.add(x.slot)
      }
    infos.set(keyOf(card.id, card.name), card)
    viewing = null
    draw()
  }

  // --- The card list (right) ----------------------------------------------------------

  let asked = 0
  function browse() {
    const mine = ++asked
    listLoading = true
    setTimeout(() => {
      if (mine !== asked) return
      const f = Object.fromEntries(Object.entries(filter).filter(([, v]) => v))
      window.pet.duelCardsBrowse(query, f, [...new Set(named(deck.cards).map(c => c.name.trim()))], deck.theme).then(got => {
        if (mine !== asked) return
        listLoading = !!(got && got.loading)
        listed = (got && got.cards) || []
        for (const i of listed) infos.set(keyOf(i.id, i.name), i)
        if (listLoading) setTimeout(browse, 3000)
        drawList()
      })
    }, 200)
  }

  function drawList() {
    const typedOrFiltered = query.trim() || Object.values(filter).some(Boolean)
    $('listNote').textContent = listLoading ? T('deck.dbLoading') : !listed.length ? T('dk.noResults') : typedOrFiltered ? T('dk.found', { n: listed.length }) : T('dk.around')
    $('cards').replaceChildren(
      ...listed.map(i => {
        const n = copies(i.name)
        const b = el('button', `lcard${viewing && viewing.id === i.id ? ' on' : ''}`, img(i.img), n ? el('span', 'cnt', `×${n}`) : null, i.hand ? el('span', 'ht', T('deckc.handTrap')) : null)
        b.title = `${i.name}\n${typeLine(i)}`
        b.onclick = () => ((viewing = i), draw())
        b.ondblclick = () => add(i)
        return b
      }),
    )
  }

  function drawFilters() {
    const chips = [['', T('deckc.all')], ['monster', T('deckc.monster')], ['spell', T('deckc.spell')], ['trap', T('deckc.trap')], ['hand', T('deckc.handTrap')]].map(([v, text]) => {
      const b = el('button', `chip${filter.type === v ? ' on' : ''}`, text)
      b.onclick = () => ((filter.type = v), drawFilters(), browse())
      return b
    })
    const select = (key, options, label) => {
      const s = el('select', filter[key] ? 'on' : '', el('option', '', label), ...options.map(([v, text]) => Object.assign(el('option', '', text), { value: String(v) })))
      s.firstChild.value = ''
      s.value = filter[key]
      s.onchange = () => ((filter[key] = s.value), drawFilters(), browse())
      return s
    }
    const clear = el('button', 'chip', T('deckc.clear'))
    clear.onclick = () => (Object.keys(filter).forEach(k => (filter[k] = '')), drawFilters(), browse())
    $('filters').replaceChildren(
      el('div', 'chips', chips),
      el(
        'div',
        'sels',
        select('attr', words('deckc.attrs').slice(0, 6).map((w, n) => [1 << n, w]), T('deckc.attr')),
        select('race', words('deckc.races').map((w, n) => [1 << n, w]).filter((_, n) => n !== 21 && n !== 22), T('deckc.race')),
        select('kind', KINDS.map((b, n) => [b, words('deckc.kinds')[n]]), T('deckc.kind')),
        select('level', Array.from({ length: 13 }, (_, n) => [n, `★${n}`]).slice(1), T('deckc.level')),
        clear,
      ),
    )
    $('fToggle').classList.toggle('on', Object.values(filter).some(Boolean))
  }

  // The deck's numbers (the second tab).
  function drawStats() {
    const main = named(deck.cards.filter(c => part(c) === 'm'))
    const extra = named(deck.cards.filter(c => part(c) === 'e'))
    const mi = main.map(infoOf).filter(Boolean)
    const ex = extra.map(infoOf).filter(Boolean)
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
    const exKinds = [0x40, 0x2000, 0x800000, 0x4000000].map(b => [words('deckc.kinds')[KINDS.indexOf(b)], ex.filter(i => i.kind & b).length]).filter(x => x[1])
    const missing = main.length + extra.length - mi.length - ex.length
    $('statsPane').replaceChildren(
      el('div', 'stats',
        el('p', 'big', T('deckc.split', { m: count(i => i.kind & MONSTER), s: count(i => i.kind & SPELL), t: count(i => i.kind & TRAP) })),
        el('p', '', T('deckc.hands', { n: hands }), '　', T('deckc.tuners', { n: count(i => i.kind & 0x1000) })),
        hands ? el('p', 'good', T('deckc.open', { p: chance(5), q: chance(6) })) : null,
        levels.size ? el('p', 'note', T('deckc.levels')) : null,
        levels.size ? el('div', 'bars', [...levels.entries()].sort((a, b) => a[0] - b[0]).map(([lv, n]) => el('div', 'bar', el('i', '', Object.assign(el('span'), { style: `height:${(n / most) * 100}%` })), el('small', '', `★${lv}`), el('em', '', String(n))))) : null,
        exKinds.length ? el('p', '', exKinds.map(([k, n]) => `${k} ${n}`).join('・')) : null,
        missing ? el('p', 'note', T('deckc.noInfo', { n: missing })) : null,
      ),
    )
  }

  // The database's word on the deck's cards, asked for those not yet known.
  let infoAsked = 0
  function learn() {
    const want = deck.cards.filter(c => c.name.trim() && !infos.has(keyOf(c.id, c.name)))
    if (!want.length) return
    const mine = ++infoAsked
    window.pet.duelCardInfo(want.map(c => [c.id || null, c.name.trim()])).then(got => {
      if (!got || mine !== infoAsked) return
      if (got.loading) return void setTimeout(learn, 3000)
      want.forEach((c, n) => infos.set(keyOf(c.id, c.name), got.cards[n] || null))
      draw()
    })
  }

  // --- The top: saved decks, its name, save ------------------------------------------

  let dropping = 0
  let allArmed = 0
  function drawTop() {
    const sel = $('decks')
    const options = decks.map(d => Object.assign(el('option', '', T('deck.libItem', { name: d.name, n: d.main })), { value: d.id }))
    const saved = deck.lib && decks.some(d => d.id === deck.lib)
    if (!saved) options.unshift(Object.assign(el('option', '', T('deck.unsaved')), { value: '' }))
    sel.replaceChildren(...options)
    sel.value = saved ? deck.lib : ''
    if (document.activeElement !== $('deckName')) $('deckName').value = deck.name || ''
    $('deckName').placeholder = T('deck.namePh2')
    $('newDeck').textContent = T('deck.newDeck')
    $('dropDeck').textContent = dropping ? T('deck.dropAgain') : T('deck.dropDeck')
    $('dropDeck').disabled = !deck.lib
    $('allok').textContent = allArmed ? T('deck.allOkAgain') : T('deck.allOk')
    $('theme').value = deck.theme || ''
    $('rescan').disabled = !!scanning || !hasPicture
    $('dirty').hidden = !unsaved()
    $('dirty').textContent = T('dk.unsavedDot')
  }

  function draw() {
    clearInterval(timer)
    learn()
    drawTop()
    drawDeck()
    drawDetail()
    drawList()
    if (tab === 'stats') drawStats()
    const unsure = deck.cards.filter(c => !c.sure && c.name.trim()).length
    const why = error === 'noGame' ? T('deck.noGame') : error === 'picture' ? T('deck.noShot') : error === 'notDeck' ? T('deck.notDeckScreen') : error ? T('deck.failed', { why: error }) : ''
    $('status').replaceChildren(why ? el('span', 'bad', why) : el('span', unsure ? 'warn' : 'good', unsure ? T('deck.unsure', { n: unsure }) : deck.cards.length ? T('deck.allSure') : ''), el('span', 'note', T('dk.keys')))
    $('scan').hidden = !scanning
    if (scanning) {
      const tick = () => ($('scanText').textContent = T('deck.scanning', { s: Math.max(0, Math.round((Date.now() - scanning) / 1000)) }))
      tick()
      timer = setInterval(tick, 1000)
    }
  }

  // --- Leaving, saving, scanning ------------------------------------------------------

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
    $('askBack').onclick = () => (done(), drawTop())
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

  function scanGame() {
    leaving(() => {
      scanning = Date.now()
      error = null
      draw()
      window.pet.duelScan()
    })
  }

  const close = () => leaving(() => window.pet.duelDeckClose())
  const menu = on => ($('menuList').hidden = !on)
  $('back').onclick = close
  $('save').onclick = save
  $('scanGame').onclick = scanGame
  $('menu').onclick = e => (e.stopPropagation(), menu($('menuList').hidden))
  document.addEventListener('click', e => !e.target.closest('.menuwrap') && menu(false))
  $('decks').onchange = e => {
    const id = e.target.value
    if (id) leaving(() => window.pet.duelDeckUse(id))
  }
  $('deckName').addEventListener('input', e => ((deck.name = e.target.value), ($('dirty').hidden = !unsaved())))
  $('newDeck').onclick = () => (menu(false), leaving(() => window.pet.duelDeckNew()))
  $('dropDeck').onclick = () => {
    if (!deck.lib) return
    if (!dropping) {
      dropping = setTimeout(() => ((dropping = 0), drawTop()), 3000)
      return drawTop()
    }
    clearTimeout(dropping)
    dropping = 0
    menu(false)
    window.pet.duelDeckDrop(deck.lib)
  }
  // Every card marked right: pressed twice.
  $('allok').onclick = () => {
    if (!allArmed) {
      allArmed = setTimeout(() => ((allArmed = 0), drawTop()), 3000)
      return drawTop()
    }
    clearTimeout(allArmed)
    allArmed = 0
    menu(false)
    for (const c of deck.cards) (c.sure = true), (c.remembered = false)
    draw()
  }
  $('rescan').onclick = () => {
    deck.theme = $('theme').value.trim()
    const keep = deck.cards.filter(c => c.sure && c.name.trim() && !c.slot.includes('+')).map(c => [c.slot, c.name.trim()])
    scanning = Date.now()
    error = null
    menu(false)
    draw()
    window.pet.duelDeckRescan(deck.theme, keep)
  }
  $('theme').addEventListener('input', e => (deck.theme = e.target.value))
  $('q').addEventListener('input', e => ((query = e.target.value), browse()))
  $('fToggle').onclick = () => ($('filters').hidden = !$('filters').hidden)
  const showTab = which => {
    tab = which
    $('tabList').classList.toggle('on', which === 'list')
    $('tabStats').classList.toggle('on', which === 'stats')
    $('listPane').hidden = which !== 'list'
    $('statsPane').hidden = which !== 'stats'
    if (which === 'stats') drawStats()
  }
  $('tabList').onclick = () => showTab('list')
  $('tabStats').onclick = () => showTab('stats')
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') {
      if (!$('ask').hidden) return ($('ask').hidden = true)
      if (!$('menuList').hidden) return menu(false)
      if (picked || viewing) return ((picked = null), (viewing = null), draw())
      return close()
    }
    // Ctrl+S saves; Ctrl+F to the search.
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') (e.preventDefault(), save())
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') (e.preventDefault(), $('q').focus())
  })
  // Dragged by its top bar (not its buttons).
  for (const n of [$('top'), $('label'), document.querySelector('.grow')]) n.setAttribute('data-tauri-drag-region', '')

  let first = true
  window.pet.onDuelDeck(p => {
    lang = p.lang || 'zh-TW'
    document.documentElement.lang = lang
    for (const [id, key] of [['label', 'deck.label'], ['save', 'deck.save'], ['rescan', 'deck.rescan'], ['themeLabel', 'deck.theme'], ['scanGame', 'deck.scanGameShort'], ['tabList', 'dk.list'], ['tabStats', 'deckc.statsTitle']]) $(id).textContent = T(key)
    $('theme').placeholder = T('deck.themePh')
    $('q').placeholder = T('dk.search')
    $('back').title = T('dk.backTip')
    $('scanGame').title = T('deck.emptyHow')
    $('fToggle').title = T('dk.filters')
    $('menu').title = T('dk.menu')
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
    if (first) (drawFilters(), (first = false))
    browse()
    draw()
  })
})()
