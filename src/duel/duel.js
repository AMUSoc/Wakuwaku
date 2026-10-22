// Master Duel's helper (duel.rs): what the opponent plays and where to stop
// it, as Claude read it off a picture of the game. While it looks, how long;
// then the deck, its plan, the effects to stop (with what, when, why), what
// to watch for, a tip, and the cards it saw. ✕ or its key puts it away;
// 重新看 takes a new picture. Above it all the four, each pet with her part
// and her line on it (軍師團): one clicked is asked, the agent itself
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
  let typed = '' // what is typed in it, kept through redraws

  // A pet's head: the first cells of her sheet, idle, or running while she thinks.
  function head(m, busy) {
    const h = el('div', `head${busy ? ' busy' : ''}`)
    h.style.setProperty('--c', m.color || '#888')
    if (m.sprite) h.style.backgroundImage = `url("${m.sprite}")`
    else h.append(el('span', '', NAMES[m.agent][0]))
    return h
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
      else if (a) line = el('div', 'line', el('span', 'q', `「${a.q}」`), a.a, a.stoodIn ? el('span', 'muted', T('duel.crewStoodIn', { name: NAMES[m.agent] })) : null)
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
    const parts = [el('h2', '', T('duel.crewTitle')), el('div', 'crew', cells)]
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
  const ZONE_WORD = { M1: 'duel.zoneM', M2: 'duel.zoneM', M3: 'duel.zoneM', M4: 'duel.zoneM', M5: 'duel.zoneM', S1: 'duel.zoneS', S2: 'duel.zoneS', S3: 'duel.zoneS', S4: 'duel.zoneS', S5: 'duel.zoneS', F: 'duel.zoneF', EL: 'duel.zoneEL', ER: 'duel.zoneER', hand: 'duel.zoneHand', gy: 'duel.zoneGy', chain: 'duel.zoneChain' }

  function draw() {
    clearInterval(timer)
    const typing = document.activeElement && document.activeElement.id === 'crewQ'
    $('panel').hidden = false
    $('again').textContent = T('duel.again')
    $('mydeck').textContent = T('deck.button')
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
      body.replaceChildren(el('p', 'muted', known || !data.why ? '' : String(data.why)), el('p', 'muted', T('duel.howTo', { key: data.key })), ...(crewBlock(null) || []))
    } else if (data.state === 'deck') {
      $('label').textContent = T('duel.deckRead')
      $('deck').textContent = T('duel.deckReadSub')
      body.replaceChildren(el('pre', 'mine', data.deck || ''), el('p', 'muted', T('duel.deckFix')))
    } else if (data.state === 'ready' && !f.deck) {
      $('label').textContent = T('duel.nothing')
      $('deck').textContent = ''
      body.replaceChildren(el('p', '', f.plan || ''), el('p', 'muted', T('duel.howTo', { key: data.key })), ...(crewBlock(f) || []))
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
      parts.push(...(crewBlock(f) || []))
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
      body.replaceChildren(el('p', 'muted', T('duel.howTo', { key: data.key })), ...(crewBlock(null) || []))
    }
    // Thinking: her seconds go on.
    if (Object.values(data.asks || {}).some(a => a.state === 'asking') && data.state !== 'looking')
      timer = setInterval(() => document.querySelectorAll('.line.busy').forEach(l => (l.textContent = T('duel.crewThinking', { s: secs(Date.now() - l.since) }))), 1000)
    const foot = [T('duel.keyHint', { key: data.key || 'Alt+D' })]
    if (data.state === 'ready' && data.took) foot.push(T('duel.took', { s: secs(data.took) }))
    if (data.hand) foot.push(T('duel.handSet'))
    $('foot').textContent = foot.join(' · ')
    if (typing && $('crewQ')) $('crewQ').focus()
  }

  $('close').addEventListener('click', () => window.pet.duelClose())
  $('again').addEventListener('click', () => window.pet.duelAgain())
  $('mydeck').addEventListener('click', () => window.pet.duelDeckOpen())
  window.pet.onDuel(payload => {
    data = payload || { state: 'none' }
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
