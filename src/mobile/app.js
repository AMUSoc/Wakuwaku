// The phone's page (mobile.rs): each agent's pet playing its session's mood,
// what its sessions are doing, how a turn ended, and the prompts waiting,
// answered here as on her panel. Asked again every 2 seconds while the page
// is in sight.
//
// Everything that came from the sessions (names, replies, commands) is set
// as text, never as HTML.
;(function () {
  const { t } = window.I18n
  const { CELL_W, CELL_H, CLIPS, MOOD_CLIP } = window.Sprite
  const Status = window.Status

  const POLL_MS = 2000
  // Buttons wake a moment after a prompt appears: a tap already on its way
  // does not answer it.
  const ARM_MS = 600
  // A pet drawn at half its size.
  const SCALE = 0.5
  const AGENT_NAMES = { claude: 'Claude Code', codex: 'Codex', agy: 'Antigravity', grok: 'Grok' }
  const COLOR = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }

  const agentsBox = document.getElementById('agents')
  const asksBox = document.getElementById('asks')
  const notice = document.getElementById('notice')
  const link = document.getElementById('link')

  let lang = document.documentElement.lang || 'zh-TW'
  let state = null
  let asks = []
  let lastOk = 0
  let timer
  // The prompt shown, the question it is on, what is picked and typed.
  let shownId = null
  let step = 0
  let picks = []
  let typed = ''
  let armedAt = 0
  let sending = false

  const T = (key, vars) => t(lang, key, vars)

  function el(tag, props, ...children) {
    const node = document.createElement(tag)
    for (const [key, value] of Object.entries(props || {})) {
      if (key === 'class') node.className = value
      // Through the CSSOM: the page's policy allows no style attributes.
      else if (key === 'style') node.style.cssText = value
      else if (key.startsWith('data-')) node.setAttribute(key, value)
      else node[key] = value
    }
    for (const child of children.flat()) {
      if (child === null || child === undefined || child === false) continue
      node.append(typeof child === 'string' ? document.createTextNode(child) : child)
    }
    return node
  }

  // --- Asking her ---------------------------------------------------------------------

  async function get(path) {
    const res = await fetch(path, { credentials: 'same-origin', cache: 'no-store' })
    if (res.status === 401) throw Object.assign(new Error('token'), { token: true })
    if (!res.ok) throw new Error(String(res.status))
    return res.json()
  }

  async function poll() {
    clearTimeout(timer)
    try {
      const [s, a] = await Promise.all([get('api/state'), get('api/asks')])
      state = s
      lang = s.lang || lang
      document.documentElement.lang = lang
      asks = a.asks || []
      lastOk = Date.now()
      showNotice('')
      draw()
    } catch (e) {
      showNotice(e.token ? T('m.needToken') : T('m.offline'))
      if (e.token) {
        state = null
        asks = []
        draw()
      }
    }
    drawLink()
    if (!document.hidden) timer = setTimeout(poll, POLL_MS)
  }

  async function answer(choice) {
    if (sending || shownId === null || Date.now() < armedAt) return
    sending = true
    const id = shownId
    try {
      const res = await fetch('api/answer', {
        method: 'POST',
        credentials: 'same-origin',
        headers: { 'content-type': 'application/json', 'x-wakuwaku': '1' },
        body: JSON.stringify({ id, choice }),
      })
      const got = res.ok ? await res.json() : { ok: false }
      if (!got.ok) showNotice(T('m.answerGone'))
    } catch {
      showNotice(T('m.offline'))
    }
    sending = false
    poll()
  }

  function showNotice(text) {
    notice.textContent = text
    notice.hidden = !text
  }

  function drawLink() {
    const fresh = Date.now() - lastOk < POLL_MS * 3
    link.className = `link ${fresh ? 'on' : 'off'}`
    link.textContent = fresh ? T('m.connected') : T('m.disconnected')
  }

  // --- The prompts --------------------------------------------------------------------

  const button = (label, kind, onClick) => el('button', { class: `btn ${kind}`, onclick: onClick }, label)

  function head(title, ask) {
    return el(
      'div',
      { class: 'ask-head' },
      el('span', { class: 'title' }, title),
      el('span', { class: 'chip' }, AGENT_NAMES[ask.from || 'claude'] || ask.from),
      ask.project ? el('span', { class: 'chip' }, ask.project) : null,
    )
  }

  function alwaysNote(always) {
    const rules = [
      ...always.rules,
      ...(always.dirs.length ? [T('rule.directories', { dirs: always.dirs.join(', ') })] : []),
      ...always.modes.map(mode => T('rule.mode', { mode })),
    ].join('; ')
    const where = always.where.map(w => T(`where.${w}`)).join(', ')
    return el('div', { class: 'note' }, T('panel.alwaysNote', { rules, where }))
  }

  function permissionBody(ask) {
    return [
      head(T('panel.needsApproval', { tool: ask.tool }), ask),
      el('pre', { class: 'code' }, ask.summary || ''),
      el(
        'div',
        { class: 'row' },
        button(T('panel.allow'), 'primary', () => answer({ action: 'allow' })),
        ask.always ? button(T('panel.always'), 'always', () => answer({ action: 'always' })) : null,
        button(T('panel.deny'), 'danger', () => answer({ action: 'deny' })),
      ),
      ask.always ? alwaysNote(ask.always) : null,
    ]
  }

  function planBody(ask) {
    return [
      head(T('panel.planTitle'), ask),
      el('pre', { class: 'code plan' }, ask.summary || T('panel.emptyPlan')),
      el('div', { class: 'row' }, button(T('panel.approve'), 'primary', () => answer({ action: 'allow' })), button(T('panel.deny'), 'danger', () => answer({ action: 'deny' }))),
    ]
  }

  // The current question's answer from what is picked and typed, or null.
  function currentAnswer(q) {
    const text = typed.trim()
    if (q.kind === 'text') return text ? { text } : null
    if (q.kind === 'number') {
      const n = Number(typed)
      return typed !== '' && Number.isFinite(n) && n >= q.min && n <= q.max ? { number: n } : null
    }
    const labels = Array.isArray(picks[step]) ? picks[step] : []
    if (text) return { text: [...labels, text].join(', ') }
    return labels.length ? labels : null
  }

  function next(ask) {
    const got = currentAnswer(ask.questions[step])
    if (got === null) return
    picks[step] = got
    typed = ''
    if (step < ask.questions.length - 1) {
      step += 1
      drawAsks(true)
    } else {
      answer({ action: 'answer', answers: picks })
    }
  }

  function questionBody(ask) {
    if (!ask.canAnswer) return [head(ask.title || T('panel.questionTitle'), ask), el('div', { class: 'text' }, T('panel.cannotAnswer'))]
    const q = ask.questions[step]
    const count = ask.questions.length > 1 ? ` (${step + 1}/${ask.questions.length})` : ''
    const isLast = step === ask.questions.length - 1
    const confirm = button(isLast ? T('panel.confirm') : T('panel.next'), 'primary', () => next(ask))
    const box = props => {
      const input = el('input', { class: 'input', value: typed, ...props })
      input.addEventListener('input', () => (typed = input.value))
      input.addEventListener('keydown', e => e.key === 'Enter' && next(ask))
      return input
    }
    const parts = [head(`${q.header || ask.title || T('panel.questionTitle')}${count}`, ask), el('div', { class: 'text' }, q.question), q.description ? el('div', { class: 'note' }, q.description) : null]
    if (q.kind === 'text') return [...parts, box({ type: 'text', placeholder: q.placeholder || T('panel.typeAnswer'), maxLength: 2000 }), el('div', { class: 'row' }, confirm)]
    if (q.kind === 'number') {
      if (typed === '' && Number.isFinite(q.defaultValue)) typed = String(q.defaultValue)
      return [...parts, box({ type: 'number', min: q.min, max: q.max, step: q.step, inputMode: 'decimal' }), el('div', { class: 'note' }, T('panel.numberRange', { min: q.min, max: q.max })), el('div', { class: 'row' }, confirm)]
    }
    const picked = Array.isArray(picks[step]) ? picks[step] : []
    const options = q.options.map(o =>
      el(
        'button',
        {
          class: `btn option${picked.includes(o.label) ? ' picked' : ''}`,
          onclick: () => {
            if (Date.now() < armedAt) return
            if (q.multiSelect) {
              picks[step] = picked.includes(o.label) ? picked.filter(l => l !== o.label) : [...picked, o.label]
              drawAsks(true)
            } else {
              picks[step] = [o.label]
              typed = ''
              next(ask)
            }
          },
        },
        el('span', { class: 'label' }, o.label),
        o.description ? el('span', { class: 'desc' }, o.description) : null,
      ),
    )
    return [...parts, el('div', { class: 'options' }, options), box({ type: 'text', placeholder: T('panel.other'), maxLength: 2000 }), el('div', { class: 'row' }, confirm)]
  }

  // The first prompt waiting; redrawn only when it changes, or `force`
  // (a step on), so what is typed is not lost to a poll.
  let drawnKey = ''
  function drawAsks(force = false) {
    const ask = asks[0]
    const key = ask ? `${ask.id}:${asks.length}:${lang}` : ''
    if (!force && key === drawnKey) return
    drawnKey = key
    if (!ask) {
      asksBox.hidden = true
      asksBox.replaceChildren()
      shownId = null
      return
    }
    if (ask.id !== shownId) {
      shownId = ask.id
      step = 0
      picks = []
      typed = ''
      armedAt = Date.now() + ARM_MS
      // A new prompt: to the top, where it is.
      window.scrollTo({ top: 0, behavior: 'smooth' })
    }
    const body = ask.kind === 'question' ? questionBody(ask) : ask.kind === 'plan' ? planBody(ask) : permissionBody(ask)
    const more = asks.length > 1 ? el('div', { class: 'more' }, T('panel.more', { n: asks.length - 1 })) : null
    asksBox.replaceChildren(...body.filter(Boolean), more || '')
    asksBox.hidden = false
  }

  // --- The agents ---------------------------------------------------------------------

  const cards = {}

  function card(agent) {
    if (cards[agent]) return cards[agent]
    const pet = el('div', { class: 'pet' })
    const name = el('div', { class: 'name' })
    const line = el('div', { class: 'line' })
    const reply = el('div', { class: 'reply' })
    const others = el('div', { class: 'others' })
    const root = el('article', { class: 'agent', 'data-agent': agent }, pet, el('div', { class: 'info' }, name, line, reply, others))
    agentsBox.append(root)
    cards[agent] = { root, pet, name, line, reply, others, sheet: '', mood: 'idle', version: 2 }
    return cards[agent]
  }

  function drawAgent(a) {
    const c = card(a.agent)
    const s = a.now || {}
    const detailed = state.details !== false
    const mood = s.mood || 'idle'
    c.mood = mood
    c.root.style.setProperty('--edge', a.color || COLOR.idle)
    c.root.style.setProperty('--mood', COLOR[mood] || COLOR.idle)
    c.root.classList.toggle('busy', mood !== 'idle')
    if (a.sheet && a.sheet !== c.sheet) {
      c.sheet = a.sheet
      c.version = a.version === 1 ? 1 : 2
      c.pet.style.backgroundImage = `url("${a.sheet}")`
      c.pet.style.backgroundSize = `${CELL_W * 8 * SCALE}px ${(c.version === 2 ? 2288 : 1872) * SCALE}px`
    }
    c.pet.classList.toggle('none', !a.sheet)
    const session = s.id ? Status.nameOf(s, detailed) : ''
    c.name.replaceChildren(...[el('span', { class: 'who' }, AGENT_NAMES[a.agent]), session && el('span', { class: 'session' }, ` · ${session}`)].filter(Boolean))
    // At work, the step's own clock says it (status.js stepOf).
    const time = mood === 'working' ? '' : Status.time(s)
    c.line.textContent = s.id ? [Status.status(lang, s, { detailed }), time].filter(Boolean).join(' · ') : T('m.noSession')
    // How the turn ended, in its own words (an error is in the line above).
    const said = Status.isEnding(s) ? s.reply || '' : ''
    c.reply.textContent = said
    c.reply.hidden = !said
    // The agent's other sessions: a line each, and how an ended one ended.
    const rest = (a.list || []).filter(x => x.id !== s.id)
    c.others.replaceChildren(
      ...rest.map(x => {
        const brief = Status.brief(lang, x, { detailed })
        const words = Status.isEnding(x) ? brief : [Status.nameOf(x, detailed), brief || T('mood.idle')].filter(Boolean).join(' · ')
        return el(
          'div',
          { class: 'other', style: `--mood:${COLOR[x.mood] || COLOR.idle}` },
          el('div', { class: 'other-line' }, el('i'), el('span', {}, words)),
          Status.isEnding(x) && x.reply ? el('div', { class: 'other-reply' }, x.reply) : null,
        )
      }),
    )
    c.others.hidden = !rest.length
  }

  function draw() {
    document.title = asks.length ? `(${asks.length}) Wakuwaku` : 'Wakuwaku'
    drawAsks()
    if (!state) {
      agentsBox.replaceChildren()
      for (const k of Object.keys(cards)) delete cards[k]
      return
    }
    for (const a of state.agents || []) drawAgent(a)
  }

  // Each pet plays its mood's row of the sheet.
  function animate(now) {
    for (const c of Object.values(cards)) {
      if (!c.sheet) continue
      const clip = CLIPS[MOOD_CLIP[c.mood] || 'idle']
      const frame = Math.floor(now / clip.ms) % clip.frames
      c.pet.style.backgroundPosition = `${-frame * CELL_W * SCALE}px ${-clip.row * CELL_H * SCALE}px`
    }
    requestAnimationFrame(animate)
  }

  // A ticking clock between polls (how long a step has run).
  setInterval(() => state && !document.hidden && (state.agents || []).forEach(a => a.now?.mood === 'working' && drawAgent(a)), 1000)

  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) poll()
    else clearTimeout(timer)
  })

  // Added to the home screen: only where the browser allows a worker (https,
  // or this computer itself).
  if ('serviceWorker' in navigator && window.isSecureContext) navigator.serviceWorker.register('sw.js').catch(() => {})

  requestAnimationFrame(animate)
  poll()
})()
