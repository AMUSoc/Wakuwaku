// Master Duel's helper (duel.rs): what the opponent plays and where to stop
// it, as Claude read it off a picture of the game. While it looks, how long;
// then the deck, its plan, the effects to stop (with what, when, why), what
// to watch for, a tip, and the cards it saw. Its key brings up a bar of her
// tools down the game's right (看場面, 軍師團, 我的牌組, 掃描牌組); a card
// slides out to its left for the look or for the four, each pet with her
// part and her line on it (軍師團): one clicked is asked, the agent itself
// answering (duel.rs ask).
;(function () {
  const { t } = window.I18n
  let data = { state: 'none' }
  let timer = 0
  const T = (key, vars) => t(data.lang || 'zh-TW', key, vars)
  const $ = id => document.getElementById(id)

  function el(tag, cls, ...kids) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    for (const kid of kids.flat()) if (kid != null && kid !== false && kid !== '') node.append(kid)
    return node
  }

  const secs = ms => Math.max(0, Math.round(ms / 1000))
  const NAMES = { claude: 'Claude', codex: 'Codex', grok: 'Grok', agy: 'Antigravity' }
  let asking = null // the agent whose question box is open
  let mode = 'look' // the card beside the bar: the look or the four
  let lastAt = 0
  let typed = '' // what is typed in it, kept through redraws

  // A pet's head: the first cells of her sheet, idle, or running while she thinks.
  function head(m, busy) {
    const h = el('div', `head${busy ? ' busy' : ''}`)
    h.style.setProperty('--c', m.color || '#888')
    if (m.sprite) h.style.backgroundImage = `url("${m.sprite}")`
    else h.append(el('span', '', NAMES[m.agent][0]))
    return h
  }

  // The watch: on or resting, updating, and the cards the duel has shown.
  function watchBlock() {
    const w = data.watch || {}
    if (!w.on) return []
    const line = el('div', `watchline${w.resting ? ' resting' : ''}`, w.resting ? T('duel.watchResting') : T('duel.watchOn'))
    if (data.updating) {
      const up = el('span', 'updating', T('duel.updating', { s: secs(Date.now() - data.updating) }))
      up.since = data.updating
      line.append(up)
    }
    const out = [line]
    if ((w.seen || []).length) out.push(el('div', 'seen', el('span', 'muted', T('duel.matchSeen')), w.seen.slice(-20).map(c => el('span', 'chip', c))))
    return out
  }

  // The deck guessed, looked up on the web: how it plays, where to stop it.
  function researchBlock() {
    const r = (data.watch || {}).research
    if (!r) return []
    if (!r.found) return [el('p', 'muted', T('duel.researching', { deck: r.deck }))]
    if (r.found.failed) return []
    const v = r.found
    return [
      el('h2', '', T('duel.research', { deck: v.deck || r.deck })),
      v.plan ? el('p', '', v.plan) : null,
      ...(v.chokes || []).slice(0, 4).map(c => el('div', 'choke', el('b', '', c.card || ''), c.effect ? el('span', 'effect', `・${c.effect}`) : null, c.why ? el('div', 'why', c.why) : null)),
      (v.traps || []).length ? el('p', 'muted', T('duel.theirTraps', { cards: v.traps.join('、') })) : null,
    ].filter(Boolean)
  }

  function crewBlock(f) {
    const crew = data.crew || []
    if (!crew.length) return null
    const asks = data.asks || {}
    const said = (f && f.crew) || {}
    const cells = crew.map(m => {
      const a = asks[m.agent]
      const role = T(`duel.crew.${m.agent}`)
      let line
      if (a && a.state === 'asking') line = Object.assign(el('div', 'line busy', T('duel.crewThinking', { s: secs(Date.now() - a.since) })), { since: a.since })
      else if (a && a.state === 'failed') line = el('div', 'line bad', T('duel.crewFailed', { why: a.a || '' }))
      else if (a) line = el('div', 'line', el('span', 'q', `「${a.q}」`), a.a, a.stoodIn ? el('span', 'muted', T('duel.crewStoodIn', { name: NAMES[m.agent] })) : a.looked ? el('span', 'muted', T('duel.crewLooked')) : null)
      else line = el('div', 'line', String(said[m.agent] || '').replace(/^[^：:]{2,4}[：:]/, '') || el('span', 'muted', T('duel.crewHint')))
      const cell = el('button', `mate${asking === m.agent ? ' on' : ''}`, head(m, a && a.state === 'asking'), el('div', 'who', el('b', '', NAMES[m.agent]), el('span', 'role', role)), line)
      cell.style.setProperty('--c', m.color || '#888')
      cell.onclick = () => {
        asking = asking === m.agent ? null : m.agent
        window.pet.duelCrewTyping(!!asking)
        draw()
        if (asking) setTimeout(() => $('crewQ')?.focus(), 120)
      }
      return cell
    })
    const parts = [el('div', 'crew', cells)]
    if (asking) {
      const box = el('input', 'askq')
      box.id = 'crewQ'
      box.placeholder = T('duel.crewAsk', { name: NAMES[asking] })
      box.autocomplete = 'off'
      box.value = typed
      box.oninput = () => (typed = box.value)
      const go = q => {
        q = String(q || '').trim()
        if (!q) return
        window.pet.duelCrewAsk(asking, q)
        asking = null
        typed = ''
        window.pet.duelCrewTyping(false)
        draw()
      }
      box.onkeydown = e => {
        if (e.key === 'Enter') (e.preventDefault(), go(box.value))
        if (e.key === 'Escape') (e.preventDefault(), (asking = null), window.pet.duelCrewTyping(false), draw())
      }
      const send = el('button', 'send', T('duel.crewSend'))
      send.onclick = () => go(box.value)
      const quick = [...T(`duel.crewQ.${asking}`).split('|'), ...((f && f.seen) || []).slice(0, 3).map(card => T('duel.crewAboutCard', { card }))]
      parts.push(
        el(
          'div',
          'askbox',
          el('div', 'askrow', box, send),
          el(
            'div',
            'quick',
            quick.map(q => {
              const b = el('button', 'chip', q)
              b.onclick = () => go(q)
              return b
            }),
          ),
        ),
      )
    }
    return parts
  }
  // Where a card to stop is (the frame on the game bears its number).
  const ZONE_WORD = { M1: 'duel.zoneM', M2: 'duel.zoneM', M3: 'duel.zoneM', M4: 'duel.zoneM', M5: 'duel.zoneM', S1: 'duel.zoneS', S2: 'duel.zoneS', S3: 'duel.zoneS', S4: 'duel.zoneS', S5: 'duel.zoneS', F: 'duel.zoneF', EL: 'duel.zoneEL', ER: 'duel.zoneER', hand: 'duel.zoneHand', gy: 'duel.zoneGy', ban: 'duel.zoneBan', chain: 'duel.zoneChain' }

  // Where it is, drawn: the opponent's side as the game lays it (back row,
  // monsters with the field zone at the right, the two extra zones in the
  // middle), its zone lit with the number its frame on the game bears (she
  // could not tell "怪獸區 3" on the screen, 2026-10-23).
  const MAP = [
    ['S1', 'S2', 'S3', 'S4', 'S5', ''],
    ['M1', 'M2', 'M3', 'M4', 'M5', 'F'],
    ['', 'EL', '', 'ER', '', ''],
  ]
  function miniMap(zone, n) {
    if (!MAP.flat().includes(zone)) return null
    return el(
      'div',
      'mini',
      MAP.flat().map(z => el('i', z ? `cell${z === zone ? ' on' : ''}${z === 'F' ? ' f' : ''}` : 'gap', z === zone ? String(n) : '')),
    )
  }

  function draw() {
    clearInterval(timer)
    const typing = document.activeElement && document.activeElement.id === 'crewQ'
    drawBar()
    $('panel').hidden = !data.fly
    $('again').textContent = T('duel.again')
    $('again').hidden = mode !== 'look'
    $('again').disabled = data.state === 'looking'
    const body = $('body')
    const f = data.found || {}
    $('sure').hidden = true
    $('alt').textContent = ''
    if (mode === 'crew') {
      $('label').textContent = T('duel.crewTitle')
      $('deck').textContent = f.deck || T('duel.crewSub')
      body.replaceChildren(...(crewBlock(data.found || null) || []))
    } else if (data.state === 'looking') {
      $('label').textContent = T('duel.looking')
      $('deck').textContent = data.last && data.last.deck ? T('duel.wasDeck', { deck: data.last.deck }) : T('duel.lookingSub')
      const clock = el('div', 'clock')
      const tick = () => (clock.textContent = T('duel.for', { s: secs(Date.now() - data.since) }))
      tick()
      timer = setInterval(tick, 1000)
      body.replaceChildren(el('div', 'wait', el('div', 'dots', el('i'), el('i'), el('i')), clock, el('p', 'muted', T('duel.waitNote'))))
    } else if (data.state === 'error') {
      $('label').textContent = T('duel.oops')
      const known = ['picture', 'noGame', 'timeout'].includes(data.why)
      $('deck').textContent = T(known ? `duel.why.${data.why}` : 'duel.why.other')
      body.replaceChildren(el('p', 'muted', known || !data.why ? '' : String(data.why)), el('p', 'muted', T('duel.howTo', { key: data.key })))
    } else if (data.state === 'deck') {
      $('label').textContent = T('duel.deckRead')
      $('deck').textContent = T('duel.deckReadSub')
      body.replaceChildren(el('pre', 'mine', data.deck || ''), el('p', 'muted', T('duel.deckFix')))
    } else if (data.state === 'ready' && !f.deck) {
      $('label').textContent = T('duel.nothing')
      $('deck').textContent = ''
      body.replaceChildren(el('p', '', f.plan || ''), el('p', 'muted', T('duel.howTo', { key: data.key })))
      body.prepend(...watchBlock())
    } else if (data.state === 'ready') {
      $('label').textContent = T('duel.deckLabel')
      $('deck').textContent = f.deck
      $('alt').textContent = f.deckAlt || ''
      if (f.sure) {
        $('sure').hidden = false
        $('sure').textContent = T('duel.sure', { s: f.sure })
        $('sure').dataset.sure = f.sure
      }
      const parts = []
      // Asked whether to chain: the answer first, above all else.
      if (f.chain && f.chain.answer) parts.push(el('div', 'chain', el('div', 'q', T('duel.chainQ', { card: f.chain.card || '' })), el('div', 'a', f.chain.answer)))
      parts.unshift(...watchBlock())
      if (f.plan) parts.push(el('p', 'plan', f.plan))
      // What she read as my hand (and my set cards): advice goes by these only.
      const mine = [...(f.myHand || []), ...(f.mySet || []).map(c => `${c}${T('duel.setMark')}`)]
      parts.push(el('div', 'myhand', el('span', 'muted', T('duel.myHand')), mine.length ? mine.map(c => el('span', `chip${c === '?' ? ' unknown' : ''}`, c)) : el('span', 'muted', T('duel.noHand'))))
      const stops = (f.stop || []).slice(0, 3)
      if (stops.length) {
        parts.push(el('h2', '', T('duel.stop')))
        stops.forEach((s, i) =>
          parts.push(
            el(
              'div',
              'stop',
              miniMap(s.zone, i + 1) || el('div', 'n', String(i + 1)),
              el(
                'div',
                'what',
                el('div', 'card', s.card || '', s.effect ? el('span', 'effect', `・${s.effect}`) : null),
                el('div', 'line', s.with ? el('span', `with${/手上沒有|没有|none/i.test(s.with) ? ' none' : ''}`, s.with) : null, s.when ? el('span', 'when', s.when) : null, ZONE_WORD[s.zone] ? el('span', 'where', T(ZONE_WORD[s.zone], { n: (s.zone || '').slice(1) })) : null),
                s.why ? el('div', 'why', s.why) : null,
              ),
            ),
          ),
        )
      }
      // Her side: going first, going second, this turn's steps.
      const me = f.me || {}
      const steps = (me.now || []).slice(0, 4)
      if (me.first || me.second || steps.length) {
        parts.push(el('h2', '', T('duel.me')))
        if (steps.length) parts.push(el('ol', 'steps', steps.map(x => el('li', '', x))))
        if (me.first) parts.push(el('p', 'side', el('b', '', T('duel.first')), me.first))
        if (me.second) parts.push(el('p', 'side', el('b', '', T('duel.second')), me.second))
      }
      const watch = (f.watch || []).slice(0, 3)
      if (watch.length) {
        parts.push(el('h2', '', T('duel.watch')))
        parts.push(el('ul', 'watch', watch.map(w => el('li', '', w))))
      }
      if (f.tip) parts.push(el('div', 'tip', f.tip))
      parts.push(...researchBlock())
      const seen = (f.seen || []).slice(0, 12)
      if (seen.length) parts.push(el('div', 'seen', el('span', 'muted', T('duel.seen')), seen.map(c => el('span', 'chip', c))))
      body.replaceChildren(...parts)
    } else {
      $('label').textContent = T('duel.title')
      $('deck').textContent = ''
      body.replaceChildren(el('p', 'muted', T('duel.howTo', { key: data.key })))
    }
    // Thinking: her seconds go on.
    if (data.updating) timer = setInterval(() => document.querySelectorAll('.updating').forEach(u => (u.textContent = T('duel.updating', { s: secs(Date.now() - u.since) }))), 1000)
    else if (Object.values(data.asks || {}).some(a => a.state === 'asking') && data.state !== 'looking')
      timer = setInterval(() => document.querySelectorAll('.line.busy').forEach(l => (l.textContent = T('duel.crewThinking', { s: secs(Date.now() - l.since) }))), 1000)
    const foot = mode === 'crew' ? [] : [T('duel.keyHint', { key: data.key || 'Alt+D' })]
    if (data.state === 'ready' && data.took) foot.push(T('duel.took', { s: secs(data.took) }))
    // An answer some minutes old: said so, not taken for now.
    const age = data.state === 'ready' && data.at ? Math.floor((Date.now() - data.at) / 60000) : 0
    if (age >= 1) foot.push(T('duel.age', { n: age }))
    if (data.hand) foot.push(T('duel.handSet'))
    $('foot').textContent = foot.join(' · ')
    if (typing && $('crewQ')) $('crewQ').focus()
    fitCard()
  }

  // The card as tall as what it holds (header, body, footer, its padding).
  let fitted = 0
  function fitCard() {
    if (!data.fly) return
    requestAnimationFrame(() => {
      const p = $('panel')
      const head = p.querySelector('header').offsetHeight
      const foot = p.querySelector('footer').offsetHeight
      // The body stretches to fill: its contents measured instead.
      const r = document.createRange()
      r.selectNodeContents($('body'))
      const want = Math.ceil(head + r.getBoundingClientRect().height + foot + 16 + 12 + 12 + 16)
      if (Math.abs(want - fitted) > 4) {
        fitted = want
        window.pet.duelFit(want)
      }
    })
  }

  // The bar: each button its tool; the card it opens out, again to put it in.
  function drawBar() {
    for (const [id, key] of [['bWatch', 'duel.bar.watch'], ['bLook', 'duel.bar.look'], ['bCrew', 'duel.bar.crew'], ['bDeck', 'duel.bar.deck'], ['bScan', 'duel.bar.scan']]) $(id).querySelector('span').textContent = T(key)
    $('bScan').title = T('duel.bar.scanTip')
    $('bHide').title = T('duel.bar.hide')
    $('bLook').title = T('duel.tip.look')
    $('bWatch').title = T('duel.tip.watch')
    $('bCrew').title = T('duel.tip.crew')
    $('bDeck').title = T('duel.tip.deck')
    $('bLook').classList.toggle('on', !!data.fly && mode === 'look')
    $('bCrew').classList.toggle('on', !!data.fly && mode === 'crew')
    $('bLook').classList.toggle('busy', data.state === 'looking' || !!data.updating)
    $('bWatch').classList.toggle('on', !!(data.watch && data.watch.on))
    $('bWatch').classList.toggle('resting', !!(data.watch && data.watch.on && data.watch.resting))
  }
  const open = m => {
    mode = m
    data.fly = true
    window.pet.duelFly(true)
    draw()
  }
  const shut = () => {
    data.fly = false
    asking = null
    window.pet.duelCrewTyping(false)
    window.pet.duelFly(false)
    draw()
  }
  // 看場面: a new look (the card shows it as it goes); out already, put away.
  // 看場面: always a new look (its card's ✕ puts the card away).
  $('bLook').addEventListener('click', () => {
    open('look')
    if (data.state !== 'looking') window.pet.duelAgain()
  })
  // 盯場: on, the look's card out and kept fresh; off again by the same button.
  $('bWatch').addEventListener('click', () => {
    const on = !(data.watch && data.watch.on)
    if (on) mode = 'look'
    window.pet.duelWatch(on)
  })
  $('bCrew').addEventListener('click', () => (data.fly && mode === 'crew' ? shut() : open('crew')))
  $('bDeck').addEventListener('click', () => window.pet.duelDeckOpen())
  $('bScan').addEventListener('click', () => window.pet.duelScan())
  $('bHide').addEventListener('click', () => window.pet.duelClose())
  $('close').addEventListener('click', shut)
  $('again').addEventListener('click', () => window.pet.duelAgain())
  window.pet.onDuel(payload => {
    data = payload || { state: 'none' }
    // A look going: its card the one out.
    if (data.state === 'looking') mode = 'look'
    // The watch's new answer: the card flashes once.
    if (data.quiet && data.at && data.at !== lastAt && mode === 'look') {
      $('panel').classList.remove('flash')
      void $('panel').offsetWidth
      $('panel').classList.add('flash')
    }
    lastAt = data.at || lastAt
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
