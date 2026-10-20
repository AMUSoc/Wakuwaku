// The side island (dock.rs): one row for each agent down the right edge of
// the screen, its pet and a bubble of what it is doing. A click on a row
// opens it in place: every session of that agent, how its turn ended, its
// prompts (answered here, or on the island for the kinds only it asks).
;(function () {
  const { t } = window.I18n
  const { CELL_W, CELL_H, CLIPS, MOOD_CLIP } = window.Sprite
  const Status = window.Status

  const NAMES = { claude: 'Claude', codex: 'Codex', agy: 'Antigravity', grok: 'Grok' }
  // The pets drawn at a third of their size.
  const SCALE = 1 / 3

  const dock = document.getElementById('dock')
  let data = null
  let asks = []
  let open = null
  const T = (key, vars) => t(data?.lang || 'zh-TW', key, vars)

  function el(tag, attrs, ...kids) {
    const node = document.createElement(tag)
    for (const [k, v] of Object.entries(attrs || {})) {
      if (k === 'class') node.className = v
      else if (k === 'style') node.style.cssText = v
      else if (k.startsWith('on')) node.addEventListener(k.slice(2), v)
      else node.setAttribute(k, v)
    }
    for (const kid of kids.flat()) if (kid != null && kid !== false) node.append(kid)
    return node
  }

  // --- The pets' frames -------------------------------------------------------

  const films = new Map() // agent -> { film, mood }
  let frame = 0
  setInterval(() => {
    frame++
    for (const { film, mood } of films.values()) {
      const clip = CLIPS[MOOD_CLIP[mood] || 'idle']
      const at = Math.floor(frame / Math.max(1, Math.round(clip.ms / 100))) % clip.frames
      film.style.transform = `translate(${-at * CELL_W * SCALE}px, ${-clip.row * CELL_H * SCALE}px)`
    }
  }, 100)

  function head(row, mood) {
    const box = el('div', { class: 'head' })
    if (row.sprite) {
      const film = el('div', { class: 'film' })
      film.style.backgroundImage = `url("${row.sprite}")`
      film.style.setProperty('--sheet-h', `${row.spriteVersion === 1 ? 1872 : 2288}px`)
      box.append(film)
      films.set(row.agent, { film, mood })
    } else {
      box.append(el('div', { class: 'none' }, NAMES[row.agent][0]))
      films.delete(row.agent)
    }
    return box
  }

  // --- The rows ---------------------------------------------------------------------

  const asksOf = agent => asks.filter(a => (a.from || 'claude') === agent || (agent === 'claude' && !a.from))

  function line(s) {
    return Status.brief(data.lang, s, { detailed: data.details }) || Status.status(data.lang, s, { detailed: data.details })
  }

  function sessionView(s) {
    const time = Status.time(s)
    return el(
      'div',
      { class: 'session' },
      el('div', { class: 'name' }, Status.nameOf(s, data.details) || NAMES[s.agent] || ''),
      el('div', { class: 'line' }, [Status.status(data.lang, s, { detailed: data.details }), time].filter(Boolean).join(' · ')),
      data.details && s.reply ? el('div', { class: 'reply' }, s.reply) : null,
      s.jump
        ? el(
            'div',
            { class: 'buttons' },
            el('button', { onclick: e => (e.stopPropagation(), window.pet.jump(s.id)) }, T('dock.jump')),
          )
        : null,
    )
  }

  function askView(ask) {
    const answer = action => e => {
      e.stopPropagation()
      window.pet.answer(ask.id, { action })
    }
    const isYesNo = ask.kind === 'permission' || ask.kind === 'plan'
    const title = ask.kind === 'plan' ? T('panel.planTitle') : ask.kind === 'permission' ? T('panel.needsApproval', { tool: ask.tool || '' }) : T('dock.onIsland')
    return el(
      'div',
      { class: 'ask' },
      el('div', null, title),
      ask.summary ? el('pre', null, ask.summary) : null,
      isYesNo
        ? el(
            'div',
            { class: 'buttons' },
            el('button', { class: 'primary', onclick: answer('allow') }, T('panel.allow')),
            el('button', { class: 'danger', onclick: answer('deny') }, T('panel.deny')),
          )
        : null,
    )
  }

  // Each agent's row, kept: the pointer's enter and leave stay with it
  // whatever is drawn in it.
  const rows = new Map()
  // The pointer on a row: the window takes the mouse (dock.rs hover).
  let pointed = false
  function rowNode(agent) {
    if (rows.has(agent)) return rows.get(agent)
    const node = el('div', {
      onclick: e => {
        // In what it opened (a reply to select, a button): stays open.
        if (e.target.closest('.more')) return
        open = open === agent ? null : agent
        draw()
      },
      oncontextmenu: e => {
        e.preventDefault()
        window.pet.menu()
      },
      onmouseenter: () => {
        pointed = true
        window.pet.hover(true)
      },
      onmouseleave: () => {
        pointed = false
        window.pet.hover(false)
      },
    })
    rows.set(agent, node)
    return node
  }

  function rowView(row) {
    const now = row.now || {}
    const mood = now.mood || 'idle'
    const waiting = asksOf(row.agent)
    const isOpen = open === row.agent
    const list = row.list || []
    const isIdle = mood === 'idle' && !waiting.length
    const what = now.id ? line(now) : T('dock.none')
    const more = list.length > 1 && !isOpen ? T('dock.more', { n: list.length - 1 }) : ''
    const node = rowNode(row.agent)
    node.className = `row mood-${mood}${isIdle ? ' idle' : ''}${isOpen ? ' open' : ''}`
    node.style.setProperty('--agent', row.color)
    node.replaceChildren(
      el(
        'div',
        { class: 'bubble' },
        el('div', { class: 'who' }, el('span', { class: 'dot' }), NAMES[row.agent], more ? ` · ${more}` : ''),
        el('div', { class: 'what' }, what),
        el('div', { class: 'more' }, list.length ? list.map(sessionView) : el('div', { class: 'line' }, T('dock.none')), waiting.map(askView)),
      ),
      head(row, mood),
    )
    return node
  }

  function draw() {
    if (!data) return
    // A row's reply scrolled: kept where it was.
    const scrolled = [...dock.querySelectorAll('.more, .reply')].map(n => n.scrollTop)
    films.clear()
    const nodes = data.rows.map(rowView)
    if (nodes.some((n, i) => dock.children[i] !== n)) dock.replaceChildren(...nodes)
    dock.querySelectorAll('.more, .reply').forEach((n, i) => (n.scrollTop = scrolled[i] || 0))
  }

  window.pet.onAsks(list => {
    asks = Array.isArray(list) ? list : []
    draw()
  })

  window.pet.onDock(payload => {
    data = payload
    document.documentElement.lang = data.lang
    draw()
  })

  // Its clocks (how long a turn has run) once a second, while something
  // runs; not under the pointer, where a button may be half clicked.
  setInterval(() => {
    if (!pointed && data && data.rows.some(r => r.now && (r.now.mood === 'working' || r.now.mood === 'waiting'))) draw()
  }, 1000)
})()
