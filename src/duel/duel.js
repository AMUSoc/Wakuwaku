// Master Duel's helper (duel.rs): what the opponent plays and where to stop
// it, as Claude read it off a picture of the game. While it looks, how long;
// then the deck, its plan, the effects to stop (with what, when, why), what
// to watch for, a tip, and the cards it saw. ✕ or its key puts it away;
// 重新看 takes a new picture.
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
  // Where a card to stop is (the frame on the game bears its number).
  const ZONE_WORD = { M1: 'duel.zoneM', M2: 'duel.zoneM', M3: 'duel.zoneM', M4: 'duel.zoneM', M5: 'duel.zoneM', S1: 'duel.zoneS', S2: 'duel.zoneS', S3: 'duel.zoneS', S4: 'duel.zoneS', S5: 'duel.zoneS', F: 'duel.zoneF', EL: 'duel.zoneEL', ER: 'duel.zoneER', hand: 'duel.zoneHand', gy: 'duel.zoneGy', chain: 'duel.zoneChain' }

  function draw() {
    clearInterval(timer)
    $('panel').hidden = false
    $('again').textContent = T('duel.again')
    $('again').disabled = data.state === 'looking'
    const body = $('body')
    const f = data.found || {}
    $('sure').hidden = true
    $('alt').textContent = ''
    if (data.state === 'looking') {
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
      if (f.plan) parts.push(el('p', 'plan', f.plan))
      const stops = (f.stop || []).slice(0, 3)
      if (stops.length) {
        parts.push(el('h2', '', T('duel.stop')))
        stops.forEach((s, i) =>
          parts.push(
            el(
              'div',
              'stop',
              el('div', 'n', String(i + 1)),
              el(
                'div',
                'what',
                el('div', 'card', s.card || '', s.effect ? el('span', 'effect', `・${s.effect}`) : null),
                el('div', 'line', s.with ? el('span', 'with', s.with) : null, s.when ? el('span', 'when', s.when) : null, ZONE_WORD[s.zone] ? el('span', 'where', T(ZONE_WORD[s.zone], { n: (s.zone || '').slice(1) })) : null),
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
      const seen = (f.seen || []).slice(0, 12)
      if (seen.length) parts.push(el('div', 'seen', el('span', 'muted', T('duel.seen')), seen.map(c => el('span', 'chip', c))))
      body.replaceChildren(...parts)
    } else {
      $('label').textContent = T('duel.title')
      $('deck').textContent = ''
      body.replaceChildren(el('p', 'muted', T('duel.howTo', { key: data.key })))
    }
    const foot = [T('duel.keyHint', { key: data.key || 'Alt+D' })]
    if (data.state === 'ready' && data.took) foot.push(T('duel.took', { s: secs(data.took) }))
    if (data.hand) foot.push(T('duel.handSet'))
    $('foot').textContent = foot.join(' · ')
  }

  $('close').addEventListener('click', () => window.pet.duelClose())
  $('again').addEventListener('click', () => window.pet.duelAgain())
  window.pet.onDuel(payload => {
    data = payload || { state: 'none' }
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
