// The settings, grown out of the island: one click on it opens them (and only
// then does the island take the keyboard); Esc, a click outside, ✕ or the head
// close them. Five pages:
//   now       the sessions (a click on one goes to its window), and four quick switches
//   pets      the pets downloaded, a download by link or id, the gallery
//   look      her island, size, strolls, eyes, language
//   alerts    how long endings stay, and idle sessions in sight; notifications,
//             sound, prompts, quiet
//   widgets   plugins: the built-in ones, those she runs for you (a switch, their settings folded under), their order, turns, nudges, waku, and how to write one
//   connect   how Claude Code and Codex reach her, start at login, the hooks, about
//
// What they show comes from main as a snapshot, again whenever it changes;
// what they change goes back as a patch, checked there. island.js decides the
// island's shape (it is 'settings' while these are open) and places her.
//
// Everything that came from elsewhere (session names, pet names, errors) is
// escaped before it goes into the page.
;(function () {
  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  if (ROLE !== 'island') {
    window.Settings = { isOpen: () => false }
    return
  }

  const { t, list: nameList } = window.I18n
  const { CLIPS } = window.Sprite
  const Status = window.Status
  const Widgets = window.Widgets

  // The island's colours (on black), the same as island.js.
  const COLOR = { idle: '#8e8e93', working: '#5e9bff', waiting: '#ffb340', done: '#34d27b', review: '#b18cff', error: '#ff5c6c' }
  // Four pages: General (the sessions, the alerts, connecting, startup and
  // the language), Look (her home, Windows, her and the pets), Plugins and
  // Mail. The pets, the alerts and connecting were pages of their own: asked
  // for by those names (island.rs asks for the pets, or connecting), the
  // page they are part of opens, scrolled to them (PART_OF, showPart).
  const TABS = ['now', 'look', 'widgets']
  const TAB_KEY = { now: 's.tabGeneral', look: 's.tabLook', widgets: 's.tabWidgets' }
  const PART_OF = { pets: 'look', alerts: 'now', connect: 'now', update: 'now', crash: 'now' }
  const HEAD_KEY = { working: 's.headWorking', waiting: 's.headWaiting', done: 's.headDone', review: 's.headReview', error: 's.headError' }
  const SIZES = [['small', 0.4], ['medium', 0.55], ['large', 0.75]]
  const HOLDS = ['seen', 8, 30, 120]
  // Minutes a session at rest stays in sight; 0 always (state.rs is_shown).
  const IDLE_HIDES = [15, 30, 60, 120, 0]
  const WAITS = [30, 60, 120, 290]
  const SPINS = [0, 5, 8, 15]
  const ICON = {
    eye: '<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    moon: '<path d="M20 14A8 8 0 1 1 10 4a7 7 0 0 0 10 10z"/>',
    sound: '<path d="M4 9v6h4l5 4V5L8 9H4z"/><path d="M16 9a4 4 0 0 1 0 6"/>',
    pill: '<rect x="3" y="8" width="18" height="8" rx="4"/>',
  }

  const layer = document.getElementById('island-settings')
  let isOpen = false
  // When they last opened: on a Mac the loss of focus that comes with taking
  // the keyboard is not a click outside (opened from her menu, the menu going
  // away gave a blur at once, and they shut as they opened, 2026-10-14). On
  // Windows a blur closes them at once, as ever.
  let openedAt = 0
  const BLUR_GRACE_MS = 500
  let tab = 'now'
  let snap = null
  let lang = 'en'
  // The crash report shown, as it would be sent (pageCrash).
  let crashShown = false
  // The download box: what is typed, and how the last download went.
  let typedRef = ''
  let fetching = null
  let fetchNote = { text: '', isError: false }
  // The gallery, loaded the first time the pets page is shown; or what a
  // search finds there, the words typed in the download box (a link there
  // is downloaded, any other words are looked for as the site's own search
  // box looks: a name, part of one, a word of its description). Each ask
  // numbered: a slower answer to an earlier search never lands over a later.
  const gallery = { sort: 'popular', query: '', items: [], page: 0, totalPages: 1, total: 0, loading: false, error: '', getting: '', asked: 0 }
  let searchTimer
  const isLink = text => /:\/\/|codex-pets\.net/i.test(text)
  let thumbTimers = []
  let clockTimer
  // The plugins page: the plugin whose settings are folded out, and what is
  // typed in them and not yet kept ("weather.City": "上海").
  let openPlugin = ''
  const drafts = {}
  // The colour picker is open: the focus it takes does not close the
  // settings.
  let picking = false
  // Phone access at an address of one's own being typed (none set yet), and
  // how the last ntfy push to try it went.
  let mobileCustom = false
  let pushNote = null

  const T = (key, vars) => t(lang, key, vars)
  const esc = value =>
    String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
  const svg = name => `<svg class="ic" viewBox="0 0 24 24">${ICON[name]}</svg>`

  layer.innerHTML = `
    <div class="s-head" data-close></div>
    <div class="s-tabs"><span class="ind"></span>${TABS.map(k => `<button data-tab="${k}"><span></span><i class="n" hidden></i></button>`).join('')}</div>
    <div class="s-ask"></div>
    <div class="s-body"></div>
    <div class="s-foot"><span class="foot-text"></span><span class="grow"></span><span class="ver"></span></div>`
  const head = layer.querySelector('.s-head')
  const tabs = layer.querySelector('.s-tabs')
  tabs.style.setProperty('--tabs', TABS.length)
  const askSlot = layer.querySelector('.s-ask')
  const body = layer.querySelector('.s-body')
  const foot = layer.querySelector('.s-foot')

  // The new markup, changed in place: only what differs is touched, so a
  // number that changed (a plugin's reading) does not redraw the page around
  // it, and a box being typed in keeps its focus and caret.
  function setHTML(el, html) {
    if (el._html === html) return false
    el._html = html
    const next = document.createElement('template')
    next.innerHTML = html
    morphChildren(el, next.content)
    return true
  }

  function morph(from, to) {
    if (from.nodeType !== to.nodeType || from.nodeName !== to.nodeName) return from.replaceWith(to.cloneNode(true))
    if (from.nodeType !== Node.ELEMENT_NODE) {
      if (from.nodeValue !== to.nodeValue) from.nodeValue = to.nodeValue
      return
    }
    for (const { name } of [...from.attributes]) if (!to.hasAttribute(name)) from.removeAttribute(name)
    for (const { name, value } of to.attributes) if (from.getAttribute(name) !== value) from.setAttribute(name, value)
    // A box's text, once typed in, is its own: set while it is not being typed in.
    if (from.nodeName === 'INPUT' && from !== document.activeElement && from.value !== (to.getAttribute('value') ?? '')) from.value = to.getAttribute('value') ?? ''
    morphChildren(from, to)
    if (from.nodeName === 'TEXTAREA' && from !== document.activeElement && from.value !== to.textContent) from.value = to.textContent
  }

  function morphChildren(from, to) {
    const have = [...from.childNodes]
    const want = [...to.childNodes]
    want.forEach((node, i) => (i < have.length ? morph(have[i], node) : from.appendChild(node.cloneNode(true))))
    for (const extra of have.slice(want.length)) extra.remove()
  }

  function clock(ms) {
    const s = Math.max(0, Math.floor(ms / 1000))
    const h = Math.floor(s / 3600)
    const m = Math.floor((s % 3600) / 60)
    const sec = String(s % 60).padStart(2, '0')
    return h ? `${h}:${String(m).padStart(2, '0')}:${sec}` : `${m}:${sec}`
  }

  // How long a turn has run, or how long the finished one took.
  function timeOf(s) {
    if ((s.mood === 'working' || s.mood === 'waiting') && s.since) return clock(Date.now() - s.since)
    if ((s.mood === 'done' || s.mood === 'review' || s.mood === 'error') && s.took) return clock(s.took)
    return ''
  }

  // A clock as markup that tick() keeps current, so a redraw is not needed for it.
  const clockTag = (s, cls = 'clock') =>
    `<span class="${cls}" data-mood="${esc(s.mood)}" data-since="${s.since || ''}" data-took="${s.took || ''}" style="color:${COLOR[s.mood] || COLOR.idle}">${timeOf(s)}</span>`

  function tick() {
    for (const c of layer.querySelectorAll('[data-mood][data-since]')) {
      c.textContent = timeOf({ mood: c.dataset.mood, since: Number(c.dataset.since) || null, took: Number(c.dataset.took) || null })
    }
  }

  // --- Small parts -------------------------------------------------------------------

  const sw = (key, on, extra = '') => `<button class="sw${on ? ' on' : ''}" data-sw="${key}" ${extra}></button>`
  const seg = (key, options, current) =>
    `<span class="seg" data-seg="${key}">${options.map(([value, label]) => `<span data-value="${esc(value)}" class="${String(value) === String(current) ? 'on' : ''}">${esc(label)}</span>`).join('')}</span>`
  const row = (title, note, control) =>
    `<div class="r"><div class="grow"><div>${title}</div>${note ? `<div class="d">${note}</div>` : ''}</div>${control}</div>`
  const sec = text => `<div class="sec">${esc(text)}</div>`
  // Where a part of a page begins, to scroll to (PART_OF).
  const part = (id, html) => `<i class="part" data-part="${id}"></i>${html}`

  function thumb(px, url, version, clip = 'idle') {
    const s = px / 192
    const sheetH = version === 1 ? 1872 : 2288
    return `<div class="thumb" data-clip="${clip}" style="width:${px}px;height:${Math.round(208 * s)}px"><div class="sheet" style="background-image:url(&quot;${esc(url)}&quot;);background-size:1536px ${sheetH}px;transform:scale(${s})"></div></div>`
  }

  function startThumbs() {
    for (const timer of thumbTimers) clearTimeout(timer)
    thumbTimers = []
    for (const el of layer.querySelectorAll('.thumb')) {
      const { row: r, frames, ms } = CLIPS[el.dataset.clip] || CLIPS.idle
      const sheet = el.querySelector('.sheet')
      let frame = 0
      const step = () => {
        sheet.style.backgroundPosition = `${-frame * 192}px ${-r * 208}px`
        frame = (frame + 1) % frames
        const timer = setTimeout(step, ms)
        thumbTimers.push(timer)
      }
      step()
    }
  }

  // --- The pages -------------------------------------------------------------------------

  function pageNow() {
    const s = snap.settings
    const sessions = snap.sessions || []
    // Each session: its name, and where it is (or how its turn ended).
    const detailed = s.details !== false
    const list = sessions.length
      ? sessions
          .map(x => {
            const name = Status.nameOf(x, detailed)
            const what = [
              { codex: 'Codex', agy: 'Antigravity', grok: 'Grok' }[x.agent] || '',
              Status.status(lang, x, { detailed, withClock: false }),
              detailed && x.name && x.project !== name ? x.project : '',
              detailed && Status.isEnding(x) ? x.reply : '',
            ].filter(Boolean)
            // One with a window to go to: a click on it goes there.
            const jump = x.jump ? ` data-jump="${esc(x.id)}"` : ''
            const tip = [...what, x.jump ? T('jump.hint') : ''].filter(Boolean).join('\n')
            return `<div class="r${x.jump ? ' jumps' : ''}"${jump} title="${esc(tip)}"><span class="dot" style="background:${COLOR[x.mood] || COLOR.idle}"></span><div class="grow"><div class="ellip">${esc(name || '—')}</div><div class="d ellip">${esc(what.join(' · '))}</div></div>${clockTag(x)}</div>`
          })
          .join('')
      : `<div class="note">${esc(T('home.noSessions', { list: toolNames(toolsHere()) }))}</div>`
    // Her out on the desktop.
    const tiles = [
      ['show', 'eye', T('s.outTile'), !snap.hidden && snap.herOut === true],
      ['dnd', 'moon', T('menu.dnd'), s.dnd],
      ['sound', 'sound', T('settings.sound'), s.sound],
    ]
    return `${sec(T('home.sessions'))}<div class="grp">${list}</div>
      ${sec(T('s.quick'))}<div class="tiles">${tiles
        .map(([key, icon, label, on]) => `<div class="tile${on ? ' on' : ''}" data-tile="${key}"><span class="c">${svg(icon)}</span>${esc(label)}</div>`)
        .join('')}</div>`
  }

  // Dates and times as each language writes them.
  const LOCALES = { zh: 'zh-CN', 'zh-TW': 'zh-TW', en: 'en-US' }

  // The agents a pet may be set for (data.rs AGENTS), by name.
  const AGENT_NAMES = { claude: 'Claude Code', codex: 'Codex', agy: 'Antigravity', grok: 'Grok' }

  // Each agent's own pet (agentPets): the island draws a session as its
  // agent's, one not set (or gone) as the pet in use (data.rs pet_for). And
  // whether it is out on the desktop (desktop): a pet of its own there,
  // showing its agent's sessions alone.
  function agentPetsPart() {
    const own = snap.settings.agentPets || {}
    const out = snap.settings.desktop || {}
    const inUse = snap.pets.find(p => p.id === snap.settings.pet)
    const rows = Object.entries(AGENT_NAMES)
      .map(([agent, name]) => {
        const pet = snap.pets.find(p => p.id === own[agent])
        const shown = pet || inUse
        const note = pet ? pet.name : T('s.agentPetDefault', { name: inUse ? inUse.name : '—' })
        const dir = (snap.settings.agentDirs || {})[agent] || ''
        return `<div class="r petrow">${shown ? thumb(36, shown.url, shown.version) : ''}<div class="grow"><div>${esc(name)}</div><div class="d">${esc(note)}</div>
          <input class="in agent-dir" data-agent-dir="${agent}" spellcheck="false" placeholder="${esc(T('s.agentDir'))}" title="${esc(T('s.agentDirNote'))}" value="${esc(dir)}"></div>
          ${pet ? `<button class="pbtn sm" data-agent-pet-clear="${agent}">${esc(T('s.agentPetClear'))}</button>` : ''}
          <span class="d agent-out">${esc(T('s.agentOut'))}</span><button class="sw${out[agent] ? ' on' : ''}" data-agent-out="${agent}" title="${esc(T('s.agentOut'))}"></button></div>`
      })
      .join('')
    return `${sec(T('s.agentPets'))}<div class="grp">${rows}<div class="note">${esc(T('s.agentPetsNote'))}</div></div>`
  }

  // A pet's chips: the agents it is set for, a press setting it or not.
  function agentChips(p) {
    const own = snap.settings.agentPets || {}
    return `<div class="agent-chips">${Object.entries(AGENT_NAMES)
      .map(([agent, name]) => `<span class="chip pick${own[agent] === p.id ? ' on' : ''}" data-agent-pet="${agent}" data-pet-id="${esc(p.id)}">${esc(name)}</span>`)
      .join('')}</div>`
  }

  function pagePets() {
    const current = snap.settings.pet
    const installed = snap.pets.length
      ? snap.pets
          .map(
            p => `<div class="r petrow">${thumb(52, p.url, p.version)}<div class="grow"><div>${esc(p.name)}</div><div class="d">${esc([p.author, p.codex ? T('s.fromCodex') : '', p.version === 1 ? T('home.v1') : ''].filter(Boolean).join(' · '))}</div>${agentChips(p)}</div>
            ${p.id === current ? `<span class="d in-use">${esc(T('home.inUse'))}</span>` : `<button class="pbtn" data-use="${esc(p.id)}">${esc(T('home.use'))}</button>`}</div>`,
          )
          .join('')
      : `<div class="note">${esc(T('settings.petsEmpty'))}</div>`
    const note = fetchNote.text ? `<div class="note${fetchNote.isError ? ' err' : ''}">${esc(fetchNote.text)}</div>` : `<div class="note">${esc(T('settings.fetchNote'))}</div>`
    const have = new Set(snap.pets.map(p => p.id))
    const cards = gallery.items
      .map(
        g => `<div class="card"><div class="pv">${g.preview ? `<img src="${esc(g.preview)}" alt="" loading="lazy">` : ''}</div>
          <div class="nm ellip" title="${esc(g.name)}">${esc(g.name)}</div><div class="d ellip">${esc(g.author)} · ♥ ${g.likes}</div>
          ${have.has(g.id) ? `<span class="d in-use">${esc(T('home.installed'))}</span>` : `<button class="pbtn sm" data-get="${esc(g.id)}" ${gallery.getting ? 'disabled' : ''}>${esc(gallery.getting === g.id ? T('home.getting') : T('home.get'))}</button>`}</div>`,
      )
      .join('')
    const more =
      gallery.loading ? `<div class="note">${esc(T('home.getting'))}</div>` : gallery.page < gallery.totalPages ? `<button class="pbtn wide" data-more>${esc(T('home.more'))}</button>` : ''
    // The button downloads a link, and looks for other words.
    const fetchLabel = fetching ? T('settings.fetching') : T(isLink(typedRef.trim()) ? 'settings.fetch' : 's.search')
    const q = gallery.query
    const listTitle = !q ? T('home.gallery') : gallery.loading && !gallery.items.length ? T('s.searching', { q }) : T('s.found', { q, n: gallery.total })
    const none = q && !gallery.loading && !gallery.error && !gallery.items.length ? `<div class="note">${esc(T('s.foundNone', { q }))}</div>` : ''
    return `${sec(T('home.installed'))}<div class="grp">${installed}</div>
      ${snap.pets.length ? agentPetsPart() : ''}
      ${sec(T('s.download'))}<div class="grp"><div class="r"><input class="in" id="s-ref" spellcheck="false" placeholder="${esc(T('s.fetchPlaceholder'))}"><button class="pbtn al" data-fetch ${fetching ? 'disabled' : ''}>${esc(fetchLabel)}</button></div>${note}</div>
      <div class="sec row-sec"><span class="ellip">${esc(listTitle)}</span><span class="grow"></span>${seg('gallery', [['popular', T('home.popular')], ['newest', T('home.newest')]], gallery.sort)}</div>
      ${gallery.error ? `<div class="note err">${esc(T('home.galleryError', { message: gallery.error }))}</div>` : ''}${none}
      <div class="cards">${cards}</div>${more}
      <div class="links"><span class="link" data-site="site">${esc(T('settings.browse'))} ›</span></div>`
  }

  // --- Her island's look (island.js drawIslandLook) ---------------------------------
  // By Windows' mode: a preset, or its parts one by one (none set: black).
  // Folded out under the island's options. Open, the settings are the
  // island itself, so a small one here shows the look (hovered, it opens).
  // Its look (islandTopLook, zhoupengjie/Wakuwaku#27): black by default in
  // either mode (the iPhone's, its faint line, white words); no 「云母毛玻璃」
  // where Windows' acrylic cannot go under it (nothing else would blur).
  const ITOP_DEFAULT = {
    dark: { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false },
    light: { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false },
  }
  const ITOP_PRESETS = {
    dark: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#242424', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#2c2c2c', op: 100, blur: 0, edge: 'ring', bare: false }],
    ],
    light: [
      ['black', { bg: '#000000', op: 100, blur: 0, edge: 'line', bare: false }],
      ['gray', { bg: '#f3f3f3', op: 100, blur: 0, edge: 'line', bare: false }],
      ['white', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'line', bare: false }],
      ['mica', { bg: '#f6f6f6', op: 62, blur: 40, edge: 'line', bare: false }],
      ['ring', { bg: '#fbfbfb', op: 100, blur: 0, edge: 'ring', bare: false }],
    ],
  }
  // The moods' colours on a light ground, as island.js has them (COLOR_LIGHT).
  const COLOR_DEEP = { idle: '#6e6e73', working: '#0067c0', waiting: '#b25e00', done: '#0f7b3f', review: '#7346c9', error: '#c42b3c' }
  // Grounds to pick for each mode: as dark (or light) as its words need.
  const ILOOK_SWATCHES = {
    dark: ['#000000', '#1c1c1c', '#202020', '#2c2c2c', '#3a3a3a', '#1f2733', '#2a2433', '#1f2d27', '#33271f'],
    light: ['#ffffff', '#fbfbfb', '#f3f3f3', '#e8e8e8', '#dddddd', '#eef3fb', '#f5effa', '#eef7f1', '#fbf3ec'],
  }
  let ilookOpen = false
  // The mode being set: '' follows Windows' own.
  let ilookMode = ''
  const winMode = () => (snap.winLook?.light ? 'light' : 'dark')
  const ilookFor = () => ilookMode || winMode()
  const ilookKey = () => 'islandTopLook'
  const ilookDefault = mode => ITOP_DEFAULT[mode]
  const ilookPresets = mode => ITOP_PRESETS[mode].filter(([id]) => id !== 'mica' || snap.plate === true)
  const ilookOf = mode => ({ ...ilookDefault(mode), ...(snap.settings[ilookKey()]?.[mode] || {}) })
  const sameLook = (a, b) => a.bg.toLowerCase() === b.bg.toLowerCase() && a.op === b.op && a.blur === b.blur && a.edge === b.edge && !!a.bare === !!b.bare
  const ilookPreset = mode => (ilookPresets(mode).find(([, p]) => sameLook(p, ilookOf(mode))) || ['custom'])[0]
  const ilookName = (mode, id) => T(id === 'custom' ? 's.ilookCustom' : id === 'black' ? 's.ilookP.top.black' : `s.ilookP.${mode}.${id}`)

  // A mode's look kept (its default kept as none); while a slider moves,
  // only shown here, kept once it is let go.
  function ilookSave(mode, look, keep = true) {
    const key = ilookKey()
    const all = { ...(snap.settings[key] || {}) }
    all[mode] = sameLook(look, ilookDefault(mode)) ? null : look
    if (keep) return patch({ [key]: all })
    snap.settings[key] = all
    draw()
  }

  // A colour's relative luminance (WCAG), as island.js judges a ground:
  // above 0.35, the words on it go dark.
  function luminance(hex) {
    const [r, g, b] = [1, 3, 5].map(i => {
      const v = parseInt(hex.slice(i, i + 2), 16) / 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }
  const inkDarkOn = hex => luminance(hex) > 0.35

  function toHsl(hex) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255)
    const max = Math.max(r, g, b)
    const min = Math.min(r, g, b)
    const l = (max + min) / 2
    const d = max - min
    const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1))
    const h = d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4
    return [h, s, l]
  }

  function fromHsl(h, s, l) {
    const c = (1 - Math.abs(2 * l - 1)) * s
    const x = c * (1 - Math.abs((h % 2) - 1))
    const [r, g, b] = [[c, x, 0], [x, c, 0], [0, c, x], [0, x, c], [x, 0, c], [c, 0, x]][Math.floor(h) % 6]
    const m = l - c / 2
    return `#${[r, g, b].map(v => Math.round(Math.min(1, Math.max(0, v + m)) * 255).toString(16).padStart(2, '0')).join('')}`
  }

  // A ground picked, any colour: the words on it go light or dark as it is
  // (island.js), so only a mid one reads poorly either way (between 0.18
  // and 0.36, under 4.5 to 1 with white words, and dark ones not yet):
  // moved out to the nearer side, its hue kept.
  function ilookSteer(hex) {
    hex = hex.toLowerCase()
    const y = luminance(hex)
    if (y <= 0.18 || y >= 0.36) return hex
    const [h, s, l] = toHsl(hex)
    const darker = y - 0.18 < 0.36 - y
    let [lo, hi] = darker ? [0, l] : [l, 1]
    // The lightness where it crosses out of the middle, a little past it.
    for (let i = 0; i < 24; i++) {
      const mid = (lo + hi) / 2
      const y2 = luminance(fromHsl(h, s, mid))
      if (darker) {
        if (y2 <= 0.175) lo = mid
        else hi = mid
      } else if (y2 >= 0.365) hi = mid
      else lo = mid
    }
    return fromHsl(h, s, darker ? lo : hi)
  }

  // The island as the look makes it: a pill at the top of a desktop; big,
  // it opens down under the pointer. Blurred only where Windows' acrylic
  // goes under it (island.js placePlate), open or not.
  function ilookPreview(mode, look, big = false) {
    const [r, g, b] = [1, 3, 5].map(i => parseInt(look.bg.slice(i, i + 2), 16))
    // Its words, and the moods' colours, as light or dark as its ground is.
    const inkDark = inkDarkOn(look.bg)
    const moods = inkDark ? COLOR_DEEP : COLOR
    const ring = moods[snap.now?.mood] || moods.working
    const line = inkDark ? 'rgba(0,0,0,0.08)' : 'rgba(255,255,255,0.07)'
    const edge = look.edge === 'ring' ? `0 0 0 1.5px ${ring}, 0 0 12px ${ring}59` : look.edge === 'none' ? '0 0 0 0 transparent' : `0 0 0 1px ${line}`
    const plate = snap.plate === true && look.op < 100 && look.blur > 0
    const blur = look.blur && plate ? `blur(${look.blur}px) saturate(1.25)` : 'none'
    const style = `--pv-bg:rgba(${r},${g},${b},${look.op / 100});--pv-blur:${blur};--pv-edge:${edge};--pv-ring:${ring}`
    const more = big ? `<span class="more"><span>${esc(T('s.ilookSample2'))}</span><span>${esc(T('s.ilookSample3'))}</span></span>` : ''
    return `<div class="ilook-pv ${mode}${inkDark ? ' ink-dark' : ''}${look.bare ? ' bare' : ''}${big ? ' big' : ''}${plate ? ' plate' : ''} top" style="${style}"><span class="cap"><span class="top"><i class="face"></i><span class="w">${esc(T('s.ilookSample'))}</span><span class="t">1:42</span></span>${more}</span></div>`
  }

  function ilookPanel() {
    const mode = ilookFor()
    const look = ilookOf(mode)
    const active = ilookPreset(mode)
    const note = T('s.ilookTop')
    const cards = ilookPresets(mode)
      .map(([id, p]) => `<button class="ilook-card${id === active ? ' on' : ''}" data-ilook-preset="${id}">${ilookPreview(mode, p)}<span>${esc(ilookName(mode, id))}</span></button>`)
      .join('')
    const swatches = ILOOK_SWATCHES[mode]
      .map(c => `<button class="swc${c === look.bg.toLowerCase() ? ' on' : ''}" data-ilook-bg="${c}" style="background:${c}" title="${c}"></button>`)
      .join('')
    const picker = `<span class="ilook-sw">${swatches}<input type="color" class="swc-in" data-ilook="bg" value="${esc(look.bg)}" title="${esc(T('s.ilookPick'))}"></span>`
    const range = (key, min, max, unit) => `<span class="ilook-rng"><input type="range" class="rng" data-ilook="${key}" min="${min}" max="${max}" step="1" value="${look[key]}"><span class="rv">${look[key]}${unit}</span></span>`
    // A blur only where Windows' acrylic can go under the island.
    const blurRow = snap.plate !== true ? '' : row(esc(T('s.ilookBlur')), esc(T('s.ilookBlurTopNote')), range('blur', 0, 60, 'px'))
    return `<div class="ilook">
        ${note ? `<div class="note">${esc(note)}</div>` : ''}
        <div class="ilook-top">${seg('ilookMode', [['dark', T('s.ilookDark')], ['light', T('s.ilookLight')]], mode)}<span class="grow"></span><button class="pbtn sm" data-ilook-reset ${sameLook(look, ilookDefault(mode)) ? 'disabled' : ''}>${esc(T('s.ilookReset'))}</button></div>
        ${mode === winMode() ? '' : `<div class="note">${esc(T('s.ilookOther'))}</div>`}
        ${ilookPreview(mode, look, true)}
        <div class="ilook-cards">${cards}</div>
        ${row(esc(T('s.ilookBg')), esc(T('s.ilookBgNote')), picker)}
        ${row(esc(T('s.ilookOp')), '', range('op', 20, 100, '%'))}
        ${blurRow}
        ${row(esc(T('s.ilookEdge')), '', seg('ilookEdge', ['none', 'line', 'ring'].map(e => [e, T(`s.ilookEdge.${e}`)]), look.edge))}
      </div>`
  }

  // A slider moved (shown here) or let go (kept); a colour picked, moved
  // out of the greys no words read on.
  function ilookInput(target, keep) {
    const mode = ilookFor()
    const key = target.dataset.ilook
    const value = key === 'bg' ? ilookSteer(target.value) : Number(target.value)
    if (key === 'bg' && value !== target.value.toLowerCase()) target.value = value
    ilookSave(mode, { ...ilookOf(mode), [key]: value }, keep)
  }

  // The fold's own row: how the island looks in Windows' mode now.
  function ilookRow() {
    const mode = winMode()
    const now = T('s.ilookNow', { mode: T(mode === 'dark' ? 's.ilookDark' : 's.ilookLight'), name: ilookName(mode, ilookPreset(mode)) })
    return `${row(esc(T('s.ilook')), esc(now), `<button class="pbtn sm" data-ilook-fold>${esc(T(ilookOpen ? 's.ilookFold' : 's.ilookAdjust'))}</button>`)}${ilookOpen ? ilookPanel() : ''}`
  }

  function pageLook() {
    const s = snap.settings
    return `${sec(T('home.display'))}
      <div class="grp">${row(esc(T('s.islandWidth')), esc(T('s.islandWidthNote')), seg('islandWidth', ['narrow', 'normal', 'wide'].map(w => [w, T(`s.width.${w}`)]), s.islandWidth || 'normal'))}${row(esc(T('s.oledShift')), esc(T('s.oledShiftNote')), sw('oledShift', s.oledShift))}${ilookRow()}</div>
      ${sec(T('s.her'))}<div class="grp">
        ${row(esc(T('menu.size')), '', seg('scale', SIZES.map(([name, scale]) => [scale, T(`menu.${name}`)]), s.scale))}
        ${row(esc(T('s.details')), esc(T('s.detailsNote')), sw('details', s.details !== false))}
        ${row(esc(T('s.walk')), esc(T('s.walkNote')), sw('walk', s.walk))}
        ${row(esc(T('s.look')), '', sw('look', s.look))}
      </div>
      ${part('pets', pagePets())}`
  }

  function pageAlerts() {
    const s = snap.settings
    const holds = HOLDS.map(h => [h, h === 'seen' ? T('s.holdSeen') : h >= 60 ? T('settings.minutes', { n: h / 60 }) : T('settings.seconds', { n: h })])
    const waits = WAITS.map(w => [w, w >= 60 ? T('settings.minutes', { n: Math.round(w / 60) }) : T('settings.seconds', { n: w })])
    const idles = IDLE_HIDES.map(m => [m, !m ? T('s.idleAlways') : m >= 60 ? T('s.paperHours', { n: m / 60 }) : T('s.minutesShort', { n: m })])
    const notify = s.notify || {}
    return `${sec(T('s.holdTitle'))}<div class="grp">${row(esc(T('s.hold')), '', seg('hold', holds, s.hold))}</div>
      ${sec(T('s.idleTitle'))}<div class="grp">${row(esc(T('s.idleHide')), esc(T('s.idleHideNote')), seg('idleHideMin', idles, s.idleHideMin))}</div>
      ${sec(T('settings.notify'))}<div class="grp">
        ${row(esc(T('settings.notifyWaiting')), '', sw('notify.waiting', notify.waiting))}
        ${row(esc(T('settings.notifyDone')), '', sw('notify.done', notify.done))}
        ${row(esc(T('settings.notifyError')), '', sw('notify.error', notify.error))}
        ${row(esc(T('settings.sound')), '', sw('sound', s.sound))}
      </div>
      ${sec(T('settings.prompts'))}<div class="grp">${row(esc(T('settings.promptWait')), esc(T('settings.promptWaitNote')), seg('promptWaitSec', waits, s.promptWaitSec))}</div>
      ${sec(T('s.quiet'))}<div class="grp">
        ${row(esc(T('menu.dnd')), esc(T('s.dndNote')), sw('dnd', s.dnd))}
        ${row(esc(T('settings.hideInFullscreen')), '', sw('hideInFullscreen', s.hideInFullscreen, snap.fullscreenAvailable ? '' : 'disabled'))}
        ${row(esc(T('s.clickAction')), esc(T('s.clickActionNote')), seg('clickAction', [['jump', T('s.clickJump')], ['terminal', T('s.clickTerminal')]], s.clickAction || 'jump'))}
        ${row(esc(T('s.lol')), esc(T('s.lolNote')), sw('lol', s.lol !== false))}
        ${row(esc(T('s.riotKey')), esc(T('s.riotKeyNote')), `<input class="in" id="s-riot-key" type="password" spellcheck="false" autocomplete="off" placeholder="RGAPI-…" value="${esc(s.riotApiKey || '')}">`)}
        ${row(esc(T('s.duel')), esc(T('s.duelNote', { key: s.duelKey || 'Alt+D' })), sw('duel', s.duel !== false))}
        ${row(esc(T('s.duelHand')), esc(T('s.duelHandNote')), `<input class="in" id="s-duel-hand" spellcheck="false" autocomplete="off" placeholder="${esc(T('s.duelHandHint'))}" value="${esc(s.duelHand || '')}">`)}
        <div class="r"><div class="grow"><div>${esc(T('s.duelDeck'))}</div><div class="d">${esc(T('s.duelDeckNote'))}</div></div><button class="pbtn sm" data-duel-deck>${esc(T('deck.button'))}</button></div>
      </div>`
  }

  // The plugins page as rows, in the order the person put them in: the
  // built-in ones, the plugins she runs (with what their widgets say), and
  // the widgets of anyone else's scripts.
  function widgetRows() {
    const rank = key => {
      const i = (snap.settings.widgetOrder || []).indexOf(key)
      return i < 0 ? Infinity : i
    }
    const widgets = snap.widgets || []
    const rows = [
      ...widgets.filter(w => w.builtIn).map(w => ({ key: w.id, widget: w })),
      ...(snap.plugins || []).map(p => ({ key: p.id, plugin: p, mine: widgets.filter(w => w.plugin === p.id) })),
      ...widgets.filter(w => !w.builtIn && !w.plugin).map(w => ({ key: w.id, widget: w })),
    ]
    return rows.map((r, i) => ({ ...r, i })).sort((x, y) => rank(x.key) - rank(y.key) || x.i - y.i)
  }

  // What a plugin she runs says under its name: what it does, or why it
  // shows nothing.
  function pluginNote(p, mine) {
    if (p.state === 'needs') return { text: T('pl.needs', { what: T(`pl.${p.id}.${p.missing}`) }), bad: true }
    if (p.state === 'stopped') {
      // No PowerShell would start it: not the plugin's fault (#28).
      const why = p.noShell ? T('pl.noShell', { line: p.said || '' }) : p.said ? T('pl.stopped', { line: p.said }) : T('pl.stoppedQuiet')
      return { text: p.retryIn ? `${why} · ${T('pl.retry', { n: p.retryIn })}` : why, bad: true }
    }
    if (p.state === 'running' && !mine.length) return p.said ? { text: T('pl.trouble', { line: p.said }), bad: true } : { text: T('pl.starting') }
    return { text: T(`w.ex.${p.id}.note`) }
  }

  // A plugin's settings, folded out under it: what is being typed stays.
  function pluginFields(p) {
    const fields = p.params.map(f => {
      const key = `${p.id}.${f.name}`
      const label = esc(T(`pl.${key}`))
      if (f.kind === 'flag') return `<div class="fr"><span class="fl">${label}</span><span class="grow"></span>${sw('pf:' + key, f.value === true)}</div>`
      const value = key in drafts ? drafts[key] : f.value || ''
      return `<label class="fr"><span class="fl">${label}${f.required ? ' *' : ''}</span><input class="in" data-field="${esc(key)}" value="${esc(value)}" placeholder="${esc(T(`pl.${key}.hint`))}" spellcheck="false"${f.kind === 'number' ? ' inputmode="decimal"' : ''}></label>`
    })
    return `<div class="pf">${fields.join('')}</div>`
  }

  // The monitor's settings, folded out under it: its parts on or off, and
  // how often it reads (1 to 10 seconds).
  function monitorFields() {
    const m = snap.settings.monitor || {}
    const parts = ['cpu', 'mem', 'net', 'battery'].map(k => `<div class="fr"><span class="fl">${esc(T(`mon.${k}`))}</span><span class="grow"></span>${sw('mon:' + k, m[k] !== false)}</div>`)
    const every = m.every || 2
    const pace = `<div class="fr"><span class="fl">${esc(T('mon.every'))}</span><span class="grow"></span><button class="pbtn sm" data-every="-1" ${every <= 1 ? 'disabled' : ''}>−</button><span class="every">${esc(T('settings.seconds', { n: every }))}</span><button class="pbtn sm" data-every="1" ${every >= 10 ? 'disabled' : ''}>+</button></div>`
    const pin = `<div class="fr"><span class="fl">${esc(T('mon.pin'))}</span><span class="d grow">${esc(T('mon.pinNote'))}</span>${sw('mon:pin', m.pin !== false)}</div>`
    return `<div class="pf">${pin}${parts.join('')}${pace}</div>`
  }

  // Plugins: built in, run by her, or anyone's script, each with its switch
  // (and a step up); how they take turns, whether they may open the island,
  // waku for the terminal, and how to write one.
  function pageWidgets() {
    const s = snap.settings
    const left = ms => (ms >= 60000 ? T('settings.minutes', { n: Math.round(ms / 60000) }) : T('settings.seconds', { n: Math.max(1, Math.round(ms / 1000)) }))
    const rows = widgetRows()
      .map((r, i) => {
        const up = i > 0 ? `<button class="pbtn sm" data-wup="${esc(r.key)}" title="${esc(T('w.up'))}">↑</button>` : ''
        if (r.plugin) {
          const p = r.plugin
          const says = r.mine.map(w => Widgets.words(lang, w, s.details !== false)).map(({ label, value }) => [label, value].filter(Boolean).join(' ')).join(' · ')
          const note = pluginNote(p, r.mine)
          const color = (r.mine[0] && r.mine[0].color) || (p.on ? '#5e9bff' : COLOR.idle)
          const isOpen = openPlugin === p.id
          const more = p.params.length ? `<button class="pbtn sm" data-pexp="${esc(p.id)}">${esc(T(isOpen ? 'pl.fold' : 'pl.settings'))}</button>` : ''
          return `<div class="r" data-part="widgets:${esc(r.key)}"><span class="wi" style="color:${esc(color)}">${Widgets.icon(p.icon)}</span><div class="grow"><div class="ellip">${esc(T(`w.ex.${p.id}`))}<span class="d"> ${esc(says)}</span></div><div class="d${note.bad ? ' bad' : ''}">${esc(note.text)}</div></div>${more}${up}${sw('pl:' + p.id, p.on)}</div>${isOpen ? pluginFields(p) : ''}`
        }
        const w = r.widget
        const { label, value } = Widgets.words(lang, w, s.details !== false)
        // A built-in one says what it shows; on, with nothing read, says so.
        const builtIn = w.builtIn && [T('w.builtIn'), T(`w.about.${w.id}`), w.on && w.value == null ? T('w.unread') : ''].filter(Boolean).join(' · ')
        const from = builtIn || [T('w.script', { time: left(w.leftMs || 0) }), w.private ? T('w.private') : ''].filter(Boolean).join(' · ')
        // The monitor: its readings as icon and number, its settings folded under it.
        const says = Widgets.partsOf(w) ? ` ${Widgets.partsHTML(w)}` : `<span class="d"> ${esc(value)}</span>`
        const isMonitor = w.id === 'monitor'
        const more = isMonitor ? `<button class="pbtn sm" data-pexp="monitor">${esc(T(openPlugin === 'monitor' ? 'pl.fold' : 'pl.settings'))}</button>` : ''
        return `<div class="r" data-part="widgets:${esc(r.key)}"><span class="wi" style="color:${esc(w.color || COLOR.idle)}">${Widgets.icon(w.icon)}</span><div class="grow"><div class="ellip">${esc(label)}${says}</div><div class="d">${esc(from)}</div></div>${more}${up}${sw('w:' + w.id, w.on)}</div>${isMonitor && openPlugin === 'monitor' ? monitorFields() : ''}`
      })
      .join('')
    const spins = SPINS.map(n => [n, n ? T('settings.seconds', { n }) : T('w.spinOff')])
    const waku = `powershell -ExecutionPolicy Bypass -File "${snap.waku}" npm run build`
    const example = `Invoke-RestMethod -Method Post http://127.0.0.1:${snap.port}/widget -ContentType application/json -Body '{"id":"hello","label":"Hello","value":"42","icon":"star"}'`
    return `${sec(T('w.section'))}<div class="grp"><div class="note">${esc(T('w.note'))}</div>${rows}</div>
      ${sec(T('w.show'))}<div class="grp">
        ${row(esc(T('w.spin')), '', seg('widgetSpin', spins, s.widgetSpin ?? 8))}
        ${row(esc(T('w.nudge')), esc(T('w.nudgeNote')), sw('widgetNudge', s.widgetNudge !== false))}
      </div>
      ${sec(T('w.terminal'))}<div class="grp"><div class="note">${esc(T('w.ex.waku.note'))}</div>
        <div class="r"><span class="cmd mono">${esc(waku)}</span><button class="pbtn" data-copy="${esc(waku)}">${esc(T('home.copy'))}</button></div>
      </div>
      ${sec(T('w.write'))}<div class="grp"><div class="note">${esc(T('w.writeNote'))}</div>
        <div class="r"><span class="cmd mono">${esc(example)}</span><button class="pbtn" data-copy="${esc(example)}">${esc(T('home.copy'))}</button></div>
      </div>`
  }

  function pageConnect() {
    const conn = { plugin: 'home.connPlugin', hooks: 'home.connHooks', both: 'home.connBoth', none: 'home.connNone' }[snap.connection] || 'home.connNone'
    const isOk = snap.connection === 'plugin' || snap.connection === 'hooks'
    const sessions = (snap.sessions || []).length
    const hooksWord = {
      ok: 'settings.hooksOk',
      missing: 'settings.hooksMissing',
      stale: 'settings.hooksStale',
      partial: 'settings.hooksPartial',
      httpOnly: 'settings.hooksHttpOnly',
      unreadable: 's.hooksUnreadable',
    }[snap.hooks]
    const hooksButtons =
      snap.hooks === 'missing'
        ? `<button class="pbtn" data-hooks="install">${esc(T('settings.install'))}</button>`
        : snap.hooks === 'unreadable'
          ? ''
          : `<button class="pbtn" data-hooks="install">${esc(T(snap.hooks === 'ok' || snap.hooks === 'httpOnly' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn no" data-hooks="remove">${esc(T('settings.remove'))}</button>`
    // Codex: hooks in its hooks.json, which it runs only once trusted (/hooks):
    // until an event came, they may be waiting for that.
    const codex = snap.codex || {}
    // upgradable: all works; Codex was updated since and runs background hooks now.
    const codexIn = ['ok', 'stale', 'upgradable', 'partial'].includes(codex.hooks)
    const codexConn =
      codex.hooks === 'ok' && codex.seen
        ? 's.codexConnOk'
        : codex.hooks === 'ok'
          ? 's.codexConnTrust'
          : codex.hooks === 'upgradable'
            ? 's.codexConnUpgrade'
            : codexIn
              ? 's.codexConnRepair'
              : 's.codexConnNone'
    const codexColor = codexConn === 's.codexConnOk' ? COLOR.done : codex.hooks === 'upgradable' ? COLOR.working : codexIn ? COLOR.waiting : COLOR.idle
    const codexWord = {
      ok: 'settings.hooksOk',
      missing: 'settings.hooksMissing',
      stale: 'settings.hooksStale',
      upgradable: 's.codexUpgradable',
      partial: 'settings.hooksPartial',
      unreadable: 's.codexUnreadable',
      absent: 's.codexAbsent',
    }[codex.hooks]
    const codexButtons =
      codex.hooks === 'missing'
        ? `<button class="pbtn" data-codex="install">${esc(T('settings.install'))}</button>`
        : codexIn
          ? `<button class="pbtn" data-codex="install">${esc(T(codex.hooks === 'ok' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn no" data-codex="remove">${esc(T('settings.remove'))}</button>`
          : ''
    // Antigravity: hooks in ~/.gemini/config/hooks.json, a block of ours.
    const agy = snap.agy || {}
    const agyConn = agyIn() && agy.seen ? 's.agyConnOk' : agyIn() ? 's.agyConnWait' : 's.agyConnNone'
    const agyColor = agyConn === 's.agyConnOk' ? COLOR.done : agyIn() ? COLOR.waiting : COLOR.idle
    const agyWord = {
      ok: 'settings.hooksOk',
      missing: 'settings.hooksMissing',
      stale: 'settings.hooksStale',
      off: 's.agyOff',
      unreadable: 's.agyUnreadable',
      absent: 's.agyAbsent',
    }[agy.hooks]
    const agyButtons =
      agy.hooks === 'missing'
        ? `<button class="pbtn" data-agy="install">${esc(T('settings.install'))}</button>`
        : ['ok', 'stale', 'off'].includes(agy.hooks)
          ? `<button class="pbtn" data-agy="install">${esc(T(agy.hooks === 'ok' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn no" data-agy="remove">${esc(T('settings.remove'))}</button>`
          : ''
    // Grok Build: a hooks file of ours in ~/.grok/hooks, all of it.
    const grok = snap.grok || {}
    const grokConn = grokIn() && grok.seen ? 's.grokConnOk' : grokIn() ? 's.grokConnWait' : 's.grokConnNone'
    const grokColor = grokConn === 's.grokConnOk' ? COLOR.done : grokIn() ? COLOR.waiting : COLOR.idle
    const grokWord = {
      ok: 'settings.hooksOk',
      missing: 'settings.hooksMissing',
      stale: 'settings.hooksStale',
      unreadable: 's.grokUnreadable',
      absent: 's.grokAbsent',
    }[grok.hooks]
    const grokButtons =
      grok.hooks === 'missing'
        ? `<button class="pbtn" data-grok="install">${esc(T('settings.install'))}</button>`
        : ['ok', 'stale', 'unreadable'].includes(grok.hooks)
          ? `<button class="pbtn" data-grok="install">${esc(T(grok.hooks === 'ok' ? 'settings.reinstall' : 'settings.repair'))}</button><button class="pbtn no" data-grok="remove">${esc(T('settings.remove'))}</button>`
          : ''
    return `${sec(T('s.status'))}<div class="grp"><div class="r"><span class="dot" style="background:${isOk ? COLOR.done : snap.connection === 'both' ? COLOR.waiting : COLOR.idle}"></span><div class="grow">${esc(T(conn))}</div><span class="d">${esc(T('s.sessionsOnline', { n: sessions }))}</span></div>
        <div class="r"><span class="dot" style="background:${codexColor}"></span><div class="grow">${esc(T(codexConn))}</div></div>
        <div class="r"><span class="dot" style="background:${agyColor}"></span><div class="grow">${esc(T(agyConn))}</div></div>
        <div class="r"><span class="dot" style="background:${grokColor}"></span><div class="grow">${esc(T(grokConn))}</div></div></div>
      ${sec(T('home.plugin'))}<div class="grp"><div class="note">${esc(T('home.pluginWhy'))}</div>
        ${snap.pluginCommands.map(c => `<div class="r"><span class="cmd mono">${esc(c)}</span><button class="pbtn" data-copy="${esc(c)}">${esc(T('home.copy'))}</button></div>`).join('')}
      </div>
      ${sec(T('home.advanced'))}<div class="grp">
        <div class="note">${esc(T('home.advancedWhy'))}</div>
        <div class="r"><div class="grow"><div>${esc(T('settings.hooks'))}: ${esc(T(hooksWord))}</div><div class="d mono ellip">${esc(snap.settingsFile)}</div></div>${hooksButtons}</div>
      </div>
      ${sec(T('s.codex'))}<div class="grp">
        <div class="note">${esc(T('s.codexWhy'))}</div>
        <div class="r"><div class="grow"><div>${esc(T('settings.hooks'))}: ${esc(T(codexWord))}</div><div class="d mono ellip">${esc(codex.file)}</div></div>${codexButtons}</div>
        ${codex.hooks === 'upgradable' ? `<div class="note">${esc(T('s.codexUpgradeNote'))}</div>` : ''}
        ${codexIn && codex.hooks !== 'upgradable' && !codex.seen ? `<div class="note">${esc(T('s.codexTrust'))}</div>` : ''}
        ${codex.hooks !== 'absent' && !codex.async ? `<div class="note">${esc(T('s.codexOld', { version: codex.version }))}</div>` : ''}
      </div>
      ${sec(T('s.agy'))}<div class="grp">
        <div class="note">${esc(T('s.agyWhy'))}</div>
        <div class="r"><div class="grow"><div>${esc(T('settings.hooks'))}: ${esc(T(agyWord))}</div><div class="d mono ellip">${esc(agy.file || '')}</div></div>${agyButtons}</div>
      </div>
      ${sec(T('s.grok'))}<div class="grp">
        <div class="note">${esc(T('s.grokWhy'))}</div>
        <div class="r"><div class="grow"><div>${esc(T('settings.hooks'))}: ${esc(T(grokWord))}</div><div class="d mono ellip">${esc(grok.file || '')}</div></div>${grokButtons}</div>
      </div>
      ${sec(T('settings.about'))}<div class="grp">
        <div class="r"><div class="grow">${esc(T('s.version', { version: snap.version }))}</div><span class="link" data-site="repo">${esc(T('home.repo'))} ›</span></div>
        <div class="note">${esc(T('home.credits'))}</div>
      </div>`
  }

  // Her updates (updates.rs): this version and the latest, its notes and
  // Update, or a look now; how an update goes (downloaded, checked, waiting
  // for a prompt's answer, restarting) or went wrong; looked for by itself
  // or not; downloaded from GitHub or Gitee first. A build run from the
  // source tree is never looked for.
  function pageUpdate() {
    const u = snap.update || {}
    const version = T('s.updateVersion', { version: u.current || snap.version })
    if (!u.may) return `${sec(T('s.update'))}<div class="grp">${row(esc(version), esc(T('s.updateSource')), '')}</div>`
    const latest = u.latest
    const newer = !!latest?.newer
    const mb = n => (n / 1048576).toFixed(1)
    let state = ''
    if (u.stage === 'download') state = T('s.updateDownloading', { from: u.from || '', pct: u.total ? Math.min(100, Math.floor((100 * u.got) / u.total)) : 0, got: mb(u.got || 0), total: mb(u.total || 0) })
    else if (u.stage) state = T(`s.updateStage.${u.stage}`)
    else if (u.checking) state = T('s.updateChecking')
    else if (u.error) state = T(`s.updateError.${u.error.code}`, { detail: u.error.detail || '' })
    else if (newer && !latest.canInstall) state = T('s.updateError.noDigest')
    else if (newer) state = T('s.updateNewer', { version: latest.version })
    else if (latest) state = T('s.updateLatest')
    else if (u.checkedAt) state = T('s.updateNone')
    const busy = !!u.stage || !!u.checking
    const buttons = newer
      ? `<span class="btns"><button class="pbtn sm" data-update="page">${esc(T('s.updateNotes'))}</button><button class="pbtn sm al" data-update="update" ${busy || !latest.canInstall ? 'disabled' : ''}>${esc(T('s.updateInstall'))}</button></span>`
      : `<button class="pbtn sm" data-update="check" ${busy ? 'disabled' : ''}>${esc(T('s.updateCheck'))}</button>`
    const title = newer ? `${version}${T('s.updateLatestIs', { version: latest.version })}` : version
    const when = u.checkedAt
      ? T('s.updateCheckedAt', { time: new Date(u.checkedAt).toLocaleString(LOCALES[lang] || 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' }) })
      : ''
    return `${sec(T('s.update'))}<div class="grp">
        ${row(esc(title), esc(state), buttons)}
        ${row(esc(T('s.updateAuto')), esc([T('s.updateAutoNote'), when].filter(Boolean).join(' · ')), sw('autoUpdateCheck', snap.settings.autoUpdateCheck !== false))}
        ${row(esc(T('s.updateFrom')), esc(T('s.updateFromNote')), seg('updateFrom', [['auto', T('s.updateFromAuto')], ['gitee', 'Gitee'], ['github', 'GitHub']], snap.settings.updateFrom || 'auto'))}
      </div>`
  }

  // Her crash reports (crash.rs): asked about once (yes from now on, or no),
  // the report shown as it would be sent; sent by themselves or not; those
  // waiting, sending, what went wrong, the last one's issue. Nothing while
  // there is nowhere to send to (a build from the source tree, no relay yet).
  function pageCrash() {
    const c = snap.crash || {}
    if (!c.can) return ''
    const when = ms => new Date(ms).toLocaleString(LOCALES[lang] || 'en-US', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' })
    const ask =
      c.mode === 'ask' && c.newest
        ? `<div class="grp">
            <div class="note">${esc(T('s.crashAsk', { when: when(c.newest.at) }))}</div>
            ${crashShown ? `<pre class="crash-text mono">${esc(c.newest.text)}</pre>` : ''}
            <div class="r"><button class="pbtn sm" data-crash-show>${esc(T(crashShown ? 's.crashHide' : 's.crashShow'))}</button><div class="grow"></div><span class="btns"><button class="pbtn sm" data-crash="no">${esc(T('s.crashNo'))}</button><button class="pbtn sm al" data-crash="yes">${esc(T('s.crashYes'))}</button></span></div>
          </div>`
        : ''
    let state = ''
    if (c.sending) state = T('s.crashSending')
    else if (c.mode === 'auto' && c.waiting && c.error) state = T('s.crashError', { n: c.waiting, detail: c.error })
    else if (c.mode === 'auto' && c.waiting) state = T('s.crashWaiting', { n: c.waiting })
    else if (c.last?.at) state = T('s.crashSent', { when: when(c.last.at), n: c.last.number ? `#${c.last.number}` : '' })
    const retry = c.mode === 'auto' && c.waiting && !c.sending ? `<button class="pbtn sm" data-crash="send">${esc(T('s.crashSend'))}</button>` : ''
    const open = c.last?.url ? `<button class="pbtn sm" data-crash="open">${esc(T('s.crashOpen'))}</button>` : ''
    return `${sec(T('s.crash'))}${ask}<div class="grp">
        ${row(esc(T('s.crashAuto')), esc([T('s.crashAutoNote'), state].filter(Boolean).join(' · ')), `${retry}${open}${sw('crashReports', c.mode === 'auto')}`)}
      </div>`
  }

  // --- The welcome: Claude Code, Codex, Antigravity and Grok connected, at the top of General ---
  // While nothing is connected, and on the first open after one is (until
  // she is set up, onboarded): a card with a button for each of the three on
  // this computer, as equals (whichever one uses, or all), in a row: its name,
  // and under it how it stands; under them what each still needs, or why it
  // could not be written; what the buttons write; Claude Code's plugin the
  // other way. Connected (a session of any seen), one row, and the others on
  // this computer offered under it. Managing the connections stays in the
  // page's Connect part. (The appearance session's spec, 2026-10-13.)
  let connecting = ''
  // Why each could not be written: { error, kind } (locked, invalid, path, other).
  const failed = {}
  let pluginOpen = false
  // "Not now" said (connectHintOff: the welcome's 以后再说, the island's
  // 不用提醒) and nothing connected: the welcome folded to one row at the
  // page's top; opened again by its 去连接, or by asking for connecting.
  let welcomeOpen = false
  const TOOL = { claude: 'Claude Code', codex: 'Codex', agy: 'Antigravity', grok: 'Grok' }
  // What each one's buttons say they are for (the handlers below).
  const ACT = { claude: 'data-hooks', codex: 'data-codex', agy: 'data-agy', grok: 'data-grok' }
  const codexIn = () => ['ok', 'stale', 'upgradable', 'partial'].includes(snap.codex?.hooks)
  const claudeIn = () => snap.connection !== 'none'
  const agyIn = () => ['ok', 'stale'].includes(snap.agy?.hooks)
  const grokIn = () => ['ok', 'stale'].includes(snap.grok?.hooks)
  const anyIn = () => claudeIn() || codexIn() || agyIn() || grokIn()
  const welcomeFolded = () => snap.settings.connectHintOff === true && !welcomeOpen && !anyIn()
  const showWelcome = () => !anyIn() || !snap.settings.onboarded

  // The ones on this computer, in this order: Claude Code by its folder (or
  // connected), Codex, Antigravity and Grok by theirs. None found: all (it
  // cannot be told).
  function toolsHere() {
    const all = ['claude', 'codex', 'agy', 'grok']
    const here = all.filter(tool => (tool === 'claude' ? snap.claudeHere !== false || claudeIn() : snap[tool]?.hooks !== 'absent'))
    return here.length ? here : all
  }

  // Their names as one says them: 「A、B 或 C」, "A, B and C".
  const toolNames = (tools, kind = 'or') => nameList(lang, tools.map(tool => TOOL[tool]), kind)

  // How one stands: busy, seen (a session's event came, or the plugin in),
  // set (written, no session yet: Codex's to be trusted first), failed, none.
  function toolState(tool) {
    if (connecting === tool) return 'busy'
    if (tool === 'claude' && claudeIn()) return snap.claudeSeen || snap.connection === 'plugin' ? 'seen' : 'set'
    if (tool === 'codex' && codexIn()) return snap.codex?.seen ? 'seen' : 'set'
    if (tool === 'agy' && agyIn()) return snap.agy?.seen ? 'seen' : 'set'
    if (tool === 'grok' && grokIn()) return snap.grok?.seen ? 'seen' : 'set'
    return failed[tool] ? 'failed' : 'none'
  }

  // Windows' default accent for the accent-filled buttons: its light shade
  // on the dark settings, its dark one on the light.
  function accentButton() {
    return winMode() === 'light' ? '--acc:#0067c0;--acc-ink:#fff' : '--acc:#4cc2ff;--acc-ink:#000'
  }

  // One's button: its name, and under it how it stands; set up and
  // connected in green, a step left (Codex) in amber, and those do nothing.
  function toolButton(tool) {
    const state = toolState(tool)
    const name = `<span class="wt-name">${esc(TOOL[tool])}</span>`
    const line = text => `<span class="wt-state">${text}</span>`
    if (state === 'busy') return `<button class="wc-tool" disabled>${name}${line(`<span class="spin"></span>${esc(T('wc.busy'))}`)}</button>`
    if (state === 'seen') return `<button class="wc-tool done" disabled>${name}${line(esc(T('wc.toolOn')))}</button>`
    if (state === 'set' && tool === 'codex') return `<button class="wc-tool warn" disabled>${name}${line(esc(T('wc.toolStep')))}</button>`
    if (state === 'set') return `<button class="wc-tool done" disabled>${name}${line(esc(T('wc.toolSet')))}</button>`
    return `<button class="wc-tool" ${ACT[tool]}="install">${name}${line(esc(T(state === 'failed' ? 'wc.toolRetry' : 'wc.toolGo')))}</button>`
  }

  // The file each one's hooks go in, as read (~ for the home folder), and its name.
  const toolFile = tool =>
    ({ claude: snap.settingsShown || snap.settingsFile, codex: snap.codex?.fileShown || snap.codex?.file, agy: snap.agy?.fileShown || snap.agy?.file, grok: snap.grok?.fileShown || snap.grok?.file })[tool] || ''

  // Under the buttons: what each still needs (Claude Code, Antigravity and
  // Grok where it was written, and undone), or why it could not be written.
  function toolNote(tool) {
    const state = toolState(tool)
    const mono = text => `<span class="mono">${esc(text)}</span>`
    const file = toolFile(tool)
    if (['claude', 'agy', 'grok'].includes(tool) && state === 'set') {
      // Grok's file is ours alone: made, nothing to back up.
      const backed = { claude: snap.settingsBackup, agy: snap.agy?.backup }[tool]
      const where = backed ? T('wc.written', { path: mono(file), backup: mono(`${file.split(/[\\/]/).pop()}.wakuwaku.bak`) }) : T(tool === 'grok' ? 'wc.createdOurs' : 'wc.created', { path: mono(file) })
      return `<div class="wc-tnote"><div>${T(`wc.${tool}Left`)}</div><div class="wc-where">${where}<span class="link" ${ACT[tool]}="remove">${esc(T('wc.undo'))}</span></div></div>`
    }
    if (tool === 'codex' && state === 'set') return `<div class="wc-tnote warn">${T('wc.codexLeft')}</div>`
    if (state !== 'failed') return ''
    const { error, kind, folder } = failed[tool]
    // Her folder's path has a space, which agy can't run: not a file not
    // written, but where she is (the appearance session's words).
    if (kind === 'path') return `<div class="wc-err"><div><b>${esc(T('wc.errConnect', { name: TOOL[tool] }))}</b></div><div>${esc(T('wc.way.path', { folder: folder || '' }))}</div></div>`
    const way = `${T(`wc.way.${kind}`)}${T(tool === 'claude' ? 'wc.wayPlugin' : 'wc.wayEnd')}`
    return `<div class="wc-err"><div><b>${esc(T('wc.errLead', { name: TOOL[tool], file: { claude: 'settings.json', grok: 'wakuwaku.json' }[tool] || 'hooks.json' }))}</b></div>
        <div class="mono">${esc(file)}</div><div>${esc(error)}</div><div>${esc(way)}</div></div>`
  }

  function welcomeBlock() {
    const tools = toolsHere()
    if (welcomeFolded()) {
      return `<div class="grp"><div class="r"><span class="dot" style="background:${COLOR.idle}"></span><div class="grow d">${esc(T('wc.notYet', { list: toolNames(tools) }))}</div><span class="link" data-welcome-open>${esc(T('hint.go'))} ›</span></div></div>`
    }
    const seen = tools.filter(tool => toolState(tool) === 'seen')
    // Connected: one row, this once; the others on this computer not
    // written yet offered under it.
    if (seen.length) {
      const others = tools.filter(tool => ['none', 'failed', 'busy'].includes(toolState(tool)))
      const offer = tool =>
        toolState(tool) === 'busy'
          ? `<button class="pbtn sm" disabled><span class="spin"></span>${esc(TOOL[tool])}</button>`
          : `<button class="pbtn sm" ${ACT[tool]}="install">${esc(TOOL[tool])}</button>`
      const more = others.length ? `<div class="wc-also"><span class="d">${esc(T('wc.also'))}</span>${others.map(offer).join('')}</div>` : ''
      const sessions = (snap.sessions || []).length
      return `<div class="grp"><div class="r"><span class="dot" style="background:${COLOR.done}"></span><div class="grow"><div>${esc(T('wc.onList', { list: toolNames(seen, 'and') }))}</div>
          <div class="d">${esc(T(sessions === 1 ? 'wc.online1' : 'wc.online', { n: sessions }))} · <span class="link" data-goto="connect">${esc(T('wc.manage'))}</span></div>${more}</div></div></div>`
    }
    const many = tools.length > 1
    const pet = (snap.pets || []).find(p => p.id === snap.settings.pet)
    const face = pet ? `<span class="wc-face">${thumb(40, pet.url, pet.version)}</span>` : ''
    // A nudge to approve only where one of them can ask on her (Antigravity
    // and Grok can not: Grok's is answered in Grok).
    const sub = `${T('wc.sub1')}${tools.some(tool => tool !== 'agy' && tool !== 'grok') ? T('wc.subAsk') : ''}${T('wc.sub2')}${many ? T('wc.subMany') : ''}`
    const safe = [many ? 'wc.safeMany' : `wc.safe.${tools[0]}`, 'wc.safe2', 'wc.safe3'].map(k => `<div><span class="fi">\uE73E</span><span>${T(k)}</span></div>`).join('')
    // Claude Code's plugin, while it is here and not written.
    const commands = snap.pluginCommands || []
    const plugBody = pluginOpen
      ? `<div class="wc-plugbody"><div class="d">${esc(T('wc.plugHow'))}</div>${commands.map(c => `<div class="wc-cmd mono">${esc(c)}</div>`).join('')}
          <div class="wc-copy"><button class="pbtn" data-copy="${esc(commands.join('\n'))}" data-label="${esc(T('wc.copyBoth'))}">${esc(T('wc.copyBoth'))}</button><span class="d">${esc(T('wc.plugWake'))}</span></div></div>`
      : ''
    const plugin =
      tools.includes('claude') && ['none', 'failed', 'busy'].includes(toolState('claude'))
        ? `<div class="wc-div"></div>
        <button class="wc-plug${pluginOpen ? ' open' : ''}" data-plugin-toggle><span class="fi chev">\uE76C</span><span>${esc(T('wc.plug'))}</span><span class="d">${esc(T('wc.plugNote'))}</span></button>${plugBody}`
        : ''
    // Nothing connected: "not now" at its foot (as the island's 不用提醒).
    const later = !anyIn() ? `<button class="wc-later" data-welcome-later>${esc(T('wc.later'))}</button>` : ''
    return `<div class="wcard" style="${accentButton()}">
        <div class="wc-head">${face}<div class="grow"><div class="wc-title">${esc(T('wc.title', { list: toolNames(tools) }))}</div><div class="wc-sub">${esc(sub)}</div></div></div>
        <div class="wc-lead">${esc(T(many ? 'wc.leadMany' : 'wc.leadOne'))}</div>
        <div class="wc-tools" style="grid-template-columns:repeat(${tools.length},minmax(0,1fr))">${tools.map(toolButton).join('')}</div>${tools.map(toolNote).join('')}
        <div class="wc-safe">${safe}</div>${plugin}${later}
      </div>`
  }

  // Phone access (mobile.rs): on or off, where it is listened for, the QR
  // code of the page's address with its token; ntfy's pushes.
  function pageMobile() {
    const s = snap.settings
    const m = snap.mobile || {}
    const bind = s.mobileBind || 'tailscale'
    const isCustom = mobileCustom || (bind !== 'tailscale' && bind !== '0.0.0.0')
    const ntfy = s.ntfy || {}
    const problem = !s.mobile
      ? ''
      : m.problem === 'noTailscale'
        ? T('s.mobileNoTailscale')
        : m.problem === 'badAddress'
          ? T('s.mobileBadAddress')
          : m.problem
            ? T('s.mobileBindFailed', { why: m.problem.replace(/^bind: /, '') })
            : ''
    const where = s.mobile && m.listening ? T('s.mobileListening', { at: `${m.listening}:${snap.port}` }) : ''
    const seen = m.seenAgo == null ? T('s.mobileNeverSeen') : m.seenAgo < 60000 ? T('s.mobileSeenNow') : T('s.mobileSeenAgo', { time: clock(m.seenAgo) })
    let qr = ''
    if (s.mobile && m.url) {
      try {
        qr = `<div class="qr">${window.QR.svg(m.url)}</div>`
      } catch {
        qr = ''
      }
    }
    const tried = pushNote ? `<div class="d ${pushNote.ok ? '' : 'bad'}">${esc(pushNote.ok ? T('s.ntfySent') : T('s.ntfyFailed', { why: pushNote.error || '' }))}</div>` : ''
    return `${sec(T('s.mobile'))}<div class="grp">
        ${row(esc(T('s.mobileOn')), esc(T('s.mobileOnNote')), sw('mobile', s.mobile))}
        ${
          s.mobile
            ? `${row(esc(T('s.mobileBind')), esc(T('s.mobileBindNote')), seg('mobileBind', [['tailscale', 'Tailscale'], ['0.0.0.0', T('s.mobileAll')], ['custom', T('s.mobileCustom')]], isCustom ? 'custom' : bind))}
        ${isCustom ? row(esc(T('s.mobileAddress')), '', `<input class="in" id="s-mobile-ip" spellcheck="false" autocomplete="off" placeholder="192.168.1.20" value="${esc(isCustom && bind !== 'tailscale' && bind !== '0.0.0.0' ? bind : '')}">`) : ''}
        <div class="r"><div class="grow"><div class="${problem ? 'bad' : ''}">${esc(problem || where)}</div><div class="d">${esc(seen)}</div></div></div>
        ${
          qr
            ? `<div class="r qr-row">${qr}<div class="grow"><div class="d">${esc(T('s.mobileScan'))}</div><div class="r"><span class="cmd mono">${esc(m.url)}</span></div><div class="r"><button class="pbtn sm" data-copy="${esc(m.url)}">${esc(T('home.copy'))}</button><button class="pbtn sm no" data-mobile="token">${esc(T('s.mobileNewToken'))}</button></div></div></div>`
            : ''
        }
        ${row(esc(T('s.mobileUrl')), esc(T('s.mobileUrlNote')), `<input class="in" id="s-mobile-url" spellcheck="false" autocomplete="off" placeholder="https://pc.tailnet.ts.net" value="${esc(s.mobileUrl || '')}">`)}`
            : ''
        }
      </div>
      ${s.mobile ? phoneBlock() : ''}
      ${sec(T('s.ntfy'))}<div class="grp">
        ${row(esc(T('s.ntfyOn')), esc(T('s.ntfyNote')), sw('ntfy.on', ntfy.on))}
        ${row(esc(T('s.ntfyServer')), esc(T('s.ntfyServerNote')), `<input class="in" id="s-ntfy-server" spellcheck="false" autocomplete="off" placeholder="https://ntfy.sh" value="${esc(ntfy.server || '')}">`)}
        <div class="r"><div class="grow"><div>${esc(T('s.ntfyTopic'))}</div><div class="d">${esc(T('s.ntfyTopicNote'))}</div><div class="r"><span class="cmd mono">${esc(ntfy.topic || '')}</span></div><div class="r"><button class="pbtn sm" data-copy="${esc(ntfy.topic || '')}">${esc(T('home.copy'))}</button><button class="pbtn sm" data-mobile="topic">${esc(T('s.ntfyNewTopic'))}</button><button class="pbtn sm al" data-mobile="test">${esc(T('s.ntfyTest'))}</button></div>${tried}</div></div>
      </div>`
  }

  // The phone's app (mobile.rs): its notifications on the island or not,
  // the last ones (an app muted or let back from each), its battery and
  // a ring to find it.
  function phoneBlock() {
    const s = snap.settings
    const m = snap.mobile || {}
    const muted = s.phoneNoticeMute || []
    const b = m.battery
    const ago = ms => (ms < 60000 ? T('s.phoneJustNow') : ms < 3600000 ? T('s.phoneMinAgo', { n: Math.round(ms / 60000) }) : T('s.phoneHourAgo', { n: Math.round(ms / 3600000) }))
    const notices = (m.notices || []).slice(0, 10)
    const list = notices
      .map(n => {
        const off = muted.includes(n.pkg)
        const words = [n.title, n.text].filter(Boolean).join('：')
        return `<div class="r"><div class="grow"><div>${esc(n.app || n.pkg)} <span class="d">· ${esc(ago(n.ago || 0))}${off ? ` · ${esc(T('s.phoneMutedTag'))}` : ''}</span></div>${words ? `<div class="d ellip">${esc(words)}</div>` : ''}</div><button class="pbtn sm${off ? '' : ' no'}" data-${off ? 'unmute' : 'mute'}="${esc(n.pkg)}">${esc(T(off ? 's.phoneUnmute' : 's.phoneMute'))}</button></div>`
      })
      .join('')
    const others = muted.filter(p => !notices.some(n => n.pkg === p))
    const mutedRow = others.length
      ? `<div class="r"><div class="grow"><div>${esc(T('s.phoneMutedList'))}</div><div class="r">${others.map(p => `<button class="pbtn sm" data-unmute="${esc(p)}">${esc(p)} ✕</button>`).join('')}</div></div></div>`
      : ''
    return `${sec(T('s.phone'))}<div class="grp">
        <div class="r"><div class="grow"><div>${esc(b ? T('s.phoneBattery', { n: b.level }) + (b.charging ? ' ⚡' : '') : T('s.phoneAway'))}</div><div class="d">${esc(T('s.phoneRingNote'))}</div></div><button class="pbtn sm" data-mobile="ring"${b ? '' : ' disabled'}>${esc(T('s.phoneRing'))}</button></div>
        ${row(esc(T('s.phoneNotices')), esc(T('s.phoneNoticesNote')), sw('phoneNotices', s.phoneNotices !== false))}
        ${row(esc(T('s.clipToPhone')), esc(T('s.clipToPhoneNote')), sw('clipToPhone', s.clipToPhone !== false))}
        ${mutedRow}
      </div>
      ${sec(T('s.phoneRecent'))}<div class="grp">${list || `<div class="r"><div class="grow d">${esc(T('s.phoneNone'))}</div></div>`}</div>`
  }

  // General: the welcome (nothing connected yet), the sessions and the
  // quick switches, the alerts, startup and the language, her updates, her
  // crash reports, connecting (the plugin, the hooks) and what this is.
  function pageGeneral() {
    return `${showWelcome() ? part('welcome', welcomeBlock()) : ''}${pageNow()}
      ${part('alerts', pageAlerts())}
      ${part('mobile', pageMobile())}
      ${sec(T('s.maps'))}<div class="grp">
        ${row(esc(T('s.mapsKey')), esc(T('s.mapsKeyNote')), `<input class="in" id="s-maps-key" type="password" spellcheck="false" autocomplete="off" placeholder="AIza…" value="${esc(snap.settings.mapsKey || '')}">`)}
        ${row(esc(T('s.mapsHome')), '', `<input class="in" id="s-maps-home" spellcheck="false" autocomplete="off" placeholder="${esc(T('s.mapsHomeHint'))}" value="${esc(snap.settings.mapsHome || '')}">`)}
        ${row(esc(T('s.mapsWork')), '', `<input class="in" id="s-maps-work" spellcheck="false" autocomplete="off" placeholder="${esc(T('s.mapsWorkHint'))}" value="${esc(snap.settings.mapsWork || '')}">`)}
        ${row(esc(T('s.mapsMode')), esc(T('s.mapsModeNote')), seg('mapsMode', [['drive', T('s.mapsDrive')], ['two_wheeler', T('s.mapsScooter')], ['transit', T('s.mapsTransit')]], snap.settings.mapsMode || 'drive'))}
      </div>
      ${sec(T('s.startLang'))}<div class="grp">
        ${row(esc(T('settings.startAtLogin')), esc(T('home.pluginStart')), sw('login', snap.loginAtStart))}
        ${row(esc(T('home.language')), '', seg('lang', [['auto', T('settings.languageAuto')], ['zh-TW', '繁體中文'], ['zh', '简体中文'], ['en', 'EN']], snap.settings.lang))}
      </div>
      ${part('update', pageUpdate())}
      ${part('crash', pageCrash())}
      ${part('connect', pageConnect())}`
  }

  const PAGES = { now: pageGeneral, look: pageLook, widgets: pageWidgets }

  // --- Drawing --------------------------------------------------------------------------------

  function drawHead() {
    const now = snap.now
    const pet = snap.pets.find(p => p.id === snap.settings.pet)
    const title = now.mood === 'idle' ? T('s.headIdle', { name: pet ? pet.name : 'Wakuwaku' }) : T(HEAD_KEY[now.mood])
    const asks = (snap.asks || []).length
    const sub = [Status.nameOf(now, snap.settings.details !== false), asks ? T('s.asksWaiting', { n: asks }) : ''].filter(Boolean).join(' · ')
    // By ✕, in Windows' own glyphs: locked, a click outside (or on the head)
    // closes them no more; on top, over every window as well (locked too:
    // on top and gone at the next click outside is no use). island.rs
    // keep_on_top.
    const locked = !!snap.settings.settingsPin
    const onTop = locked && !!snap.settings.settingsTop
    const lock = `<button class="x g${locked ? ' on' : ''}" data-lock title="${esc(T(locked ? 's.locked' : 's.lock'))}">${locked ? '&#xE72E;' : '&#xE785;'}</button>`
    const top = `<button class="x g${onTop ? ' on' : ''}" data-top title="${esc(T(onTop ? 's.onTop' : 's.top'))}">${onTop ? '&#xE840;' : '&#xE718;'}</button>`
    setHTML(
      head,
      `<div class="grow ellip"><span class="t">${esc(title)}</span>${sub ? ` <span class="s">· ${esc(sub)}</span>` : ''}</div>${clockTag(now)}${lock}${top}<button class="x" data-close title="${esc(T('s.close'))}">✕</button>`,
    )
  }

  function drawTabs() {
    const i = TABS.indexOf(tab)
    tabs.querySelector('.ind').style.transform = `translateX(${i * 100}%)`
    for (const b of tabs.querySelectorAll('button')) {
      b.classList.toggle('on', b.dataset.tab === tab)
      b.querySelector('span').textContent = T(TAB_KEY[b.dataset.tab])
    }
    tabs.querySelector('[data-tab="now"] .n').hidden = !(snap.asks || []).length
  }

  // The page, kept as it is when nothing changed; redrawn, the box being
  // typed in (the download box, a plugin's setting) gets the focus and the
  // caret back.
  function drawBody(reset) {
    const box = body.contains(document.activeElement) && document.activeElement.matches('input, textarea') ? document.activeElement : null
    const which =
      box &&
      (box.id ? `#${box.id}` : box.dataset.field ? `[data-field="${box.dataset.field}"]` : null)
    const caret = which ? box.selectionStart : null
    const changed = setHTML(body, PAGES[tab]())
    if (reset) body.scrollTop = 0
    if (!changed) return
    // Left as it is while it has the focus: set again, a word half-made in
    // an input method was lost (results landing while one typed).
    const again = document.getElementById('s-ref')
    if (again && again.value !== typedRef) again.value = typedRef
    const back = which && body.querySelector(which)
    if (back && back !== document.activeElement) {
      back.focus()
      back.setSelectionRange(caret, caret)
    }
    startThumbs()
  }

  function draw(reset = false) {
    if (!isOpen || !snap) return
    lang = snap.lang || lang
    drawHead()
    drawTabs()
    drawBody(reset)
    // Locked (the button by ✕), a click outside closes them no more.
    foot.querySelector('.foot-text').textContent = T(snap.settings.settingsPin ? 's.footLocked' : 's.foot')
    foot.querySelector('.ver').textContent = `v${snap.version}`
    tick()
    // The same height on every page, however much it holds: the head and
    // the tabs stay where they are from one page to the next. Never taller
    // than the screen: a page scrolls inside.
    const rest = layer.offsetHeight - body.offsetHeight
    const most = screen.availHeight - 56
    body.style.height = `${Math.max(160, Math.min(460, most - rest))}px`
    window.Island?.changed()
  }

  // After the content changed height: measure once it is laid out.
  const relayout = () => requestAnimationFrame(() => window.Island?.changed())

  // --- Open and close ----------------------------------------------------------------------------

  // A part of a page asked for by its old page's name, to scroll to once drawn.
  let partToShow = ''

  function showPart() {
    let name = partToShow
    partToShow = ''
    if (!name) return
    // Connecting while the welcome shows: the welcome, at the page's top.
    if (name === 'connect' && showWelcome()) name = 'welcome'
    const go = () => {
      const at = body.querySelector(`[data-part="${CSS.escape(name)}"]`)
      if (at) body.scrollTop += at.getBoundingClientRect().top - body.getBoundingClientRect().top
      return body.scrollTop
    }
    // There again as the page is drawn anew in a moment (the default pet put
    // in place, a snapshot coming), unless scrolled since: it was scrolled
    // once, before what came after moved it away (the test session, 2026-10-12).
    let set = go()
    for (const ms of [250, 800, 1600]) {
      setTimeout(() => {
        if (Math.abs(body.scrollTop - set) <= 1) set = go()
      }, ms)
    }
  }

  function open(next) {
    // Asked for connecting: the welcome open, folded or not.
    if (next === 'connect') welcomeOpen = true
    if (PART_OF[next]) {
      tab = PART_OF[next]
      partToShow = next
    } else if (String(next).startsWith('widgets:')) {
      // A widget's row on the plugins page (a press on it in the open island).
      tab = 'widgets'
      partToShow = next
    } else if (TABS.includes(next)) {
      tab = next
    }
    const wasOpen = isOpen
    isOpen = true
    if (!wasOpen) openedAt = Date.now()
    if (!wasOpen) {
      window.pet.keyboard(true)
      clearInterval(clockTimer)
      clockTimer = setInterval(tick, 1000)
    } else if (!document.hasFocus()) {
      // Locked and gone under another window: asked for again, to the front.
      window.pet.keyboard(true)
    }
    window.pet.settings.get().then(got => {
      snap = got
      draw(true)
      showPart()
      if (tab === 'look') loadGallery(false)
      layer.focus({ preventScroll: true })
    })
    window.Island?.changed()
  }

  function close(why = 'asked') {
    if (!isOpen) return
    console.log('settings close:', why)
    isOpen = false
    // The welcome opened again only for this time: "not now" said since
    // (the island's 不用提醒), the next open shows it folded (the test session).
    welcomeOpen = false
    clearInterval(clockTimer)
    for (const timer of thumbTimers) clearTimeout(timer)
    thumbTimers = []
    if (layer.contains(document.activeElement)) document.activeElement.blur()
    window.pet.settingsClosed()
    window.Island?.changed()
  }

  function setTab(next) {
    if (next === tab || !TABS.includes(next)) return
    tab = next
    draw(true)
    if (tab === 'look') loadGallery(false)
    relayout()
  }

  // A change: shown at once, then whatever main makes of it.
  function patch(change) {
    Object.assign(snap.settings, change)
    draw()
    window.pet.settings.set(change).then(got => {
      snap = got
      draw()
    })
  }

  async function loadGallery(more) {
    if (more ? gallery.loading : gallery.page > 0) return
    const asked = ++gallery.asked
    gallery.loading = true
    gallery.error = ''
    draw()
    const got = await window.pet.settings.gallery(more ? gallery.page + 1 : 1, gallery.sort, gallery.query)
    if (asked !== gallery.asked) return
    gallery.loading = false
    if (got.ok) {
      gallery.items = more ? [...gallery.items, ...got.items] : got.items
      gallery.page = got.page || 1
      gallery.totalPages = got.totalPages || 1
      gallery.total = got.total ?? gallery.items.length
    } else {
      gallery.error = got.error
    }
    draw()
    relayout()
  }

  // The gallery as a search finds it (no words: the gallery itself), from
  // its first page.
  function search(text) {
    clearTimeout(searchTimer)
    const query = text.trim()
    if (query === gallery.query) return
    gallery.query = query
    gallery.page = 0
    gallery.items = []
    gallery.total = 0
    loadGallery(false)
  }

  // Typed: looked for a moment after the last key (a link is not looked
  // for; the button downloads it), the button saying which it does.
  function searchSoon() {
    clearTimeout(searchTimer)
    const text = typedRef.trim()
    const button = layer.querySelector('[data-fetch]')
    if (button && !fetching) button.textContent = T(isLink(text) ? 'settings.fetch' : 's.search')
    if (!isLink(text)) searchTimer = setTimeout(() => search(text), 400)
  }

  async function fetchPet(reference, fromGallery) {
    if (fetching || gallery.getting) return
    if (fromGallery) gallery.getting = reference
    else fetching = reference
    fetchNote = { text: '', isError: false }
    draw()
    const got = await window.pet.settings.fetch(reference)
    fetching = null
    gallery.getting = ''
    if (got.snapshot) snap = got.snapshot
    if (got.ok) {
      fetchNote = { text: [T('settings.fetched', { name: got.pet.name, author: got.pet.author }), got.warning].filter(Boolean).join(' '), isError: false }
      if (!fromGallery) typedRef = ''
    } else {
      fetchNote = { text: got.error, isError: true }
    }
    draw()
    relayout()
  }

  function copy(text, button) {
    const done = () => {
      button.textContent = T('home.copied')
      setTimeout(() => (button.textContent = button.dataset.label || T('home.copy')), 1800)
    }
    navigator.clipboard?.writeText(text).then(done, () => {
      const area = document.createElement('textarea')
      area.value = text
      document.body.append(area)
      area.select()
      document.execCommand('copy')
      area.remove()
      done()
    })
  }

  // --- Clicks and keys ----------------------------------------------------------------------------

  layer.addEventListener('click', e => {
    const at = selector => e.target.closest(selector)
    if (at('[data-lock]')) return patch(snap.settings.settingsPin ? { settingsPin: false, settingsTop: false } : { settingsPin: true })
    if (at('[data-top]')) return patch(snap.settings.settingsPin && snap.settings.settingsTop ? { settingsTop: false } : { settingsPin: true, settingsTop: true })
    if (at('[data-close]')) {
      // Locked: ✕ closes them (and Esc), a press on the head no more.
      if (snap.settings.settingsPin && !at('.x[data-close]')) return
      return close('click')
    }
    const tabButton = at('[data-tab]')
    if (tabButton) return setTab(tabButton.dataset.tab)
    // A session: to its window (the settings close as it comes to the front).
    const session = at('[data-jump]')
    if (session) {
      return window.pet.jump(session.dataset.jump).then(went => {
        if (!went) session.querySelector('.d').textContent = T('jump.notFound')
      })
    }
    const s = snap?.settings
    if (!s) return

    const toggle = at('[data-sw]')
    if (toggle && !toggle.disabled) {
      const key = toggle.dataset.sw
      if (key === 'login') return window.pet.settings.login(!snap.loginAtStart).then(got => ((snap = got), draw()))
      // Crash reports: sent by themselves, or never (not asked again).
      if (key === 'crashReports') return patch({ crashReports: snap.crash?.mode === 'auto' ? 'off' : 'auto' })
      if (key.startsWith('notify.')) return patch({ notify: { ...s.notify, [key.slice(7)]: !s.notify?.[key.slice(7)] } })
      if (key === 'ntfy.on') return patch({ ntfy: { ...s.ntfy, on: !s.ntfy?.on } })
      // A widget on or off.
      if (key.startsWith('w:')) {
        const id = key.slice(2)
        const off = (s.widgetsOff || []).filter(o => o !== id)
        return patch({ widgetsOff: off.length === (s.widgetsOff || []).length ? [...off, id] : off })
      }
      if (key === 'widgetNudge') return patch({ widgetNudge: s.widgetNudge === false })
      // One of the monitor's parts on or off.
      if (key.startsWith('mon:')) return patch({ monitor: { ...(s.monitor || {}), [key.slice(4)]: (s.monitor || {})[key.slice(4)] === false } })
      // A plugin she runs on or off; on and missing a setting, its settings open.
      if (key.startsWith('pl:')) {
        const p = (snap.plugins || []).find(x => x.id === key.slice(3))
        if (!p) return
        if (!p.on && p.params.some(f => f.required && !String(f.value || '').trim())) openPlugin = p.id
        return setPlugin(p.id, { on: !p.on })
      }
      // A plugin's yes-or-no setting.
      if (key.startsWith('pf:')) {
        const [id, name] = key.slice(3).split('.')
        const p = (snap.plugins || []).find(x => x.id === id)
        return p && setPlugin(id, { [name]: !(p.params.find(f => f.name === name) || {}).value })
      }
      return patch({ [key]: !s[key] })
    }
    // The monitor a second faster or slower.
    const pace = at('[data-every]')
    if (pace && !pace.disabled) {
      const every = Math.min(10, Math.max(1, (s.monitor?.every || 2) + Number(pace.dataset.every)))
      return patch({ monitor: { ...(s.monitor || {}), every } })
    }
    // A plugin's settings folded out, or in.
    const fold = at('[data-pexp]')
    if (fold) {
      openPlugin = openPlugin === fold.dataset.pexp ? '' : fold.dataset.pexp
      draw()
      return relayout()
    }
    // A row a step up: the order as shown, with it and the one before swapped.
    const up = at('[data-wup]')
    if (up) {
      const ids = widgetRows().map(r => r.key)
      const i = ids.indexOf(up.dataset.wup)
      if (i > 0) [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]]
      return patch({ widgetOrder: ids })
    }
    const option = at('[data-seg] > span')
    if (option) {
      const key = option.parentElement.dataset.seg
      const raw = option.dataset.value
      if (key === 'ilookMode') {
        ilookMode = raw === winMode() ? '' : raw
        return draw()
      }
      if (key === 'ilookEdge') return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), edge: raw })
      // An address of one's own: kept once typed (the box below).
      if (key === 'mobileBind') {
        mobileCustom = raw === 'custom'
        return mobileCustom ? draw() : patch({ mobileBind: raw })
      }
      if (key === 'gallery') {
        if (gallery.sort === raw) return
        gallery.sort = raw
        gallery.page = 0
        gallery.items = []
        return loadGallery(false)
      }
      const value = ['lang', 'islandWidth', 'updateFrom'].includes(key) ? raw : raw === 'seen' ? 'seen' : Number(raw)
      return patch({ [key]: value })
    }
    const tile = at('[data-tile]')
    if (tile) {
      const key = tile.dataset.tile
      if (key === 'show') {
        if (!snap.hidden) {
          snap.herOut = !snap.herOut
          return patch({ out: snap.herOut })
        }
        return window.pet.settings.showPet(true).then(got => ((snap = got), draw()))
      }
      return patch({ [key]: !s[key] })
    }
    // Her island's look: folded out or in, a preset, a ground, back to Windows' grey.
    if (at('[data-ilook-fold]')) {
      ilookOpen = !ilookOpen
      draw()
      return relayout()
    }
    const ilookPick = at('[data-ilook-preset]')
    if (ilookPick) {
      const mode = ilookFor()
      return ilookSave(mode, { ...ilookPresets(mode).find(([id]) => id === ilookPick.dataset.ilookPreset)[1] })
    }
    const ilookBg = at('[data-ilook-bg]')
    if (ilookBg) return ilookSave(ilookFor(), { ...ilookOf(ilookFor()), bg: ilookBg.dataset.ilookBg })
    if (at('[data-ilook-reset]')) return ilookSave(ilookFor(), ilookDefault(ilookFor()))
    // Windows' colour dialog takes the focus: not a click outside.
    if (e.target.matches('input[type="color"]')) picking = true
    // Her updates: look now, the release's notes, Update.
    const updateButton = at('[data-update]')
    if (updateButton && !updateButton.disabled) return window.pet.settings.update(updateButton.dataset.update).then(got => ((snap = got), draw()))
    if (at('[data-crash-show]')) return ((crashShown = !crashShown), draw())
    const crashButton = at('[data-crash]')
    if (crashButton && !crashButton.disabled) return window.pet.settings.crash(crashButton.dataset.crash).then(got => ((snap = got), draw()))
    // The phone page's token or the ntfy topic made anew; a push to try it.
    const mobileButton = at('[data-mobile]')
    if (mobileButton && !mobileButton.disabled) {
      const what = mobileButton.dataset.mobile
      if (what === 'test') {
        mobileButton.disabled = true
        pushNote = null
      }
      return window.pet.settings.mobile(what).then(got => {
        snap = got
        if (got.pushed) pushNote = got.pushed
        draw()
      })
    }
    // Her deck to look over (duel.rs).
    if (at('[data-duel-deck]')) return window.pet.duelDeckOpen()
    // An app's notifications kept off the island, or let back.
    const mute = at('[data-mute]')
    if (mute) return patch({ phoneNoticeMute: [...new Set([...(snap.settings.phoneNoticeMute || []), mute.dataset.mute])] })
    const unmute = at('[data-unmute]')
    if (unmute) return patch({ phoneNoticeMute: (snap.settings.phoneNoticeMute || []).filter(p => p !== unmute.dataset.unmute) })
    const use = at('[data-use]')
    if (use) return patch({ pet: use.dataset.use })
    // An agent's pet out on the desktop, or home.
    const agentOut = at('[data-agent-out]')
    if (agentOut) {
      const agent = agentOut.dataset.agentOut
      const desktop = snap.settings.desktop || {}
      return patch({ desktop: { ...desktop, [agent]: desktop[agent] !== true } })
    }
    // An agent's own pet: set, or (pressed again, or Default) the pet in use.
    const agentPet = at('[data-agent-pet]')
    const agentClear = at('[data-agent-pet-clear]')
    if (agentPet || agentClear) {
      const own = { ...(snap.settings.agentPets || {}) }
      const agent = agentPet ? agentPet.dataset.agentPet : agentClear.dataset.agentPetClear
      if (agentPet && own[agent] !== agentPet.dataset.petId) own[agent] = agentPet.dataset.petId
      else delete own[agent]
      return patch({ agentPets: own })
    }
    if (at('[data-fetch]')) {
      const text = typedRef.trim()
      if (!text) return document.getElementById('s-ref')?.focus()
      return isLink(text) ? fetchPet(text, false) : search(text)
    }
    const get = at('[data-get]')
    if (get) return fetchPet(get.dataset.get, true)
    if (at('[data-more]')) return loadGallery(true)
    const copyButton = at('[data-copy]')
    if (copyButton) return copy(copyButton.dataset.copy, copyButton)
    const hooks = at('[data-hooks]')
    if (hooks) {
      const action = hooks.dataset.hooks
      if (action === 'install') {
        connecting = 'claude'
        delete failed.claude
        draw()
      }
      return window.pet.settings.hooks(action).then(got => {
        connecting = ''
        snap = got.snapshot
        // Not written: why (the welcome names the file), and the plugin the other way.
        if (action === 'install' && !got.ok) {
          failed.claude = { error: got.error || '', kind: got.kind || 'other' }
          pluginOpen = true
        }
        draw()
        relayout()
      })
    }
    const agyHooks = at('[data-agy]')
    if (agyHooks) {
      const action = agyHooks.dataset.agy
      if (action === 'install') {
        connecting = 'agy'
        delete failed.agy
        draw()
      }
      return window.pet.settings.agyHooks(action).then(got => {
        connecting = ''
        snap = got.snapshot
        if (action === 'install' && !got.ok) failed.agy = { error: got.error || '', kind: got.kind || 'other', folder: got.folder || '' }
        draw()
        relayout()
      })
    }
    const grokHooks = at('[data-grok]')
    if (grokHooks) {
      const action = grokHooks.dataset.grok
      if (action === 'install') {
        connecting = 'grok'
        delete failed.grok
        draw()
      }
      return window.pet.settings.grokHooks(action).then(got => {
        connecting = ''
        snap = got.snapshot
        if (action === 'install' && !got.ok) failed.grok = { error: got.error || '', kind: got.kind || 'other' }
        draw()
        relayout()
      })
    }
    const codexHooks = at('[data-codex]')
    if (codexHooks) {
      const action = codexHooks.dataset.codex
      if (action === 'install') {
        connecting = 'codex'
        delete failed.codex
        draw()
      }
      return window.pet.settings.codexHooks(action).then(got => {
        connecting = ''
        snap = got.snapshot
        if (action === 'install' && !got.ok) failed.codex = { error: got.error || '', kind: got.kind || 'other' }
        draw()
        relayout()
      })
    }
    // The welcome folded ("not now", the island's hint put away too), or
    // opened again from its row.
    if (at('[data-welcome-later]')) {
      welcomeOpen = false
      patch({ connectHintOff: true })
      return relayout()
    }
    if (at('[data-welcome-open]')) {
      welcomeOpen = true
      draw()
      return relayout()
    }
    // The welcome's plugin, folded out or in; its row to the Connect part.
    if (at('[data-plugin-toggle]')) {
      pluginOpen = !pluginOpen
      draw()
      return relayout()
    }
    const goto = at('[data-goto]')
    if (goto) {
      partToShow = goto.dataset.goto
      return showPart()
    }
    const site = at('[data-site]')
    if (site) return window.pet.settings.openSite(site.dataset.site)
  })

  layer.addEventListener('input', e => {
    if (e.target.dataset.ilook) ilookInput(e.target, false)
    if (e.target.id === 's-ref') {
      typedRef = e.target.value
      // Not mid-word in an input method: once the word is chosen.
      if (!e.isComposing) searchSoon()
    }
    if (e.target.dataset.field) drafts[e.target.dataset.field] = e.target.value
  })

  // The word an input method has chosen: looked for as if typed.
  layer.addEventListener('compositionend', e => {
    if (e.target.id !== 's-ref') return
    typedRef = e.target.value
    searchSoon()
  })

  // A plugin's setting kept once typed (Enter, or leaving the box); a
  // number that is none goes back to what it was.
  layer.addEventListener('change', e => {
    // Getting about (maps.rs): the key, home and work, kept once typed.
    if (e.target.id === 's-maps-key') return patch({ mapsKey: e.target.value.trim() })
    if (e.target.id === 's-maps-home') return patch({ mapsHome: e.target.value.trim() })
    if (e.target.id === 's-maps-work') return patch({ mapsWork: e.target.value.trim() })
    if (e.target.dataset.ilook) return ilookInput(e.target, true)
    // An agent's project folder, kept once typed (launch.rs); blank, none.
    if (e.target.dataset.agentDir) {
      const dirs = { ...(snap.settings.agentDirs || {}) }
      const dir = e.target.value.trim()
      if (dir) dirs[e.target.dataset.agentDir] = dir
      else delete dirs[e.target.dataset.agentDir]
      return patch({ agentDirs: dirs })
    }
    // Phone access: an address of one's own, the page's address through a
    // proxy, ntfy's server; kept once typed, one that is none not kept.
    if (e.target.id === 's-mobile-ip') {
      const ip = e.target.value.trim()
      if (!ip) return
      mobileCustom = false
      return patch({ mobileBind: ip })
    }
    if (e.target.id === 's-mobile-url') return patch({ mobileUrl: e.target.value.trim().replace(/\/+$/, '') })
    if (e.target.id === 's-ntfy-server') return patch({ ntfy: { ...snap.settings.ntfy, server: e.target.value.trim().replace(/\/+$/, '') } })
    // Her Riot API key, kept once typed (lcu.rs); one not as Riot gives them is not kept.
    if (e.target.id === 's-riot-key') return patch({ riotApiKey: e.target.value.trim() })
    // Her own hand traps, for Master Duel's helper (duel.rs).
    if (e.target.id === 's-duel-hand') return patch({ duelHand: e.target.value.trim().slice(0, 300) })
    const key = e.target.dataset.field
    if (!key || !(key in drafts)) return
    const value = drafts[key].trim()
    delete drafts[key]
    const [id, name] = key.split('.')
    const field = ((snap.plugins || []).find(p => p.id === id)?.params || []).find(f => f.name === name)
    if (!field || (field.kind === 'number' && value && !(Number(value) > 0))) return draw()
    setPlugin(id, { [name]: value })
  })

  // A plugin's settings changed, the others kept.
  function setPlugin(id, change) {
    const all = snap.settings.plugins || {}
    return patch({ plugins: { ...all, [id]: { ...(all[id] || {}), ...change } } })
  }

  document.addEventListener('keydown', e => {
    if (!isOpen) return
    if (e.key === 'Escape') {
      e.preventDefault()
      return close('Esc')
    }
    if (e.target.matches?.('input, textarea')) {
      if (e.key === 'Enter' && e.target.id === 's-ref' && !e.isComposing) layer.querySelector('[data-fetch]')?.click()
      if (e.key === 'Enter' && e.target.dataset.field) e.target.blur()
      return
    }
    const i = TABS.indexOf(tab)
    if (e.key === 'ArrowRight') {
      e.preventDefault()
      setTab(TABS[(i + 1) % TABS.length])
    }
    if (e.key === 'ArrowLeft') {
      e.preventDefault()
      setTab(TABS[(i + TABS.length - 1) % TABS.length])
    }
  })

  window.pet.onSettings(({ tab: next } = {}) => open(next))
  window.pet.onSettingsChanged(got => {
    if (!isOpen) return
    snap = got
    draw()
  })
  // The keyboard went elsewhere: a click outside the island; not the colour
  // picker (its own window takes the focus).
  // Locked (the button by ✕), they stay.
  window.pet.onBlur(() => !snap?.settings?.settingsPin && !picking && (!window.pet.mac || Date.now() - openedAt > BLUR_GRACE_MS) && close('blur'))
  window.addEventListener('focus', () => setTimeout(() => (picking = false), 1000))

  window.Settings = {
    isOpen: () => isOpen,
    layer,
    askSlot,
    // The language may change from the menu while they are open.
    redraw: () => draw(),
    close,
    // The plugin whose settings are folded out once the plugins page opens.
    openPlugin: id => (openPlugin = String(id || '')),
  }
})()
