// Her home: a black pill at the top centre after the iPhone's Dynamic
// Island. One element, which springs between three shapes:
//   compact   her round portrait, where the session is (or whose ending it
//             is) and the clock
//   expanded  hovered, or for a few seconds when something happens (a turn
//             done, an error, she needs you): her whole self standing in it,
//             playing that mood, beside the session's name, where it is, how
//             its turn ended, and a line for each other session; a click on
//             a session goes to its window (jump.rs)
//   ask       a prompt from Claude, answered right in the island, her beside it
// Other sessions busy too: a small "+N" by the clock, in the colour of the
// next one that wants something. The words come from status.js.
//
// While no session needs anything, plugins (widgets.rs, widgets.js) take
// turns in the compact island, and the open one lists them all; the wheel
// turns to the next, a click on one keeps it. One may ask to open the island
// for itself (a nudge): never while a session wants you.
//
// She is one element throughout: the portrait grows into her whole self and
// shrinks back, so she is never in two places at once.
//
// Press her and pull down, and the island stretches like a drop of ink with
// her in it; pull far enough and the drop pinches off and she is out: her own
// window takes her, under the cursor, and she lands where you let go. Let go
// before that and she springs back in. The island stays while she is out, her
// seat empty; bring her close and it reaches out a drop for her, let go there
// and it takes her back in.
//
// This page runs in the island's own window (role=island); her window loads
// the same page as role=pet.
//
// The window is just big enough for all this and clicks pass through it; only
// the island takes the pointer. Shapes change in CSS, never by resizing the
// window frame by frame, so the spring stays smooth. When a shape needs more
// room, the window grows first and the island after; when it needs less, the
// island shrinks first and the window after.
;(function () {
  const { t, render: say, list: nameList } = window.I18n
  const { CELL_W, CELL_H, CLIPS, MOOD_CLIP } = window.Sprite
  const Status = window.Status
  const Widgets = window.Widgets

  // Brighter than the pet's colours: these sit on black. On a light ground
  // the same, deep enough to read on white.
  const COLOR = {
    idle: '#8e8e93',
    working: '#5e9bff',
    waiting: '#ffb340',
    done: '#34d27b',
    review: '#b18cff',
    error: '#ff5c6c',
  }
  const COLOR_LIGHT = {
    idle: '#6e6e73',
    working: '#0067c0',
    waiting: '#b25e00',
    done: '#0f7b3f',
    review: '#7346c9',
    error: '#c42b3c',
  }
  // The moods' colours in her island, as its words are (setInk): deeper on
  // a light ground.
  let inkLight = false
  let groundLight = null
  const hue = () => (inkLight ? COLOR_LIGHT : COLOR)
  // A widget's own colour (a script's, or the monitor's), on a light ground
  // as deep as the moods' there: one of the moods' colours as its deeper
  // one, any other darkened until it reads on white (widgets.js).
  function deeper(colour, light) {
    if (!colour || !light) return colour
    const mood = Object.keys(COLOR).find(k => COLOR[k] === String(colour).toLowerCase())
    return mood ? COLOR_LIGHT[mood] : Widgets.deepen(colour)
  }
  const ink = colour => deeper(colour, inkLight)

  // What she plays when the island opens by itself for each mood.
  const NUDGE_CLIP = { waiting: 'waiting', done: 'waving', review: 'review', error: 'failed' }

  // How long the island stays open when something happens (longer with
  // Claude's words to read), and how long the pointer has to rest on it
  // before it opens (so passing by does not).
  const NUDGE_MS = 3600
  const NUDGE_READ_MS = 6500
  const HOVER_OPEN_MS = 140
  const HOVER_CLOSE_MS = 260
  // Matches the springs in style.css: the window waits this long to shrink.
  const SPRING_MS = 560
  const MIN_W = 112
  // The compact island's width, as set (narrow, normal, wide): the same
  // whatever it holds, so it keeps its size while the words change and the
  // widgets take turns. Just her (MIN_W) when there is nothing to say.
  const WIDTHS = { narrow: 240, normal: 300, wide: 380 }
  // On a Mac the island hangs from the screen's top edge over the menu bar
  // (notch.rs, island:top): { height, notch }, the band's height and the
  // notch's width (0 without one); null elsewhere. With a notch the compact
  // island wraps round it: her in the ear at its left, the clock in the ear at
  // its right, the words (which the notch would hide) left out.
  let screenTop = null
  const EAR = 64
  // Dropped open from the notch by her tray icon (setPeek), for PEEK_MS at
  // most if the pointer never comes.
  let peek = false
  let peekTimer = 0
  const PEEK_MS = 6000
  let settingsIdleTimer = 0
  let settingsUsed = false
  // Where the pointer was first seen in the settings: a move counts once it
  // goes a few px from there (WebKit may send one as they open under it).
  let settingsFrom = null
  // Listened for on a Mac only: the notch is there alone.
  for (const type of window.pet.mac ? ['pointermove', 'pointerdown', 'wheel', 'keydown'] : []) {
    document.addEventListener(
      type,
      e => {
        if (view !== 'settings') return
        if (type !== 'pointermove') settingsUsed = true
        else if (!settingsFrom) settingsFrom = { x: e.screenX, y: e.screenY }
        else if (Math.hypot(e.screenX - settingsFrom.x, e.screenY - settingsFrom.y) > 8) settingsUsed = true
      },
      true,
    )
  }
  const slotWidth = () => (screenTop?.notch ? screenTop.notch + 2 * EAR : WIDTHS[config.islandWidth] || WIDTHS.normal)
  // The window without extra room (main's ISLAND), and the island's distance
  // from the top of it. As wide as a pull needs (PULL_ROOM), so the window
  // only ever grows down: a window whose top-left corner moves shows its old
  // picture from the new corner for a frame or two, and the island jumped
  // sideways as she was pulled out or taken back.
  const BASE = { width: 760, height: 132 }
  const TOP = 8
  // Her two sizes: the round portrait in the compact island, and her whole
  // self (one sheet cell, halved) standing in the open one.
  const HEAD = 24
  const HEAD_SCALE = 0.21
  // Her portrait in the settings' head.
  const SET_HEAD = 32
  const SET_HEAD_SCALE = 0.28
  // The settings' width.
  const SETTINGS_W = 520
  const BODY_SCALE = 0.5
  const BODY_W = Math.round(CELL_W * BODY_SCALE)
  const BODY_H = Math.round(CELL_H * BODY_SCALE)
  const OPEN_H = BODY_H + 12
  // The open island's widest, with her and without: a little over what its
  // usual lines take (the monitor's row, the session's), so a long one (a
  // letter's subject) ends in "…" rather than widening it. The other
  // sessions it lists.
  const OPEN_MAX_W = 420
  const OPEN_BARE_MAX_W = 360
  const OTHERS_SHOWN = 3
  // Pulling her out: the room the window takes for it, how far the drop
  // stretches before it pinches off, and the drop's size.
  const PULL_ROOM = { width: 760, height: 440 }
  const BREAK_PX = 110
  const DROP_R = 56
  // Reaching for her: how far the drop goes out when she is near (part of
  // the way), and its size at the start.
  const REACH_SHARE = 0.55
  const REACH_R = 22

  const body = document.body
  const island = document.getElementById('island')
  const compact = document.getElementById('island-compact')
  const expanded = document.getElementById('island-expanded')
  const her = document.getElementById('island-her')
  const herSheet = her.querySelector('.sheet')
  // Not connected yet: a dot in Windows' accent at her portrait's top-right
  // (placeHer puts it there; style.css shows it in the compact island).
  const hintDot = document.createElement('span')
  hintDot.className = 'hint-dot'
  her.after(hintDot)
  const gooIsland = document.querySelector('#goo .goo-island')
  const gooNeck = document.querySelector('#goo .goo-neck')
  const gooDrop = document.querySelector('#goo .goo-drop')
  const dropHer = document.getElementById('drop-her')
  const dropSheet = dropHer.querySelector('.sheet')
  const panel = document.getElementById('panel')
  const stage = document.getElementById('stage')

  let lang = 'en'
  let now = { mood: 'idle', detail: '', project: '', since: null, took: null, others: 0, sessions: 0, list: [] }
  let config = {}
  // OLED's guard against burn-in (the settings' oledShift): what stays put
  // all day, the island, a screen pixel off every 2.5 minutes, round a 5 x 5
  // square row by row and back (two hours), one pixel a step: too little to
  // see. Whole screen pixels (a part of one blurs the words); by the clock,
  // so a restart goes on where it was (style.css --oled-x).
  const OLED_STEP_MS = 150000
  const OLED_PATH = (() => {
    const square = []
    for (let y = -2; y <= 2; y++) {
      const xs = [-2, -1, 0, 1, 2]
      square.push(...(y % 2 ? xs.reverse() : xs).map(x => [x, y]))
    }
    return [...square, ...square.slice(1, -1).reverse()]
  })()
  let oledOn = false
  let oledTimer
  function oledShift() {
    clearTimeout(oledTimer)
    const root = document.documentElement
    if (oledOn) {
      const at = Date.now()
      const [x, y] = OLED_PATH[Math.floor(at / OLED_STEP_MS) % OLED_PATH.length]
      const dpr = devicePixelRatio || 1
      root.style.setProperty('--oled-x', `${x / dpr}px`)
      root.style.setProperty('--oled-y', `${y / dpr}px`)
      root.classList.add('oled-shift')
      oledTimer = setTimeout(oledShift, OLED_STEP_MS - (at % OLED_STEP_MS) + 50)
    } else {
      root.classList.remove('oled-shift')
      root.style.removeProperty('--oled-x')
      root.style.removeProperty('--oled-y')
    }
  }
  let spriteUrl = null
  let second = null
  let isHover = false
  let hoverTimer
  // Something just happened: { until, clip, text }. The island opens for a
  // moment, she plays clip, and text (a hello) replaces the mood's line.
  let nudge = null
  let nudgeTimer
  let view = ''
  let clockTimer
  let blinkTimer
  let herClip = null
  // Nobody at the keyboard or mouse for a while (pet:quiet): she stands still.
  let quiet = false
  let herTimer
  let room = null
  let roomTimer
  // Her being pulled out: { x0, y0, started, broken, at, carrying }. Once
  // the drop pinches off she is carrying: her window has her, this page only
  // waits for the button to be let go.
  let pull = null
  // When the last pull ended: the click the browser sends after it (the press
  // on her, the release on the island) is not a click on the island.
  let pulledAt = 0
  let dropTimer
  // Reaching for her (her middle on the screen, close), or taking her in.
  let isReaching = false
  let isAbsorbing = false
  // Taken back in: she is home from then on for this page, before main says
  // so, so she lands in her live portrait and not in the still one.
  let isComingHome = false
  let comingHomeTimer
  // Her live portrait was showing at the last draw: one showing again is put
  // in its place at once, not sprung there from where she last stood (her
  // whole self, if she was pulled out of the open island).
  let herShown = false
  let retractTimer
  // The session a press in the open island landed on: the clocks redraw it
  // every second, so the release may come down on its redrawn self.
  let pressed = { id: '', at: 0 }
  // The widgets that are on, the one shown, and the turn to the next; all
  // with something to say, on or off.
  let widgets = []
  let widgetAt = 0
  let allWidgets = []
  // The monitor, pinned (its setting, on by default): always in sight at the
  // island's right end, never one of the turns. Unpinned it is
  // one of the widgets taking turns.
  let monitor = null

  // The widgets as sent: those that take turns are those on, but for the
  // monitor pinned; the same one stays shown as the list changes around it.
  function takeWidgets(all) {
    const was = widgets.length ? widgets[widgetAt % widgets.length].id : null
    allWidgets = Array.isArray(all) ? all : []
    monitor = allWidgets.find(w => w.id === 'monitor' && w.on !== false) || null
    widgets = allWidgets.filter(w => w.on !== false && !(w === monitor && isPinned()))
    const still = widgets.findIndex(w => w.id === was)
    if (still >= 0) widgetAt = still
  }
  // What the compact island showed last, and when something else came: it
  // slides in, and carries on sliding in when the island is drawn again.
  let shownKey = ''
  let turnedAt = 0
  // The compact island's width, as drawn last.
  let compactWidth = MIN_W
  let spinTimer = null

  const ROLE = new URLSearchParams(location.search).get('role') === 'island' ? 'island' : 'pet'
  // Risen only for the settings (she is the pet on her own): it goes once they close.
  // This page draws her home in the home's window; her own window draws her.
  const isOn = () => ROLE === 'island'
  // In her seat: there is a pet to show, and she is not out on the desktop.
  const isHome = () => !!spriteUrl && (config.out !== true || isComingHome)
  const isAsking = () => isOn() && !panel.hidden
  // The settings open in the island (settings.js).
  const isSetting = () => isOn() && !!window.Settings?.isOpen()
  const isNudging = () => !!nudge && Date.now() < nudge.until
  // The specifics on screen (session names, commands, replies), unless turned off.
  const isDetailed = () => config.details !== false
  // Neither Claude Code nor Codex connected (main.rs payload, unconnected),
  // and no "don't remind me" (connectHintOff): the island asks for it, and a
  // press on it opens the settings' welcome, where they are connected.
  const isHinting = () => now.unconnected === true && config.connectHintOff !== true
  // The tools on this computer by name, as one says them (main.rs: with
  // unconnected): 「Claude Code、Codex、Antigravity 或 Grok」.
  const toolNames = () => nameList(lang, now.tools || ['Claude Code', 'Codex', 'Antigravity', 'Grok'], 'or')
  // The not-connected hint's colour: Windows' default accent, its light
  // shade, as it reads on the island's dark ground.
  const HINT_ACCENT = '#4cc2ff'
  // Nothing from the sessions to show: the widgets' turn.
  const isQuiet = () => now.mood === 'idle'
  const shownWidget = () => (isQuiet() && widgets.length ? widgets[widgetAt % widgets.length] : null)

  // The clock: how long this turn has run, or how long the finished one took.
  const time = () => Status.time(now)

  function el(tag, cls, text) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    if (text !== undefined) node.textContent = text
    return node
  }

  // A round face with two capsule eyes, in a mood's colour: for a second
  // session, and in her place when there is no pet yet.
  function face(mood, px) {
    const node = el('span', 'face')
    node.style.cssText = `width:${px}px;height:${px}px;background:${hue()[mood] || hue().idle}`
    node.append(el('i'), el('i'))
    return node
  }

  // The portraits' clock (playPortraits).
  let portraitTimer = null

  // Her in the island's line while she is out on the desktop (her own
  // element went with her): a round portrait, her mood's first frame, ringed
  // in its colour. The face stands in only when there is no pet at all.
  function stillHer(px) {
    if (!spriteUrl) return face(now.mood, px)
    const scale = px * 0.00875
    const node = el('span', 'still-her')
    node.style.cssText = `width:${px}px;height:${px}px;box-shadow:0 0 0 1.5px ${hue()[now.mood] || hue().idle}`
    const sheet = el('span', 'sheet')
    const clip = MOOD_CLIP[now.mood]
    sheet.dataset.clip = clip
    sheet.style.cssText = `transform:translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    sheetImage(sheet, spriteUrl)
    showFrame(sheet, CLIPS[clip].row, frameNow(CLIPS[clip]))
    node.append(sheet)
    playPortraits()
    return node
  }

  // The frame a clip is on now, by the clock: a portrait drawn anew (the
  // island redraws every second) carries on where the last one was.
  const frameNow = clip => Math.floor(Date.now() / clip.ms) % clip.frames

  // Her portraits while she is out play her mood's clip (not while quiet).
  // Its timer runs only while there are any to play, started as one is
  // drawn or the quiet ends: it woke 17 times a second all day for none
  // (#14).
  function playPortraits() {
    if (portraitTimer) return
    portraitTimer = setInterval(() => {
      const sheets = document.querySelectorAll('.still-her .sheet[data-clip]')
      if (!sheets.length || !isOn() || document.hidden || quiet) {
        clearInterval(portraitTimer)
        portraitTimer = null
        return
      }
      for (const sheet of sheets) {
        const clip = CLIPS[sheet.dataset.clip]
        if (clip) showFrame(sheet, clip.row, frameNow(clip))
      }
    }, 60)
  }

  // The sheet (the film) behind a cell-sized box, moved to the frame's cell
  // by a transform on its own layer (style.css .film): a frame costs no
  // repaint (it was the box's background-position, painted anew each frame).
  function filmOf(sheet) {
    let film = sheet.querySelector('.film')
    if (!film) {
      film = el('span', 'film')
      sheet.append(film)
    }
    return film
  }

  function showFrame(sheet, row, frame) {
    filmOf(sheet).style.transform = `translate3d(${-frame * CELL_W}px, ${-row * CELL_H}px, 0)`
  }

  function sheetImage(sheet, url) {
    filmOf(sheet).style.backgroundImage = url ? `url("${url}")` : ''
  }

  // --- The shapes ---------------------------------------------------------------

  function fillCompact() {
    compact.title = isHinting() ? t(lang, 'hint.tip', { list: toolNames() }) : ''
    // Three places: her at the left (her portrait while she is out), a line
    // of words cut short in the middle, a short value at its right (the
    // clock, a widget's number), with "+N" for the other sessions before it;
    // then, pinned, the monitor's readings at the island's right end.
    const widget = shownWidget()
    let words, value, key
    if (isHinting()) {
      // Not connected: "Connect Claude Code ›" in the accent, no clock.
      words = el('span', 'label hint', t(lang, 'hint.connect'))
      words.append(el('span', 'fi hchev', '\uE76C'))
      value = el('span', 'clock', '')
      key = 'hint'
    } else if (widget && Widgets.partsOf(widget)) {
      words = el('span', 'label wparts')
      words.innerHTML = Widgets.partsHTML(widget)
      value = el('span', 'clock')
      key = `w:${widget.id}`
    } else if (widget) {
      const said = Widgets.words(lang, widget, isDetailed())
      words = el('span', 'label wlabel')
      words.insertAdjacentHTML('afterbegin', Widgets.icon(widget.icon))
      words.firstChild.style.color = ink(widget.color) || hue().idle
      words.append(said.label)
      value = el('span', 'clock wvalue', said.value)
      value.style.color = ink(widget.color) || hue().idle
      key = `w:${widget.id}`
    } else {
      words = el('span', 'label', Status.brief(lang, now, { detailed: isDetailed() }))
      value = el('span', 'clock', time())
      // Kept up each second in place (tickTimes).
      value.dataset.live = 'time'
      value.style.color = hue()[now.mood]
      key = `s:${now.id || ''}:${now.mood}`
    }
    // The other sessions, as "+N" (the island's open lines list them).
    const more = now.others > 0 ? el('span', 'more', `+${now.others}`) : null
    if (more) more.style.color = hue()[second] || hue().working
    // What takes turns, together: it slides in when it is something else.
    const turning = el('span', 'turning')
    turning.append(words, ...(more ? [more] : []), value)
    const isBare = !words.textContent && !value.textContent && !more
    const pinned = isPinned() ? el('span', 'pinned') : null
    if (pinned) pinned.innerHTML = Widgets.partsHTML(monitor)
    compact.replaceChildren(...(isHome() ? [] : [stillHer(24)]), ...(isBare && pinned ? [] : [turning]), ...(pinned ? [pinned] : []))
    compact.classList.toggle('bare', isBare)
    // As wide as set, and the readings' width more when pinned (their own,
    // fixed); just her (and the readings) with nothing to say.
    // Round a notch: always tucked into it, just as wide and empty, whatever
    // goes on, so the menu bar's icons by it stay in sight and the notch is
    // left to the app that lives there (Atoll and the like: the user's call,
    // 2026-10-14). Her tray icon drops it open (peek); a prompt opens it.
    const tucked = !!screenTop?.notch
    body.classList.toggle('tucked', tucked)
    compact.style.width = tucked
      ? `${screenTop.notch}px`
      : isBare && !screenTop?.notch
        ? ''
        : `${slotWidth() + (pinned ? pinned.offsetWidth + 12 : 0)}px`
    compactWidth = compact.style.width ? parseFloat(compact.style.width) : Math.max(MIN_W, compact.offsetWidth)
    if (key !== shownKey) {
      if (shownKey && view === 'compact') turnedAt = Date.now()
      shownKey = key
    }
    const into = Date.now() - turnedAt
    if (into < TURN_MS) {
      turning.classList.add('turn')
      turning.style.animationDelay = `-${into}ms`
    }
  }

  // How long something new takes to slide into the compact island.
  const TURN_MS = 320
  const isPinned = () => !!monitor && config.monitor?.pin !== false

  // A widget's line in the open island: the one shown is lit.
  function widgetRow(widget, isShown) {
    const { label, value } = Widgets.words(lang, widget, isDetailed())
    const row = el('span', isShown ? 'wrow on' : 'wrow')
    row.dataset.widget = widget.id
    row.insertAdjacentHTML('afterbegin', Widgets.icon(widget.icon))
    row.firstChild.style.color = ink(widget.color) || hue().idle
    const valueText = el('span', 'wv', value)
    valueText.style.color = ink(widget.color) || hue().idle
    if (Widgets.partsOf(widget)) valueText.innerHTML = Widgets.partsHTML(widget)
    row.append(el('span', 'wl', label), valueText)
    return row
  }

  // Whose it is (the session's name) with the clock, where it is, how the
  // turn ended in Claude's words, the to-do list's progress, and a line for
  // each other session. A hello (or something to fix) takes the name's place.
  function fillExpanded() {
    const detailed = isDetailed()
    const name = Status.nameOf(now, detailed)
    const title = (isNudging() && nudge.text) || (detailed && name) || t(lang, `mood.${now.mood}`)
    const sub = detailed
      ? [Status.status(lang, now), name !== now.project ? now.project : '']
      : [now.project, say(lang, now.detail)]
    const others = Status.othersOf(now.list, now)

    const head = el('span', 'head')
    const clockText = el('span', 'clock', time())
    clockText.style.color = hue()[now.mood]
    head.append(el('span', 'title', title), clockText)
    // The session shown, all of it one place to click.
    const me = el('span', 'me')
    canJump(me, now)
    me.append(head)
    // Not the title again (resting, with no name: both would say so).
    const subText = sub.filter(p => p && p !== title).join(' · ')
    if (subText) me.append(el('span', 'sub', subText))
    if (detailed && Status.isEnding(now) && now.reply) me.append(el('span', 'reply', now.reply))
    // How far down the to-do list, and the item it is on.
    if (detailed && now.todo?.total) {
      const progress = el('span', 'progress')
      const bar = el('span', 'bar')
      const fill = el('i')
      fill.style.cssText = `width:${Math.round((100 * now.todo.done) / now.todo.total)}%;background:${hue()[now.mood]}`
      bar.append(fill)
      progress.append(bar)
      if (now.todo.active) progress.append(el('span', 'item', now.todo.active))
      me.append(progress)
    }
    const lines = el('span', 'lines')
    if (isHinting() && !isNudging()) {
      // Not connected: what it is for, and a row to go and connect. The
      // tools by name only in its tip and the welcome card: three of them
      // cut the line short in the open island, its meaning with them.
      const card = el('span', 'me')
      const top = el('span', 'head')
      top.append(el('span', 'title', t(lang, 'hint.title')))
      card.append(top, el('span', 'sub', t(lang, 'hint.sub')))
      const go = el('span', 'hintgo')
      go.dataset.connect = '1'
      go.append(el('span', '', t(lang, 'hint.go')), el('span', '', '›'))
      lines.append(card, go)
    } else lines.append(me)
    if (others.length) {
      const list = el('span', 'others')
      for (const x of others.slice(0, OTHERS_SHOWN)) {
        const row = el('span', 'other')
        canJump(row, x)
        const dot = el('i', 'dot')
        dot.style.background = hue()[x.mood] || hue().idle
        const what = detailed ? Status.status(lang, x, { withClock: false }) : t(lang, `island.${x.mood}`)
        const when = el('span', 'when', Status.time(x))
        when.style.color = hue()[x.mood] || hue().idle
        row.append(dot, el('span', 'who', Status.nameOf(x, detailed)), el('span', 'what', what), when)
        list.append(row)
      }
      if (others.length > OTHERS_SHOWN) list.append(el('span', 'other rest', t(lang, 'status.moreSessions', { n: others.length - OTHERS_SHOWN })))
      lines.append(list)
    }
    // The monitor, and the widgets, the one shown lit: under the sessions too,
    // a session at work not all the island says (2026-10-10).
    if ((widgets.length || monitor) && !(isNudging() && nudge.widget)) {
      const list = el('span', 'wlist')
      const shown = shownWidget()
      if (isPinned()) list.append(widgetRow(monitor, false))
      for (const w of widgets) list.append(widgetRow(w, w === shown))
      lines.append(list)
    }
    // A widget that asked to open the island: what it says, over all else.
    if (isNudging() && nudge.widget) {
      const w = nudge.widget
      const { label, value } = Widgets.words(lang, w, isDetailed())
      // A private one's words stay off screen too, while the specifics do.
      const words = w.private && !isDetailed() ? '' : w.words
      const card = el('span', 'me')
      const top = el('span', 'head')
      top.insertAdjacentHTML('afterbegin', Widgets.icon(w.icon))
      top.firstChild.style.color = ink(w.color) || hue().idle
      top.append(el('span', 'title', words || label || value))
      card.append(top, el('span', 'sub', words ? [label, value].filter(Boolean).join(' · ') : label ? value : ''))
      lines.replaceChildren(card)
    }
    // Not connected: at its foot, "don't remind me".
    if (isHinting() && !isNudging()) {
      const off = el('span', 'hintoff', t(lang, 'hint.off'))
      off.dataset.hintOff = '1'
      lines.append(off)
    }
    expanded.replaceChildren(...(isHome() ? [] : [stillHer(48)]), lines)
  }

  // On to the next widget (or back), and its turn starts over.
  function turnWidget(step) {
    if (widgets.length < 2) return
    widgetAt = (widgetAt + step + widgets.length) % widgets.length
    clearTimeout(spinTimer)
    spinTimer = null
    update()
  }

  // Widgets take turns while the island is quiet and compact.
  function spin() {
    const sec = Number(config.widgetSpin ?? 8)
    if (!(isQuiet() && widgets.length > 1 && sec > 0 && view === 'compact')) {
      clearTimeout(spinTimer)
      spinTimer = null
      return
    }
    if (!spinTimer) {
      spinTimer = setTimeout(() => {
        spinTimer = null
        widgetAt = (widgetAt + 1) % widgets.length
        update()
      }, sec * 1000)
    }
  }

  // A session with a window to go to: clicking it there goes there.
  function canJump(node, session) {
    if (!session.jump || !session.id) return
    node.dataset.jump = session.id
    node.title = t(lang, 'jump.hint')
  }

  // To a session's window; when it has none any more, the island says so.
  function jumpTo(id) {
    window.pet.jump(id).then(went => {
      if (!went) nudgeFor(MOOD_CLIP[now.mood], t(lang, 'jump.notFound'), 2200)
    })
  }

  // --- Her, in the island -----------------------------------------------------------

  // Her shape for the island's: a round portrait ringed in the mood's colour
  // (small in the compact island, a little bigger in the settings' head), or
  // her whole self standing at the left. All the same sheet, cropped and
  // scaled, so one springs into another.
  function placeHer({ width, height }) {
    if (!isHome()) return
    const s = her.style
    const portrait = (px, scale, left, top) => {
      Object.assign(s, { left: `${left}px`, top: `${top}px`, width: `${px}px`, height: `${px}px`, borderRadius: `${px / 2}px` })
      s.setProperty('--ring', hue()[now.mood])
      herSheet.style.transform = `translate(${-(CELL_W * scale - px) / 2}px, -2px) scale(${scale})`
    }
    if (view === 'compact') {
      // At a Mac's screen top, in the middle of the band's height.
      const at = screenTop ? (screenTop.height - HEAD) / 2 : 6
      const left = screenTop ? 10 : 6
      portrait(HEAD, HEAD_SCALE, left, at)
      // The dot over her portrait's top-right, 2 px out.
      Object.assign(hintDot.style, { left: `${left + HEAD - 7}px`, top: `${at - 2}px` })
    } else if (view === 'settings') {
      portrait(SET_HEAD, SET_HEAD_SCALE, 16, 13)
    } else {
      // Standing on the island's foot.
      const top = height - BODY_H - (view === 'ask' ? 10 : 6)
      Object.assign(s, { left: '12px', top: `${top}px`, width: `${BODY_W}px`, height: `${BODY_H}px`, borderRadius: '0px' })
      herSheet.style.transform = `translate(0px, 0px) scale(${BODY_SCALE})`
    }
  }

  // Her mood's clip, in the portrait as standing; waiting by a prompt, a
  // reaction's while the island says something.
  // At rest she plays the idle row once, then holds its first frame this
  // long (as she does out on the desktop, pet.js): each frame is a frame of
  // the whole strip.
  const IDLE_HOLD_MS = 4000

  function animateHer() {
    if (!isHome()) return
    // Quiet (nobody at the keyboard or mouse): still, on her mood's first frame.
    const name = quiet ? '' : view === 'ask' ? 'waiting' : isNudging() ? nudge.clip : MOOD_CLIP[now.mood]
    if (name && name === herClip) return
    clearTimeout(herTimer)
    herClip = name
    if (!name) return showFrame(herSheet, CLIPS[MOOD_CLIP[now.mood]].row, 0)
    const { row, frames, ms } = CLIPS[name]
    let frame = 0
    const step = () => {
      showFrame(herSheet, row, frame)
      frame = (frame + 1) % frames
      if (name === 'idle' && frame === 0) {
        // The row played through: back to its first frame, held.
        herTimer = setTimeout(() => {
          showFrame(herSheet, row, 0)
          herTimer = setTimeout(step, IDLE_HOLD_MS)
        }, ms)
      } else {
        herTimer = setTimeout(step, ms)
      }
    }
    step()
  }

  // --- Room: the window grows before the island does, and shrinks after ----------------

  function roomFor(want) {
    const width = want.width + 48
    const height = TOP + want.height + 24
    return width <= BASE.width && height <= BASE.height ? null : { width, height }
  }

  // Returns true when the window has to grow first.
  function setRoom(next) {
    const size = r => r || BASE
    clearTimeout(roomTimer)
    if (JSON.stringify(next) === JSON.stringify(room)) return false
    const grows = size(next).width > size(room).width || size(next).height > size(room).height
    const apply = () => {
      room = next
      window.pet.panel(next)
    }
    if (grows) apply()
    else roomTimer = setTimeout(apply, SPRING_MS)
    return grows
  }

  // fn once the island's size spring is over: its width's or height's
  // transitionend, or a little after the spring's length should none come
  // (the size unchanged, a spring cut short). Returns what calls it off.
  function afterSpring(fn, late = 0) {
    let timer
    const stop = () => {
      clearTimeout(timer)
      island.removeEventListener('transitionend', done)
    }
    const done = e => {
      if (e && (e.target !== island || (e.propertyName !== 'width' && e.propertyName !== 'height'))) return
      stop()
      fn()
    }
    island.addEventListener('transitionend', done)
    timer = setTimeout(done, SPRING_MS + 50 + late)
    return stop
  }

  const stillMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  // --- Behind the island: a blur, or a deeper ground (zhoupengjie/Wakuwaku#20) ------
  // The page's own blur (--i-blur) blurs only what the page draws: open
  // above the strip, a see-through look had other programs' windows sharp
  // behind it. Under it, then, a window of Windows' acrylic (backplate.rs):
  // for a look both see-through and blurred, while the island is up and
  // someone is about; faded in once its spring is over. While the island changes size it stays, under the
  // smaller of its two sizes, set in further, the ground as solid as
  // DEEP_OP meanwhile (movePlate, isl-firm), and is set where the island has
  // come to once it settles, the ground back as it does. Without it
  // (Windows 10, transparency effects off, the acrylic refused, a look not
  // blurred), over a maximized window the ground deepens (setDeep) so its
  // words read.
  const PLATE_IN_MS = 150
  const PLATE_OUT_MS = 100
  const DEEP_OP = 0.9
  const DEEP_MS = 150
  // The acrylic's corners are DWM's (8 px): set in from the island's edge
  // until they lie inside its rounder ones, (R - 8)(1 - 1/√2), a pixel at
  // least. The band left the page paints
  // (plated, style.css #island::before, its rim).
  const DWM_CORNER = 8
  const plateInset = radius => Math.max(1, (radius - DWM_CORNER) * (1 - Math.SQRT1_2))
  // While it springs to another size, this much further in, and as much
  // again as the spring may carry an edge past where it goes (style.css's
  // curve: some 7 % of the way it moves).
  const PLATE_SLACK = 3
  const SPRING_OVER = 0.07
  // What the plate was last told (where, as JSON; '': none), the wait for
  // the spring before it fades in, and the wait while it moves with it.
  let plateSent = ''
  let plateWait = null
  let plateMove = null
  // A maximized window on her display (island.rs look_over_max).
  let overMaxTop = false

  function plateWanted() {
    if (!isOn() || now.plate !== true || quiet || view === 'settings' || body.classList.contains('pulling')) return false
    const { op, blur } = groundOf(lookNow())
    return op < 1 && blur > 0
  }

  // After the island is laid out; resized: from its box and corners then
  // (from, fromRadius) to its new size.
  function placePlate(resized, from, fromRadius) {
    if (!plateWanted()) return hidePlate(PLATE_OUT_MS)
    if (!plateSent) {
      if (resized || !plateWait) {
        plateWait?.()
        plateWait = afterSpring(() => {
          plateWait = null
          showPlate(PLATE_IN_MS)
        })
      }
      return
    }
    if (resized && from) return movePlate(from, fromRadius)
    if (!plateMove) showPlate(0)
  }

  // Where the island will lie at its new size, from where it lies: from its
  // top, about its middle.
  function rectAt(r, width, height) {
    return { x: r.left + (r.width - width) / 2, y: r.top, width, height }
  }

  // Changing size: under the smaller of its two boxes (where they meet),
  // each edge set in further by the slack and by as much as the spring may
  // carry that edge past where it goes (an edge it grows from stays put),
  // and the ground at once as solid as DEEP_OP (isl-firm): what the plate
  // no longer covers shows through no more than through the rim (at once,
  // the open island's words stood clear over the desktop as it closed;
  // the appearance session, 2026-10-14). Once it settles, the plate where
  // the island has come to and the ground back over DEEP_MS, together.
  function movePlate(from, fromRadius) {
    const to = rectAt(from, parseFloat(island.style.width), parseFloat(island.style.height))
    const x = Math.max(from.left, to.x)
    const y = Math.max(from.top, to.y)
    const box = { x, y, width: Math.min(from.right, to.x + to.width) - x, height: Math.min(from.bottom, to.y + to.height) - y }
    const inset = plateInset(Math.max(fromRadius || 0, parseFloat(island.style.borderRadius) || 0))
    const edge = moved => inset + PLATE_SLACK + SPRING_OVER * Math.abs(moved)
    const sides = [edge(to.x - from.left), edge(to.y - from.top), edge(to.x + to.width - from.right), edge(to.y + to.height - from.bottom)]
    for (const a of island.getAnimations()) if (a.id === 'firm') a.cancel()
    body.classList.add('isl-firm')
    sendPlate(box, sides, inset, 0)
    plateMove?.()
    plateMove = afterSpring(() => {
      plateMove = null
      const firm = getComputedStyle(island).backgroundColor
      body.classList.remove('isl-firm')
      if (!stillMotion()) island.animate([{ offset: 0, backgroundColor: firm }], { id: 'firm', duration: DEEP_MS, easing: 'ease' })
      showPlate(0)
    })
  }

  // Under the island as it is now (moved: told again, no fade).
  function showPlate(fade) {
    if (!plateWanted()) return hidePlate(0)
    const r = island.getBoundingClientRect()
    const inset = plateInset(parseFloat(getComputedStyle(island).borderTopLeftRadius) || 0)
    sendPlate({ x: r.left, y: r.top, width: r.width, height: r.height }, [inset, inset, inset, inset], inset, fade)
  }

  // The plate in box, its edges set in by sides (left, top, right, bottom;
  // never to nothing: it stays), the rim over the corners' band (rim wide);
  // faded in when new.
  function sendPlate(box, sides, rim, fade) {
    const across = Math.min(1, (box.width - 2) / (sides[0] + sides[2]))
    const down = Math.min(1, (box.height - 2) / (sides[1] + sides[3]))
    const [left, top, right, bottom] = [sides[0] * across, sides[1] * down, sides[2] * across, sides[3] * down]
    const rect = { x: box.x + left, y: box.y + top, width: box.width - left - right, height: box.height - top - bottom, vw: innerWidth, vh: innerHeight, dark: modeNow() !== 'light' }
    island.style.setProperty('--rim', `${rim.toFixed(2)}px`)
    body.classList.add('plated')
    const sent = JSON.stringify(rect)
    if (sent === plateSent) return
    const first = !plateSent
    plateSent = sent
    window.pet.plate(rect, first && !stillMotion() ? fade : 0)
  }

  function hidePlate(fade) {
    plateWait?.()
    plateWait = null
    plateMove?.()
    plateMove = null
    body.classList.remove('plated', 'isl-firm')
    if (!plateSent) return
    plateSent = ''
    window.pet.plate(null, stillMotion() ? 0 : fade)
  }

  // Resized for its room, the window moves the island in the page.
  window.addEventListener('resize', () => plateSent && !plateMove && showPlate(0))

  // Over a maximized window (one on her display), a
  // see-through ground the plate does not blur goes to DEEP_OP while open,
  // over DEEP_MS (at once as it opens), and back as it closes or the window
  // is no longer maximized.
  window.pet.onOverMax(({ on } = {}) => {
    overMaxTop = on === true
    if (isOn()) setDeep(true)
  })

  function deepWanted() {
    if (!isOn() || view === 'settings') return false
    const { op, blur } = groundOf(lookNow())
    return op < DEEP_OP && !(now.plate === true && blur > 0) && overMaxTop
  }

  function setDeep(fades) {
    const on = deepWanted()
    if (body.classList.contains('isl-deep') === on) return
    const from = getComputedStyle(island).backgroundColor
    body.classList.toggle('isl-deep', on)
    if (fades && !stillMotion()) island.animate([{ offset: 0, backgroundColor: from }], { id: 'deep', duration: DEEP_MS, easing: 'ease' })
  }

  function setSize({ width, height }) {
    island.style.width = `${width}px`
    island.style.height = `${height}px`
    island.style.borderRadius = screenTop
      ? // Hung from a Mac's screen top: square where it meets the edge.
        `0 0 ${view === 'compact' ? 12 : 30}px ${view === 'compact' ? 12 : 30}px`
      : `${height > 60 ? 30 : height / 2}px`
  }

  // --- Deciding ---------------------------------------------------------------

  function sizeOf(name) {
    const withHer = isHome()
    if (name === 'settings') return { width: window.Settings.layer.offsetWidth || SETTINGS_W, height: window.Settings.layer.offsetHeight }
    if (name === 'ask') {
      return {
        width: panel.offsetWidth + 28 + (withHer ? 108 : 0),
        height: Math.max(panel.offsetHeight + 26, withHer ? BODY_H + 20 : 0),
      }
    }
    if (name === 'expanded') {
      // As tall as what it says, never shorter than her.
      return withHer
        ? { width: Math.min(OPEN_MAX_W, Math.max(320, expanded.offsetWidth)), height: Math.max(OPEN_H, expanded.offsetHeight) }
        : { width: Math.min(OPEN_BARE_MAX_W, Math.max(300, expanded.offsetWidth)), height: Math.max(84, expanded.offsetHeight) }
    }
    return { width: compactWidth, height: screenTop ? screenTop.height : 36 }
  }

  // --- Her look, and the notch's -----------------------------------------------------

  // A widget's place in the settings: its row on the plugins page, its own
  // settings folded out (the monitor's, a plugin's she runs).
  function openWidgetSettings(w) {
    const key = w.plugin || w.id
    if (w.plugin || key === 'monitor') window.Settings?.openPlugin?.(key)
    window.pet.openSettings(key ? `widgets:${key}` : 'widgets')
  }
  // Her tray icon clicked on a Mac: the island in the notch drops open, or
  // goes back. Open, it goes back once the pointer has been in it and left,
  // or after a while if the pointer never comes.
  function setPeek(on) {
    if (on !== peek) console.log(`island: peek ${on ? 'open' : 'closed'}`)
    peek = on
    clearTimeout(peekTimer)
    if (peek) peekTimer = setTimeout(() => !isHover && setPeek(false), PEEK_MS)
    update()
  }
  // Her tray icon toggles it; a double-click on her only opens it ({ open }).
  window.pet.onPeek(p => setPeek(p?.open === true ? true : !peek))
  window.pet.onTop(({ height, notch } = {}) => {
    const next = height > 0 ? { height, notch: notch > 0 ? notch : 0 } : null
    if (JSON.stringify(next) === JSON.stringify(screenTop)) return
    screenTop = next
    body.classList.toggle('at-top', !!screenTop)
    body.classList.toggle('notched', !!screenTop?.notch)
    document.documentElement.style.setProperty('--top-h', `${screenTop ? screenTop.height : 36}px`)
    update()
  })
  // Windows' mode, in pet:update (winLight).
  const modeNow = () => (now.winLight === true ? 'light' : 'dark')
  // The look as the settings give it now (none: the defaults: black), its
  // own for each of Windows' modes (islandTopLook, zhoupengjie/Wakuwaku#27);
  // and its ground's colour (null: none given), how solid (0.2 to 1; solid
  // when no colour is given) and blurred.
  const lookNow = () => config.islandTopLook?.[modeNow()] || {}
  function groundOf(mine) {
    const bg = /^#[0-9a-f]{6}$/i.test(mine.bg || '') ? parseInt(mine.bg.slice(1), 16) : null
    const op = bg !== null && Number.isFinite(mine.op) ? Math.min(100, Math.max(20, mine.op)) / 100 : 1
    const blur = Number.isFinite(mine.blur) ? Math.min(60, Math.max(0, mine.blur)) : 0
    return { bg, op, blur }
  }

  // Her island's look, as the settings may set it for each of Windows'
  // modes (islandTopLook: { dark, light }, each { bg: '#rrggbb', op: 20 to
  // 100, blur: 0 to 60, edge: 'line' | 'ring' | 'none' }), over black
  // (style.css --i-bg, --i-edge); a part left out keeps its default. The
  // ring is the session's colour (--ring, as the island draws). The page
  // has nothing behind the island to blur: only Windows' acrylic blurs it
  // (placePlate).
  function drawIslandLook() {
    const mine = lookNow()
    const { bg, op, blur } = groundOf(mine)
    const rgba = a => `rgba(${(bg >> 16) & 255}, ${(bg >> 8) & 255}, ${bg & 255}, ${a})`
    if (bg === null) body.style.removeProperty('--i-bg')
    else body.style.setProperty('--i-bg', rgba(op))
    // Deeper over a maximized window, with nothing to blur it (setDeep).
    if (bg === null || op >= DEEP_OP) body.style.removeProperty('--i-bg-deep')
    else body.style.setProperty('--i-bg-deep', rgba(DEEP_OP))
    // The rim over the acrylic's edge (sendPlate): three quarters of the
    // way from the ground's opacity to solid, half that as it fades in.
    const rim = op + (1 - op) * 0.75
    if (bg === null || op >= 1) {
      body.style.removeProperty('--i-rim')
      body.style.removeProperty('--i-rim-half')
    } else {
      body.style.setProperty('--i-rim', rgba(rim.toFixed(3)))
      body.style.setProperty('--i-rim-half', rgba((rim / 2).toFixed(3)))
    }
    const edge = {
      ring: '0 0 0 1.5px var(--ring), 0 0 12px color-mix(in srgb, var(--ring) 35%, transparent)',
      none: '0 0 0 0 transparent',
    }[mine.edge]
    if (edge) island.style.setProperty('--i-edge', edge)
    else island.style.removeProperty('--i-edge')
    groundLight = bg === null ? null : lightness(bg) > 0.35
    setInk()
  }

  // A colour's relative lightness (WCAG), 0 to 1.
  function lightness(rgb) {
    const [r, g, b] = [(rgb >> 16) & 255, (rgb >> 8) & 255, rgb & 255].map(v => {
      v /= 255
      return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4
    })
    return 0.2126 * r + 0.7152 * g + 0.0722 * b
  }

  // Her island's words dark (style.css isl-light, hue, ink) on a light
  // ground of the settings' own (groundLight; null: none given); in the
  // settings, theirs.
  function setInk() {
    const own = view === 'settings' ? null : groundLight
    inkLight = own === null ? false : own
    body.classList.toggle('isl-light', inkLight)
  }

  function update() {
    clearInterval(clockTimer)
    // While she is being pulled out, the pull has the island.
    if (!isOn() || (pull?.started && !pull.carrying) || isAbsorbing) return
    // A prompt answered: the island closes rather than lingering open.
    if (view === 'ask' && !isAsking()) nudge = null
    const was = view
    view = isSetting() ? 'settings' : isAsking() ? 'ask' : isHover || isNudging() || peek ? 'expanded' : 'compact'
    // In the notch and closed it is out of the way: the pointer passes to
    // what is under it (the notch's own app), and a hover opens nothing.
    body.classList.toggle('aside', !!screenTop?.notch && view === 'compact')
    // Opened in the notch (from her menu) and the pointer never comes: back
    // after a while, as a peek goes back (PEEK_MS). Counted from their
    // opening, not from each redraw (the widgets redraw every few seconds).
    // Under a still pointer they are not being used: only a move, a press, a
    // wheel or a key in them since they opened is (settingsUsed): they opened
    // under the pointer that had just chosen them from her menu, and stayed.
    if (screenTop?.notch && view === 'settings' && !settingsUsed) {
      settingsIdleTimer ||= setTimeout(() => {
        settingsIdleTimer = 0
        if (!settingsUsed && isSetting()) window.Settings?.close('unused')
      }, PEEK_MS)
    } else if (view !== 'settings') {
      clearTimeout(settingsIdleTimer)
      settingsIdleTimer = 0
      settingsUsed = false
      settingsFrom = null
    }
    island.dataset.view = view
    // The deeper ground at once as it opens; changing, over a moment.
    setDeep(was === view || view === 'compact')
    setInk()
    island.style.setProperty('--ring', hue()[now.mood])
    island.classList.toggle('wants', now.mood === 'waiting')
    seatPanel()
    body.classList.toggle('has-her', isHome())
    body.classList.toggle('hinting', isHinting() && isHome())
    document.documentElement.style.setProperty('--hint-accent', HINT_ACCENT)
    body.classList.toggle('compact-her', view === 'compact' || view === 'settings')
    if (spriteUrl) sheetImage(herSheet, spriteUrl)
    fillCompact()
    fillExpanded()

    const want = sizeOf(view)
    // Reaching for her keeps the room to reach in.
    const grows = setRoom(isReaching ? PULL_ROOM : roomFor(want))
    // The window has its room before the island (and she) grow into it.
    const grow = () => {
      const resized = island.style.width !== `${want.width}px` || island.style.height !== `${want.height}px`
      // Where it is, for the plate under it to stay covered as it changes.
      const from = resized && plateSent ? island.getBoundingClientRect() : null
      const fromRadius = from ? parseFloat(getComputedStyle(island).borderTopLeftRadius) : 0
      setSize(want)
      const snap = isHome() && !herShown
      if (snap) her.classList.add('snap')
      placeHer(want)
      if (snap) {
        void her.offsetWidth
        her.classList.remove('snap')
      }
      herShown = isHome()
      animateHer()
      placePlate(resized, from, fromRadius)
    }
    if (grows) setTimeout(grow, 40)
    else grow()

    // The clocks run: the turn's, the step's, the other sessions'. Their words
    // may grow (a step timed once it has run a while), so the shape follows.
    const isTimed = time() || now.stepSince || Status.othersOf(now.list, now).some(x => Status.time(x))
    if (isTimed && (view === 'compact' || view === 'expanded')) clockTimer = setInterval(tickTimes, 1000)
    spin()
  }

  // Each second while something is timed: the times' words alone, put in
  // place (the compact island's clock); the island drawn
  // anew (update) only when a word grew or shrank, as the shape follows the
  // words, or while the island is open, whose lines carry the step's time
  // too. It was drawn anew every second (2026-10-11).
  function tickTimes() {
    if (view !== 'compact') return update()
    let resized = false
    const put = (node, text) => {
      if (!node || node.textContent === text) return
      if (node.textContent.length !== text.length) resized = true
      node.textContent = text
    }
    put(compact.querySelector('[data-live="time"]'), time())
    if (resized) update()
  }

  function nudgeFor(clip, text, ms = NUDGE_MS, widget = null) {
    // In the notch nothing opens by itself (done, failed, a hello, a widget):
    // only a prompt, which Claude waits on (the user's call, 2026-10-14).
    if (screenTop?.notch) return update()
    nudge = { until: Date.now() + ms, clip, text, widget }
    clearTimeout(nudgeTimer)
    nudgeTimer = setTimeout(() => {
      nudge = null
      update()
    }, ms)
    update()
  }

  // Now and then a blink, as a class for a moment (no running animation).
  function blink() {
    clearTimeout(blinkTimer)
    blinkTimer = setTimeout(() => {
      if (isOn() && !document.hidden) {
        island.classList.add('blink')
        setTimeout(() => island.classList.remove('blink'), 140)
      }
      blink()
    }, 3500 + Math.random() * 4000)
  }

  // --- Pulling her out: the drop --------------------------------------------------

  function pullDown(e) {
    console.log('pointerdown on her in the island, button', e.button)
    if (e.button !== 0 || !isHome() || view === 'ask' || isSetting() || isAbsorbing) return
    // No mouse events follow, so the island does not take this for a click.
    e.preventDefault()
    pull = { x0: e.clientX, y0: e.clientY, started: false, broken: false, at: null, carrying: false }
    window.pet.holding(true)
    window.addEventListener('pointermove', pullMove)
    window.addEventListener('pointerup', pullUp)
    window.addEventListener('pointercancel', pullUp)
    // The button is held on this window wherever the cursor goes, so the
    // pointer events keep coming here even once her own window has her. On the
    // island, not on her: her element is hidden once she is out, and a hidden
    // element loses the pointer.
    try {
      island.setPointerCapture(e.pointerId)
    } catch {}
  }

  function pullMove(e) {
    if (pull?.started) pulledAt = Date.now()
    if (!pull || pull.carrying) return
    if (!pull.started) {
      if (Math.hypot(e.clientX - pull.x0, e.clientY - pull.y0) < 5) return
      startPull()
    }
    stretch(e.clientX, e.clientY)
    // Pinched off: from here her own window carries her, anywhere on the screen.
    if (pull.broken) handOff(e)
  }

  function startPull() {
    pull.started = true
    nudge = null
    clearTimeout(nudgeTimer)
    clearInterval(clockTimer)
    // Room to stretch in, then the drop: she hangs in it kicking her legs.
    setRoom(PULL_ROOM)
    startDropHer()
    body.classList.add('pulling')
    hidePlate(0)
  }

  function startDropHer() {
    clearTimeout(dropTimer)
    sheetImage(dropSheet, spriteUrl)
    const { row, frames, ms } = CLIPS.jumping
    let frame = 0
    const step = () => {
      showFrame(dropSheet, row, frame)
      frame = (frame + 1) % frames
      dropTimer = setTimeout(step, ms)
    }
    step()
  }

  // Where a neck leaves her home: the point of its edge nearest (x, y), so a
  // drop below the island hangs from its lower edge.
  function edgeNear(r, x, y) {
    const [ix, iy] = [24, 14]
    return [Math.min(Math.max(x, r.left + ix), r.right - ix), Math.min(Math.max(y, r.top + iy), r.bottom - iy)]
  }

  // Her seat in her home, in the page: where she springs back to.
  function seatIn(r) {
    return [r.left + 18, r.top + 18]
  }

  // The home's double, a neck from it to (cx, cy), and a drop there; the
  // filter melts them into one shape. A neck of 0 leaves the drop on its own.
  function drawGoo(cx, cy, { neck, radius = DROP_R }) {
    const r = island.getBoundingClientRect()
    const [ax, ay] = edgeNear(r, cx, cy)
    Object.assign(gooIsland.style, {
      left: `${r.left}px`,
      top: `${r.top}px`,
      width: `${r.width}px`,
      height: `${r.height}px`,
      borderRadius: getComputedStyle(island).borderRadius,
    })
    Object.assign(gooDrop.style, { left: `${cx - radius}px`, top: `${cy - radius}px`, width: `${radius * 2}px`, height: `${radius * 2}px` })
    const length = Math.hypot(cx - ax, cy - ay)
    const angle = (Math.atan2(cy - ay, cx - ax) * 180) / Math.PI - 90
    Object.assign(gooNeck.style, {
      left: `${ax - neck / 2}px`,
      top: `${ay}px`,
      width: `${neck}px`,
      height: `${length}px`,
      transform: `rotate(${angle}deg)`,
    })
    return length
  }

  // You hold her by the head: her middle hangs below the cursor. The neck
  // thins as it stretches, and pinches off past BREAK_PX.
  function stretch(x, y) {
    const r = island.getBoundingClientRect()
    // From the top of the screen she can only come down.
    const cx = x
    const cy = Math.max(y + 30, r.bottom - 10)
    const [ax, ay] = edgeNear(r, cx, cy)
    const dist = Math.hypot(cx - ax, cy - ay)
    pull.broken = dist > BREAK_PX
    drawGoo(cx, cy, { neck: pull.broken ? 0 : Math.max(8, 34 - dist * 0.22) })
    Object.assign(dropHer.style, { left: `${cx - BODY_W / 2}px`, top: `${cy - BODY_H / 2}px` })
    pull.at = { cx, cy }
  }

  // The drop let go of her: her window takes her where she is, and the island
  // wobbles back with her seat empty.
  function handOff(e) {
    console.log('hand-off')
    pull.carrying = true
    clearTimeout(dropTimer)
    window.pet.releaseHer({ dx: pull.at.cx - e.clientX, dy: pull.at.cy - e.clientY })
    body.classList.remove('pulling')
    wobble()
    herClip = null
    update()
  }

  function wobble() {
    body.classList.remove('wobble')
    void island.offsetWidth
    body.classList.add('wobble')
    setTimeout(() => body.classList.remove('wobble'), 520)
  }

  function pullUp(e) {
    console.log('pull up:', e?.type, pull ? (pull.carrying ? 'carrying' : pull.started ? 'pulling' : 'pressed') : 'no pull')
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    window.removeEventListener('pointercancel', pullUp)
    window.pet.holding(false)
    if (!pull) return
    const done = pull
    pull = null
    if (!done.started) {
      // A click on her, not a pull: the settings, as anywhere on the island.
      if (view !== 'ask') window.pet.openSettings()
      return
    }
    pulledAt = Date.now()
    if (done.carrying) {
      // Let go: she lands there, or comes home if she was let go by the island.
      window.pet.dropHer()
      window.pet.hover(false)
      return
    }
    pull = done
    settleBack(done.at)
  }

  // Into her seat from where the drop has her: the drop and she spring back.
  function settleBack(at) {
    const r = island.getBoundingClientRect()
    const [tx, ty] = seatIn(r)
    if (at) drawGoo(at.cx, at.cy, { neck: 26 })
    body.classList.add('pulling')
    requestAnimationFrame(() => {
      body.classList.add('settling')
      drawGoo(tx, ty, { neck: 0, radius: 10 })
      Object.assign(dropHer.style, { left: `${tx - BODY_W / 2}px`, top: `${ty - BODY_H / 2}px`, transform: 'scale(0.25)' })
    })
    setTimeout(() => {
      body.classList.remove('pulling', 'settling', 'reaching')
      clearTimeout(dropTimer)
      dropHer.style.transform = ''
      pull = null
      isAbsorbing = false
      herClip = null
      window.pet.hover(island.matches(':hover'))
      update()
    }, 340)
  }

  // --- Reaching for her, and taking her back ------------------------------------------

  // Run fn once the window has its room to reach in: drawn before, the
  // island would move under the drawing as the window grows around it.
  function whenRoomy(fn) {
    setRoom(PULL_ROOM)
    if (innerWidth >= PULL_ROOM.width - 2 && innerHeight >= PULL_ROOM.height - 2) return fn()
    const go = () => {
      window.removeEventListener('resize', go)
      clearTimeout(timer)
      fn()
    }
    const timer = setTimeout(go, 200)
    window.addEventListener('resize', go)
  }

  // Her middle on the screen as she is moved near (snap: let go now and she
  // is back), or null when she has gone off again.
  function reach(at) {
    if (!isOn() || isAbsorbing) return
    clearTimeout(retractTimer)
    if (!at) {
      if (!isReaching) return
      isReaching = false
      // The drop draws back into the island.
      body.classList.add('settling')
      const r = island.getBoundingClientRect()
      const [sx, sy] = seatIn(r)
      drawGoo(sx, sy, { neck: 0, radius: 8 })
      retractTimer = setTimeout(() => {
        body.classList.remove('reaching', 'settling')
        update()
      }, 320)
      return
    }
    if (!isReaching) {
      isReaching = true
      // The next call draws, once the window has grown.
      return whenRoomy(() => isReaching && reach(at))
    }
    body.classList.remove('settling')
    body.classList.add('reaching')
    const x = at.x - window.screenX
    const y = at.y - window.screenY
    const r = island.getBoundingClientRect()
    const [ax, ay] = edgeNear(r, x, y)
    // Close: the drop reaches all the way and holds her; nearer: part way.
    const k = at.snap ? 1 : REACH_SHARE * Math.min(1, Math.max(0, 1 - (Math.hypot(x - ax, y - ay) - 140) / 140))
    const tx = ax + (x - ax) * k
    const ty = ay + (y - ay) * k
    drawGoo(tx, ty, { neck: at.snap ? 26 : 20, radius: at.snap ? DROP_R : REACH_R + 14 * k })
  }

  // She was let go close enough: from her window's spot into the drop, and
  // the drop back into the island.
  function absorb(at) {
    if (!isOn() || !at) return
    clearTimeout(retractTimer)
    isReaching = false
    isAbsorbing = true
    isComingHome = true
    clearTimeout(comingHomeTimer)
    // Main says she is in a moment after; should it not, she is not.
    comingHomeTimer = setTimeout(() => {
      isComingHome = false
      update()
    }, 3000)
    body.classList.remove('reaching', 'settling')
    startDropHer()
    whenRoomy(() => {
      const cx = at.x - window.screenX
      const cy = at.y - window.screenY
      Object.assign(dropHer.style, { left: `${cx - BODY_W / 2}px`, top: `${cy - BODY_H / 2}px`, transform: '' })
      settleBack({ cx, cy })
    })
  }

  window.pet.onReach(reach)
  window.pet.onAbsorb(absorb)
  // She landed but this page never heard the button go up: forget the pull.
  window.pet.onLanded(() => {
    if (!pull?.carrying) return
    console.log('landed without a pointerup here')
    window.removeEventListener('pointermove', pullMove)
    window.removeEventListener('pointerup', pullUp)
    window.removeEventListener('pointercancel', pullUp)
    pull = null
    window.pet.hover(false)
    update()
  })

  her.addEventListener('pointerdown', pullDown)
  island.addEventListener('lostpointercapture', () => console.log('lost pointer capture', pull ? (pull.carrying ? 'while carrying' : 'while pulling') : ''))

  // --- On and off ---------------------------------------------------------------

  // The prompt panel lives in the island while it is on (a banner in the
  // settings while they are open), above the pet otherwise.
  function seatPanel() {
    if (!isOn()) return
    const seat = isSetting() ? window.Settings.askSlot : island
    if (panel.parentElement !== seat) seat.append(panel)
  }

  function place() {
    body.classList.toggle('island', isOn())
    body.classList.toggle('home-island', isOn())
    drawIslandLook()
    seatPanel()
    if (!isOn() && panel.parentElement !== stage) stage.prepend(panel)
    if (!isOn()) {
      hidePlate(0)
      clearInterval(clockTimer)
      clearTimeout(herTimer)
      clearTimeout(dropTimer)
      clearTimeout(roomTimer)
      body.classList.remove('pulling', 'settling', 'reaching', 'has-her', 'compact-her')
      pull = null
      isReaching = false
      isAbsorbing = false
      herClip = null
      room = null
      view = ''
      herShown = false
    }
  }

  window.pet.onUpdate(data => {
    const before = now.mood
    now = data
    lang = data.lang || lang
    config = data.config || {}
    if (!!config.oledShift !== oledOn) {
      oledOn = !!config.oledShift
      oledShift()
    }
    if (config.out !== true) isComingHome = false
    spriteUrl = data.sprite
    second = data.second || null
    quiet = data.quiet === true
    takeWidgets(data.widgets)
    place()
    // Something new that wants you, or is finished: the island opens for a
    // moment, a longer one with Claude's words to read.
    const isReading = isDetailed() && Status.isEnding(now) && !!now.reply
    if (isOn() && before !== now.mood && NUDGE_CLIP[now.mood]) nudgeFor(NUDGE_CLIP[now.mood], null, isReading ? NUDGE_READ_MS : NUDGE_MS)
    else update()
  })

  // Only the widgets' words changed (the monitor's readings every couple
  // of seconds, main.rs redraw_widgets): the island drawn anew.
  window.pet.onWidgets(({ widgets: all }) => {
    takeWidgets(all)
    if (isOn()) update()
  })

  // Nobody at the keyboard or mouse for a while, or someone again: she
  // stands still, or plays on.
  window.pet.onQuiet(({ on }) => {
    quiet = on === true
    herClip = null
    animateHer()
    playPortraits()
    // The blur behind the island too: none while nobody is about.
    if (isOn()) placePlate(false)
  })

  // A hello, or something to fix: the island opens to say it, and she waves.
  window.pet.onReact(({ say: text }) => {
    if (isOn() && text) nudgeFor('waving', say(lang, text))
  })

  // A widget asks to open the island (main checked that nothing wants you).
  window.pet.onNudge(n => {
    if (!isOn()) return
    const at = widgets.findIndex(w => w.id === n.id)
    if (at >= 0) widgetAt = at
    // Not a widget's (a new version): its own icon.
    nudgeFor('waving', null, NUDGE_READ_MS, { ...n, icon: widgets[at]?.icon || n.icon, color: widgets[at]?.color })
  })

  // --- The pointer ----------------------------------------------------------------

  for (const target of [island]) {
    target.addEventListener('mouseenter', () => {
      window.pet.hover(true)
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        isHover = true
        update()
      }, HOVER_OPEN_MS)
    })
    target.addEventListener('mouseleave', () => {
      // Pulling her out keeps the window's pointer, wherever it goes.
      if (!pull?.started) window.pet.hover(false)
      clearTimeout(hoverTimer)
      hoverTimer = setTimeout(() => {
        isHover = false
        // In the notch, whatever was open goes back once the pointer has
        // left, as a notch app's does: the settings too, unless a field in
        // them is being typed in (the user's call, 2026-10-14).
        const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName || '')
        if (screenTop?.notch && isSetting() && !typing) window.Settings?.close('left')
        if (peek) setPeek(false)
        else update()
      }, HOVER_CLOSE_MS)
    })
    // One click opens the settings (no double-click to wait for), except on a
    // prompt and its buttons, in the settings themselves, or on her (her
    // press may be a pull; pullUp decides). In the open island, a click on a
    // session goes to its window instead.
    target.addEventListener('click', e => {
      // A click in the settings (✕, the head) may just have closed them: not a click to open.
      if (isSetting() || window.Settings?.layer?.contains(e.target) || Date.now() - pulledAt < 500) return
      if (view === 'ask' || panel.contains(e.target) || her.contains(e.target)) return
      // The press, if the clocks redrew the session between it and the release.
      const id = view === 'expanded' && (e.target.closest('[data-jump]')?.dataset.jump || (Date.now() - pressed.at < 1000 && pressed.id))
      pressed = { id: '', at: 0 }
      if (id) return jumpTo(id)
      // Not connected: "don't remind me" puts the hint away; any other
      // press but on a session's or a widget's row opens the settings'
      // welcome (a session listed still goes to its window: the hooks
      // removed while it runs).
      if (e.target.closest('[data-hint-off]')) return window.pet.settings.set({ connectHintOff: true })
      if (isHinting() && !e.target.closest('[data-widget]')) return window.pet.openSettings('connect')
      // A widget in the open island: the one shown from now on, and its
      // settings open (a press that only lit it felt like none: the user,
      // 2026-10-13).
      const row = view === 'expanded' && e.target.closest('[data-widget]')
      if (row) {
        const at = widgets.findIndex(w => w.id === row.dataset.widget)
        if (at >= 0) turnWidget(at - (widgetAt % widgets.length))
        return openWidgetSettings(at >= 0 ? widgets[at] : { id: row.dataset.widget })
      }
      const open = isNudging() && nudge.widget?.open
      // A page, or a part of one (a new version: the updates).
      if (open?.tab) return window.pet.openSettings(open.tab)
      window.pet.openSettings()
    })
    // The wheel turns to the next widget, while the widgets have the island.
    target.addEventListener(
      'wheel',
      e => {
        if (!isQuiet() || widgets.length < 2 || isSetting() || view === 'ask') return
        e.preventDefault()
        turnWidget(e.deltaY > 0 ? 1 : -1)
      },
      { passive: false },
    )
    target.addEventListener('pointerdown', e => {
      const session = e.button === 0 && view === 'expanded' && e.target.closest('[data-jump]')
      pressed = { id: session ? session.dataset.jump : '', at: Date.now() }
    })
    target.addEventListener('contextmenu', e => {
      if (e.target.matches('input, textarea')) return
      e.preventDefault()
      window.pet.menu()
    })
  }

  blink()

  // panel.js tells the island when its prompt changes.
  window.Island = { isOn, changed: update, reach, absorb }
})()
