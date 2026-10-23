// Master Duel's helper (duel.rs): what the opponent plays and where to stop
// it, as Claude read it off a picture of the game. While it looks, how long;
// then the deck, its plan, the effects to stop (with what, when, why), what
// to watch for, a tip, and the cards it saw. Its key brings up a bar of her
// tools down the game's right (戰況, 我的牌組, 掃描牌組); 戰況 slides a card
// out to its left and keeps looking, and under the card she tells it what
// is happening, answered in a few seconds (duel.rs say).
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
  let lastAt = 0
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

  // What she told it and its answers, the newest last and brightest.
  function sayBlock() {
    const said = (data.said || []).slice(-3)
    return said.map((x, i) => {
      let a
      if (x.state === 'asking') a = Object.assign(el('div', 'a busy', T('duel.sayThinking', { s: secs(Date.now() - x.since) })), { since: x.since })
      else if (x.state === 'failed') a = el('div', 'a bad', T('duel.sayFailed', { why: x.a || '' }))
      else a = el('div', 'a', x.a || '')
      return el('div', `said${i < said.length - 1 ? ' old' : ''}`, el('div', 'q', T('duel.you'), x.q), a)
    })
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
    drawBar()
    $('panel').hidden = !data.fly
    $('again').textContent = T('duel.again')
    $('again').disabled = data.state === 'looking'
    $('say').placeholder = T('duel.sayHint')
    $('sayGo').textContent = T('duel.saySend')
    const body = $('body')
    const f = data.found || {}
    $('sure').hidden = true
    $('alt').textContent = ''
    if (data.state === 'looking') {
      $('label').textContent = T('duel.looking')
      $('deck').textContent = data.last && data.last.deck ? T('duel.wasDeck', { deck: data.last.deck }) : T('duel.lookingSub')
      const clock = el('div', 'clock')
      const tick = () => {
        clock.textContent = T('duel.for', { s: secs(Date.now() - data.since) })
        document.querySelectorAll('.said .a.busy').forEach(u => (u.textContent = T('duel.sayThinking', { s: secs(Date.now() - u.since) })))
      }
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
      // What she read on each of the opponent's zones (to see at a glance whether she read it right).
      const field = (f.field || []).filter(z => ZONE_WORD[z.zone] && z.name)
      if (field.length)
        parts.push(
          el(
            'div',
            'myhand field',
            el('span', 'muted', T('duel.theirField')),
            field.map(z => {
              const chip = el('span', `chip${/^[?？]/.test(z.name) ? ' unknown' : ''}${z.by === 'stats' ? ' stats' : ''}`, el('b', '', T(`duel.zoneShort.${z.zone.replace(/\d/, '')}`, { n: z.zone.slice(1) })), z.name)
              if (z.by === 'stats') chip.title = T('duel.byStats')
              return chip
            }),
          ),
        )
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
      // The rest (how to play on, the four's lines) still coming.
      if (data.more) parts.push(Object.assign(el('p', 'muted more', T('duel.more', { s: secs(Date.now() - data.at) })), { since: data.at }))
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
    // What she told it, above all: a chain does not wait.
    body.prepend(...sayBlock())
    // Thinking: her seconds go on.
    const asking = (data.said || []).some(x => x.state === 'asking')
    if ((data.updating || data.more || asking) && data.state !== 'looking')
      timer = setInterval(() => {
        document.querySelectorAll('.updating').forEach(u => (u.textContent = T('duel.updating', { s: secs(Date.now() - u.since) })))
        document.querySelectorAll('.more').forEach(u => (u.textContent = T('duel.more', { s: secs(Date.now() - u.since) })))
        document.querySelectorAll('.said .a.busy').forEach(u => (u.textContent = T('duel.sayThinking', { s: secs(Date.now() - u.since) })))
      }, 1000)
    const foot = [T('duel.keyHint', { key: data.key || 'Alt+D' })]
    if (data.state === 'ready' && data.took) foot.push(T('duel.took', { s: secs(data.took) }))
    // An answer some minutes old: said so, not taken for now.
    const age = data.state === 'ready' && data.at ? Math.floor((Date.now() - data.at) / 60000) : 0
    if (age >= 1) foot.push(T('duel.age', { n: age }))
    if (data.hand) foot.push(T('duel.handSet'))
    $('foot').textContent = foot.join(' · ')
    fitCard()
  }

  // The card as tall as what it holds (header, body, footer, its padding).
  let fitted = 0
  function fitCard() {
    if (!data.fly) return
    requestAnimationFrame(() => {
      const p = $('panel')
      const head = p.querySelector('header').offsetHeight
      const foot = p.querySelector('footer').offsetHeight + $('sayrow').offsetHeight + 10
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

  // The bar: each button its tool. 戰況 out: looking now and on; again: off and in.
  function drawBar() {
    for (const [id, key] of [['bWatch', 'duel.bar.watch'], ['bDeck', 'duel.bar.deck'], ['bScan', 'duel.bar.scan']]) $(id).querySelector('span').textContent = T(key)
    $('bScan').title = T('duel.bar.scanTip')
    $('bHide').title = T('duel.bar.hide')
    $('bWatch').title = T('duel.tip.watch')
    $('bDeck').title = T('duel.tip.deck')
    const on = !!(data.watch && data.watch.on)
    $('bWatch').classList.toggle('on', on)
    $('bWatch').classList.toggle('busy', data.state === 'looking' || !!data.updating)
    $('bWatch').classList.toggle('resting', on && !!data.watch.resting)
  }
  const shut = () => {
    data.fly = false
    if (data.watch && data.watch.on) window.pet.duelWatch(false)
    typing(false)
    window.pet.duelFly(false)
    draw()
  }
  $('bWatch').addEventListener('click', () => {
    if (data.watch && data.watch.on) return shut()
    data.fly = true
    window.pet.duelFly(true)
    window.pet.duelWatch(true)
    if (data.state !== 'looking') window.pet.duelAgain()
    draw()
  })
  $('bDeck').addEventListener('click', () => window.pet.duelDeckOpen())
  $('bScan').addEventListener('click', () => window.pet.duelScan())
  $('bHide').addEventListener('click', () => window.pet.duelClose())
  $('close').addEventListener('click', shut)
  $('again').addEventListener('click', () => window.pet.duelAgain())

  // Telling her: the window takes the keyboard while she types (it never
  // does otherwise, the game keeps it), and gives it back once sent.
  let keyed = false
  function typing(on) {
    if (on === keyed) return
    keyed = on
    window.pet.duelTyping(on)
    if (!on) $('say').blur()
  }
  const send = () => {
    const q = $('say').value.trim()
    if (!q) return
    window.pet.duelSay(q)
    $('say').value = ''
    typing(false)
  }
  $('say').addEventListener('pointerdown', () => {
    typing(true)
    setTimeout(() => $('say').focus(), 120)
  })
  $('say').addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.isComposing) (e.preventDefault(), send())
    if (e.key === 'Escape') (e.preventDefault(), typing(false))
  })
  $('sayGo').addEventListener('click', send)
  // Clicked away (the game, another window): the keyboard is not taken back.
  $('say').addEventListener('blur', () => keyed && ((keyed = false), window.pet.duelTyping(false)))
  window.pet.onDuel(payload => {
    data = payload || { state: 'none' }
    // The watch's new answer: the card flashes once.
    if (data.quiet && data.at && data.at !== lastAt) {
      $('panel').classList.remove('flash')
      void $('panel').offsetWidth
      $('panel').classList.add('flash')
    }
    lastAt = data.at || lastAt
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
