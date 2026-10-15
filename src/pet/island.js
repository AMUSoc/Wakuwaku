// Her home: by display, a black pill at the top centre after the iPhone's
// Dynamic Island (island), or a strip along the bottom of the screen in
// place of Windows' taskbar (taskbar): its left end hers, the other sessions
// as tags along the rest, with the Start button, the tray's icons
// (taskbar.rs) and the clock besides. The same element in each, which
// springs between three shapes (below the island; in the taskbar, growing
// up from it):
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

  // Brighter than the pet's colours: these sit on black. On the light
  // taskbar (Windows' light mode) the same, deep enough to read on white.
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
  // a light ground; on the taskbar's strip (stripHue), as Windows' mode.
  let inkLight = false
  let groundLight = null
  const hue = () => (inkLight ? COLOR_LIGHT : COLOR)
  const stripHue = () => (isLight() ? COLOR_LIGHT : COLOR)
  // A widget's own colour (a script's, or the monitor's), on a light ground
  // as deep as the moods' there: one of the moods' colours as its deeper
  // one, any other darkened until it reads on white (widgets.js).
  function deeper(colour, light) {
    if (!colour || !light) return colour
    const mood = Object.keys(COLOR).find(k => COLOR[k] === String(colour).toLowerCase())
    return mood ? COLOR_LIGHT[mood] : Widgets.deepen(colour)
  }
  const ink = colour => deeper(colour, inkLight)
  const stripInk = colour => deeper(colour, isLight())

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
  // The taskbar's strip, the room kept above it (both island.rs's), and her
  // portrait in it.
  const TASKBAR_H = 48
  const TASKBAR_ROOM = 640
  // The settings above the taskbar: clear of it, and room for their shadow.
  const TASKBAR_GAP = 8
  const TASKBAR_SHADE = 12
  const TASK_HEAD = 30
  // On the taskbar's Mica her end is an island of its own in the strip: a
  // black capsule this far in from the strip's left end and its foot.
  const CAPSULE_X = 6
  const CAPSULE_Y = 5
  // A portrait px across is the sheet at this scale.
  const PER_PX = 0.00875
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
  // all day, the taskbar's icons, words and clock and the island, a screen
  // pixel off every 2.5 minutes, round a 5 x 5 square row by row and back
  // (two hours), one pixel a step: too little to see. Whole screen pixels
  // (a part of one blurs the words); by the clock, so a restart goes on
  // where it was. The strip's own ground stays put (style.css --oled-x).
  // Up here: what lays the taskbar out (alignApps) reads it.
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
    // The tray's icons where they are drawn now, for the menus they open.
    requestAnimationFrame(sendTrayRects)
  }
  // Windows' look as taskbar.rs sent it last: its mode (dark, light) and
  // accent colour; and whether its End task is on (a program's menu offers it).
  let look = { mode: 'dark' }
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
  // with something to say, on or off (any may be on the taskbar).
  let widgets = []
  let widgetAt = 0
  let allWidgets = []
  // The monitor, pinned (its setting, on by default): always in sight at the
  // island's right end (the bar's), never one of the turns. Unpinned it is
  // one of the widgets taking turns.
  let monitor = null

  // The widgets as sent: those that take turns are those on, but for the
  // monitor pinned and what the taskbar shows of its own; the same one
  // stays shown as the list changes around it.
  function takeWidgets(all) {
    const was = widgets.length ? widgets[widgetAt % widgets.length].id : null
    allWidgets = Array.isArray(all) ? all : []
    monitor = allWidgets.find(w => w.id === 'monitor' && w.on !== false) || null
    const onBar = home() === 'taskbar' ? Widgets.shownOn(tbLayout(), allWidgets) : new Set()
    widgets = allWidgets.filter(w => w.on !== false && !(w === monitor && isPinned()) && !onBar.has(w.id))
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
  // Her home: the island, or the taskbar.
  const home = () => (config.display === 'taskbar' ? 'taskbar' : 'island')
  // A strip across the screen (the taskbar): the other sessions have tags in it.
  // The taskbar's strip the desktop's picture through (mica.js), neither
  // solid nor clear.
  const isMica = () => isOn() && home() === 'taskbar' && !['black', 'clear'].includes(config.taskbarMaterial)
  // The taskbar's strip clear, the desktop itself through it; while a window
  // is maximized or full screen on its display (taskbar.rs "max") it fills
  // in, as Mica (its default) or solid, unless it is to stay clear always
  // (taskbarClearWhen).
  const isClear = () => isOn() && home() === 'taskbar' && config.taskbarMaterial === 'clear'
  const clearWhen = () => (['solid', 'always'].includes(config.taskbarClearWhen) ? config.taskbarClearWhen : 'mica')
  const isFilled = () => isClear() && clearWhen() !== 'always' && taskWindows.some(a => (a.windows || []).some(w => w.max && !w.min))
  // The taskbar light, as Windows' own is in its light mode (taskbar.rs).
  const isLight = () => isOn() && home() === 'taskbar' && look.mode === 'light'
  // Her end a capsule of its own in the strip (black, or light in Windows'
  // light mode), wherever the strip is not black itself: on the Mica, clear,
  // or light.
  const isCapsule = () => isMica() || isClear() || isLight()
  // The screen's edge the taskbar is along (taskbarEdge): bottom, as
  // Windows' own, or top, left, right. Its look turns with it (style.css
  // tb-top, tb-left, tb-right).
  const tbEdge = () => (home() === 'taskbar' && ['top', 'left', 'right'].includes(config.taskbarEdge) ? config.taskbarEdge : 'bottom')
  // Along the left or right of the screen: upright, its runs top, middle,
  // bottom, its buttons one above another.
  const isSide = () => tbEdge() === 'left' || tbEdge() === 'right'
  // Along the strip and across it, in the page: x and left/right on a
  // bottom or top bar, y and top/bottom on a side one.
  const along = (x, y) => (isSide() ? y : x)
  const across = (x, y) => (isSide() ? x : y)
  const lo = r => (isSide() ? r.top : r.left)
  const hi = r => (isSide() ? r.bottom : r.right)
  const len = r => (isSide() ? r.height : r.width)
  const crossLo = r => (isSide() ? r.left : r.top)
  const crossHi = r => (isSide() ? r.right : r.bottom)
  // A side bar's capsule (A): upright, as wide as the strip less 5 a side
  // (on the black strip, its end as wide as the strip), her portrait in its
  // round end, her session's clock under it, no words (the tooltip and the
  // open island have them): a circle with no clock.
  const SIDE_CLOCK = 4 + 13 + 7
  function sideCapsule(timed) {
    const width = isCapsule() ? TASKBAR_H - 2 * CAPSULE_Y : TASKBAR_H
    const inset = (width - TASK_HEAD) / 2
    return { width, height: timed ? inset + TASK_HEAD + SIDE_CLOCK : width }
  }
  // The compact taskbar island's height: the strip's, or the capsule's.
  const taskCompactH = () => (isCapsule() ? TASKBAR_H - 2 * CAPSULE_Y : TASKBAR_H)
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
  // unconnected): 「Claude Code、Codex 或 Antigravity」.
  const toolNames = () => nameList(lang, now.tools || ['Claude Code', 'Codex', 'Antigravity'], 'or')
  // Windows' accent as it reads on the island's ground: its light shade on a
  // dark one, its dark shade on a light one.
  const hintAccent = () => (isLight() ? look.accent?.dark || '#0067c0' : look.accent?.light || '#4cc2ff')
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
    // A side bar's upright capsule: her portrait (as the island draws it, or
    // a still one while she is out) and her session's clock under it; its
    // words in its tooltip.
    if (isSide()) {
      const at = time()
      const hint = isHinting()
      // Not connected: a link's glyph in the clock's place, in the accent.
      const clock = hint ? el('span', 'fi chint', '\uE71B') : el('span', 'clock', at)
      if (!hint) {
        clock.dataset.live = 'time'
        clock.style.color = hue()[now.mood]
      }
      compact.replaceChildren(...(isHome() ? [] : [stillHer(TASK_HEAD)]), ...(hint || at ? [clock] : []))
      compact.classList.toggle('bare', !(hint || at))
      compact.style.width = ''
      compact.title = hint ? t(lang, 'hint.tip', { list: toolNames() }) : Status.brief(lang, now, { detailed: isDetailed() })
      compactWidth = sideCapsule(hint || !!at).width
      return
    }
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
    const pinned = isPinned() && home() === 'island' ? el('span', 'pinned') : null
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
      const [px, at] = home() === 'taskbar' ? [TASK_HEAD, (taskCompactH() - TASK_HEAD) / 2] : [HEAD, screenTop ? (screenTop.height - HEAD) / 2 : 6]
      const left = home() === 'taskbar' ? at : screenTop ? 10 : 6
      if (home() === 'taskbar') portrait(TASK_HEAD, TASK_HEAD * PER_PX, at, at)
      else portrait(HEAD, HEAD_SCALE, left, at)
      // The dot over her portrait's top-right, 2 px out; 1 px in on an
      // upright capsule, whose round top would cut it (the island clips).
      const [dx, dy] = isSide() ? [px - 10, 1] : [px - 7, -2]
      Object.assign(hintDot.style, { left: `${left + dx}px`, top: `${at + dy}px` })
    } else if (view === 'settings') {
      portrait(SET_HEAD, SET_HEAD_SCALE, 16, 13)
    } else {
      // Under a top bar, or from a side bar's capsule in its upper half, she
      // hangs from the island's top; else she stands on its foot. Beside a
      // right bar, at the island's right (it is mirrored).
      const hangs = tbEdge() === 'top' || (isSide() && !body.classList.contains('isl-low'))
      const top = hangs ? 12 : height - BODY_H - (view === 'ask' ? 10 : 6)
      const left = tbEdge() === 'right' ? width - BODY_W - 12 : 12
      Object.assign(s, { left: `${left}px`, top: `${top}px`, width: `${BODY_W}px`, height: `${BODY_H}px`, borderRadius: '0px' })
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

  // The window with no extra room, by home: the taskbar is as wide as the
  // screen, with its room above.
  const baseRoom = () => (home() === 'taskbar' ? { width: innerWidth, height: TASKBAR_H + TASKBAR_ROOM } : BASE)

  function roomFor(want) {
    // The taskbar: what grows up from it, or the settings standing above it,
    // past its room (the window then grows up too, which flashes: its room
    // is kept as high as the settings ever are, settingsMax).
    if (home() === 'taskbar' && isSide()) return null
    if (home() === 'taskbar') {
      const height = want.height + TASKBAR_SHADE + (view === 'settings' ? TASKBAR_H + TASKBAR_GAP : isCapsule() ? CAPSULE_Y : 0)
      if (height <= TASKBAR_H + TASKBAR_ROOM) return null
      return { width: innerWidth, height }
    }
    const width = want.width + 48
    const height = TOP + want.height + 24
    return width <= BASE.width && height <= BASE.height ? null : { width, height }
  }

  // Returns true when the window has to grow first.
  function setRoom(next) {
    const size = r => r || baseRoom()
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

  // Collapsing on the taskbar, the open island's ground, edge and shadow
  // stay (style.css shrinking) until it has shrunk, then fade to the
  // capsule's over GROUND_FADE_MS: gone at once, a bare capsule's ground
  // left the open island's words over the desktop while it was still big
  // (zhoupengjie/Wakuwaku#16). Open again meanwhile, it has them at once.
  const GROUND_FADE_MS = 180
  let groundWait = null
  const stillMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches

  function holdGround(was, late) {
    if (home() === 'taskbar' && view === 'compact' && was && was !== 'compact') {
      groundWait?.()
      island.classList.add('shrinking')
      groundWait = afterSpring(fadeGround, late)
      return
    }
    if (view === 'compact' && home() === 'taskbar') return
    groundWait?.()
    groundWait = null
    island.classList.remove('shrinking')
    for (const a of island.getAnimations()) if (a.id === 'ground') a.cancel()
  }

  function fadeGround() {
    groundWait = null
    const held = getComputedStyle(island)
    const from = { offset: 0, backgroundColor: held.backgroundColor, boxShadow: held.boxShadow, backdropFilter: held.backdropFilter }
    island.classList.remove('shrinking')
    if (!stillMotion()) island.animate([from], { id: 'ground', duration: GROUND_FADE_MS, easing: 'ease-out' })
  }

  // --- Behind the island: a blur, or a deeper ground (zhoupengjie/Wakuwaku#20) ------
  // The page's own blur (--i-blur) blurs only what the page draws: open
  // above the strip, a see-through look had other programs' windows sharp
  // behind it. Under it, then, a window of Windows' acrylic (backplate.rs):
  // for a look both see-through and blurred, while the island is open (at
  // the top of the screen, while it is up at all) and someone is about;
  // faded in once its spring is over, out as it closes on the taskbar,
  // before it shrinks. While the island changes size it stays, under the
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
  // least (4.1 under the taskbar's 22 px). The band left the page paints
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
  // A maximized window on her display, at the top of the screen (island.rs
  // look_over_max; on the taskbar, its windows' list says).
  let overMaxTop = false

  function plateWanted() {
    if (!isOn() || now.plate !== true || quiet || view === 'settings' || body.classList.contains('pulling')) return false
    const { op, blur } = groundOf(lookNow())
    if (!(op < 1 && blur > 0)) return false
    return home() !== 'taskbar' || view !== 'compact'
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

  // Where the island will lie at its new size, from where it lies: at the
  // top of the screen from its top, about its middle; on the taskbar from
  // the strip's edge it grows from, at her end's side (isl-right, isl-low).
  function rectAt(r, width, height) {
    if (home() !== 'taskbar') return { x: r.left + (r.width - width) / 2, y: r.top, width, height }
    const fromRight = tbEdge() === 'right' || (!isSide() && body.classList.contains('isl-right'))
    const fromFoot = tbEdge() === 'bottom' || (isSide() && body.classList.contains('isl-low'))
    return { x: fromRight ? r.right - width : r.left, y: fromFoot ? r.bottom - height : r.top, width, height }
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

  // Over a maximized window (one on her display, taskbar.rs "max"), a
  // see-through ground the plate does not blur goes to DEEP_OP while open,
  // over DEEP_MS (at once as it opens), and back as it closes or the window
  // is no longer maximized.
  const overMax = () => (home() === 'taskbar' ? taskWindows.some(a => (a.windows || []).some(w => w.max && !w.min)) : overMaxTop)
  window.pet.onOverMax(({ on } = {}) => {
    overMaxTop = on === true
    if (isOn()) setDeep(true)
  })

  function deepWanted() {
    if (!isOn() || view === 'settings' || (home() === 'taskbar' && view === 'compact')) return false
    const { op, blur } = groundOf(lookNow())
    return op < DEEP_OP && !(now.plate === true && blur > 0) && overMax()
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
    // The taskbar's left end is square, and what grows up from it is rounded
    // above (on its Mica, her capsule and all it grows into round).
    // What grows from her end is flat where it meets the strip: the strip's
    // own end on the black one (its corners at the screen's edge square).
    const flat = { bottom: '22px 22px 0 0', top: '0 0 22px 22px', left: '0 22px 22px 0', right: '22px 0 0 22px' }
    island.style.borderRadius =
      home() === 'taskbar'
        ? view === 'compact'
          ? isCapsule()
            ? `${(isSide() ? width : height) / 2}px`
            : '0'
          : view === 'settings' || isCapsule()
            ? '22px'
            : flat[tbEdge()]
        : screenTop
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
    if (isSide()) return sideCapsule(isHinting() || !!time())
    return { width: compactWidth, height: home() === 'taskbar' ? taskCompactH() : screenTop ? screenTop.height : 36 }
  }

  // --- The taskbar's right part ---------------------------------------------------

  // Start and the programs, the monitor, the tray's icons, the clock and the
  // settings. Its left edge stays where the compact island ends, so what
  // opens from the island grows up over it. The other sessions have no tags
  // on it: their windows' buttons go to them, and the island says how they
  // are (the user's call, 2026-10-12).
  const barRest = document.createElement('div')
  barRest.id = 'bar-rest'
  // Keys as Windows' own taskbar takes them: a screen reader in a page
  // reads it by its arrows (Narrator's scan mode) and the arrows never came
  // to her buttons; as an application they come (tried, 2026-10-14). Named
  // as the keyboard comes to it (keys, below).
  barRest.setAttribute('role', 'application')
  island.after(barRest)

  // The widget shown, and the monitor, pinned: always there by the settings.
  // In the taskbar only the monitor (before the tray), in two rows as the
  // clock has its two, coloured as Windows' own taskbar would (style.css):
  // the widgets take their turns at its left end, in her place (fillCompact).
  function widgetFaces() {
    const faces = []
    const w = widgets.length && home() !== 'taskbar' ? widgets[widgetAt % widgets.length] : null
    if (w) {
      const { label, value } = Widgets.words(lang, w, isDetailed())
      const face = el('span', 'face')
      face.insertAdjacentHTML('afterbegin', Widgets.icon(w.icon))
      face.firstChild.style.color = stripInk(w.color) || stripHue().idle
      const v = el('span', 'v', value)
      v.style.color = stripInk(w.color) || stripHue().idle
      if (Widgets.partsOf(w)) v.innerHTML = Widgets.partsHTML(w)
      face.append(Widgets.partsOf(w) ? '' : label, v)
      faces.push(face)
    }
    if (isPinned()) {
      const inRows = home() === 'taskbar'
      const readings = el('span', inRows ? 'face mon rows' : 'face mon')
      readings.innerHTML = inRows ? Widgets.rowsHTML(monitor, isSide()) : Widgets.partsHTML(monitor)
      faces.push(readings)
    }
    return faces
  }

  function fillBar() {
    if (home() === 'taskbar') fillTaskbar()
  }

  // The taskbar's parts that stay put: rebuilt every second with the
  // clocks, the tray's icons would lose the pointer and blink.
  const startButton = el('span', 'start')
  startButton.dataset.bar = 'start'
  // Windows 11's mark, drawn as Windows draws it (its own is drawn by its
  // taskbar's code: no file or icon font holds it), as measured on its own
  // taskbar: as big as the programs' icons (29 of the 30 screen pixels of
  // 24px at 125%), four panes in one blue running from light at the top left
  // to deep at the bottom right (style.css .s0, .s1), a one-pixel gap. On
  // the screen's own pixels, as Windows': the panes in whole device pixels
  // for the screen's scale, and the mark moved by the part of a pixel it
  // lands off one (snapStart), or the gap blurs.
  let startScale = 0
  function drawStartMark() {
    const dpr = devicePixelRatio || 1
    if (dpr === startScale) return
    startScale = dpr
    const box = Math.round(24 * dpr)
    const gap = Math.max(1, Math.round(0.8 * dpr))
    const pane = Math.floor((box - 1 - gap) / 2)
    const at = Math.floor((box - 2 * pane - gap) / 2)
    const far = at + pane + gap
    const end = far + pane
    const r = Math.max(0.5, 0.8 * dpr).toFixed(2)
    const rect = (x, y) => `<rect x="${x}" y="${y}" width="${pane}" height="${pane}" rx="${r}"/>`
    startButton.innerHTML = `<svg viewBox="0 0 ${box} ${box}" style="width:${box / dpr}px;height:${box / dpr}px"><defs><linearGradient id="start-blue" gradientUnits="userSpaceOnUse" x1="${at}" y1="${at}" x2="${end}" y2="${end}"><stop class="s0" offset="0"/><stop class="s1" offset="1"/></linearGradient></defs><g fill="url(#start-blue)">${rect(at, at)}${rect(far, at)}${rect(at, far)}${rect(far, far)}</g></svg>`
  }
  // Moved by its layout (position: relative), not by a transform: a moved
  // layer is drawn again off the grid, blurring it (2026-10-10, on the
  // live taskbar). The part of the taskbar it is in starts on the grid too
  // (onPixels in fillTaskbar).
  function snapStart() {
    const mark = startButton.firstElementChild
    if (!mark || !mark.isConnected || startButton.classList.contains('down')) return
    const dpr = devicePixelRatio || 1
    mark.style.left = '0px'
    mark.style.top = '0px'
    const r = mark.getBoundingClientRect()
    const off = v => (Math.round(v * dpr) - v * dpr) / dpr
    mark.style.left = `${off(r.left).toFixed(3)}px`
    mark.style.top = `${off(r.top).toFixed(3)}px`
  }
  // A length that starts and ends on the screen's own pixels.
  const onPixels = v => Math.round(v * (devicePixelRatio || 1)) / (devicePixelRatio || 1)
  drawStartMark()
  window.addEventListener('resize', () => {
    drawStartMark()
    snapStart()
  })
  // As Windows 11's: its mark pressed in under the pointer and springing
  // back; a bounce when Windows' Start menu opens by the Windows key, and
  // lit while it is open (taskbar.rs taskbar:start). Pressed just now, the
  // press was its bounce.
  let startPressedAt = 0
  startButton.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    startButton.classList.add('down')
    startPressedAt = Date.now()
  })
  for (const type of ['pointerup', 'pointerleave', 'pointercancel']) startButton.addEventListener(type, () => startButton.classList.remove('down'))
  startButton.addEventListener('animationend', () => startButton.classList.remove('bounce'))
  window.pet.onStart(({ open } = {}) => {
    startButton.classList.toggle('open', !!open)
    if (!open || Date.now() - startPressedAt < 800) return
    startButton.classList.remove('bounce')
    void startButton.offsetWidth
    startButton.classList.add('bounce')
  })
  const windowsBox = el('span', 'wins')
  const facesBox = el('span', 'faces')
  const trayBox = el('span', 'tray')
  // The keyboard: Caps Lock while it is on, and the input method's script
  // or picture (a press switches its script; a right press, the input
  // methods to pick from).
  const keysBox = el('span', 'kbd')
  const capsMark = el('span', 'caps', 'A')
  const imeMark = el('span', 'ime')
  imeMark.dataset.bar = 'ime'
  keysBox.append(capsMark, imeMark)
  // Windows' quick settings (Wi-Fi, Bluetooth, volume…), as its own taskbar
  // has them: the network's way out (by cable, Wi-Fi, a VPN's, or none) and
  // the volume, in its own icon font (.fi: Segoe Fluent Icons).
  const quickBox = el('span', 'tquick')
  quickBox.dataset.bar = 'quick'
  const NET = { wired: '', wifi: '', other: '', none: '' }
  const netMark = el('span', 'fi net')
  // The volume as Windows draws it: the speaker's three waves faint, those up
  // to the volume solid over them; struck through, muted.
  const volMark = el('span', 'fi vol')
  const volBack = el('span', 'back', '')
  const volFront = el('span', 'front', '')
  volMark.append(volBack, volFront)
  // The battery after them, as Windows' own taskbar has it on a laptop (none
  // on a desktop), in Windows' own battery font (SysBatt Fluent Icons, read
  // from Windows: taskbar_battery_font) as its taskbar draws it: an outline,
  // a bolt in it while charging, a plug while plugged in and not charging (a
  // battery its laptop keeps at 80% too), and over it the level in tenths, in
  // the fills cut for the bolt or the plug; green while charging, yellow with
  // the energy saver on, red when critically low. Its percentage beside it as
  // Windows' "Battery percentage" has it. Without the font (before Windows
  // 11 has it), Segoe's battery glyphs, one for each tenth.
  const batMark = el('span', 'bat')
  batMark.hidden = true
  const batBase = el('span', 'fi base')
  const batFill = el('span', 'fi fill')
  const batGlyph = el('span', 'glyph')
  batGlyph.append(batBase, batFill)
  const batText = el('span', 'pct')
  batMark.append(batGlyph, batText)
  // Windows': each outline; the plain fills for tenths 1 to 10 after the
  // plain outline, the cut ones for 3 to 10 after the bolt's and the plug's
  // (1 and 2 do not reach the bolt or the plug, and are the plain ones).
  const SYSBATT = { plain: 0xf8d0, bolt: 0xf8db, plug: 0xf8e4 }
  const BATTERY = {
    level: ['', '', '', '', '', '', '', '', '', '', ''],
    charging: ['', '', '', '', '', '', '', '', '', '', ''],
    saver: ['', '', '', '', '', '', '', '', '', '', ''],
  }
  let sysBatt = false
  let batShown = null
  window.pet.taskbar
    .batteryFont()
    .then(bytes => {
      // Bytes as they are, or as numbers where the IPC had to send them so.
      const font = Array.isArray(bytes) ? new Uint8Array(bytes).buffer : bytes
      if (!(font instanceof ArrayBuffer) || !font.byteLength) return
      const face = new FontFace('SysBatt Fluent Icons', font)
      return face.load().then(() => {
        document.fonts.add(face)
        sysBatt = true
        showBattery(batShown)
      })
    })
    .catch(() => {})
  quickBox.append(netMark, volMark, batMark)
  // The wheel turned on it: the volume, as on Windows' own taskbar
  // (zhoupengjie/Wakuwaku#35): a notch a step up or down (2%, by Windows'
  // volume keys, its own flyout shown); a touchpad's or a fine wheel's
  // small turns gathered to a notch's worth. Not while the layout is edited.
  let wheelGathered = 0
  quickBox.addEventListener(
    'wheel',
    e => {
      if (editLayout) return
      e.preventDefault()
      const dy = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY
      wheelGathered = Math.abs(dy) >= 50 ? Math.sign(dy) * 100 : wheelGathered + dy
      const steps = Math.trunc(wheelGathered / 100)
      if (!steps) return
      wheelGathered -= steps * 100
      window.pet.taskbar.volume(-steps)
    },
    { passive: false },
  )
  function showNet(kind) {
    netMark.textContent = NET[kind] || NET.other
  }
  showNet('other')
  function showVolume(v) {
    const [level, muted] = Array.isArray(v) ? v : [null, false]
    volFront.textContent = muted ? '' : level == null || level >= 67 ? '' : level >= 34 ? '' : level > 0 ? '' : ''
    // Muted, the waves' room is the struck-through speaker's, unseen.
    volBack.textContent = muted ? volFront.textContent : ''
    volBack.hidden = !!muted
    volMark.title = level == null ? '' : muted ? t(lang, 'taskbar.muted') : t(lang, 'taskbar.volume', { n: level })
  }
  // { percent, plugged, charging, saver, critical, left (s), shown } (widgets.rs battery_state), or none.
  function showBattery(b) {
    batShown = b
    batMark.hidden = !b
    if (!b) return
    const n = b.percent
    // In tenths as Windows has them: 96% full, 86% nine, and some left a
    // tenth (red when critically low); empty at none.
    const tenth = Math.max(0, Math.min(10, Math.ceil(n / 10)))
    const plugged = b.plugged || b.charging
    batMark.classList.toggle('sysbatt', sysBatt)
    if (sysBatt) {
      const outline = SYSBATT[b.charging ? 'bolt' : plugged ? 'plug' : 'plain']
      batBase.textContent = String.fromCharCode(outline)
      batFill.textContent = !tenth ? '' : String.fromCharCode(outline === SYSBATT.plain || tenth < 3 ? SYSBATT.plain + tenth : outline + tenth - 2)
    } else {
      batBase.textContent = (plugged ? BATTERY.charging : b.saver ? BATTERY.saver : BATTERY.level)[tenth]
      batFill.textContent = ''
    }
    // The fill's color, on battery alone but for charging's.
    batMark.dataset.tint = b.charging ? 'green' : plugged ? '' : b.critical ? 'red' : b.saver ? 'yellow' : ''
    batText.textContent = `${n}%`
    batText.hidden = !b.shown
    const left = Number.isFinite(b.left) ? Math.round(b.left / 60) : null
    const time = left == null ? '' : left >= 60 ? t(lang, 'taskbar.hm', { h: Math.floor(left / 60), m: left % 60 }) : t(lang, 'taskbar.m', { m: left })
    const said = plugged ? t(lang, 'taskbar.batteryPlugged', { n }) : time ? t(lang, 'taskbar.batteryLeft', { n, time }) : t(lang, 'taskbar.battery', { n })
    batMark.title = b.saver ? `${said}\n${t(lang, 'taskbar.batterySaver')}` : said
  }
  // What is using the microphone, the camera, the location, as Windows'
  // taskbar shows it before the input method; a press, its privacy settings.
  const PRIVACY = { mic: '', cam: '', loc: '' }
  const privBox = el('span', 'tpriv')
  let privDrawn = ''
  function showPrivacy(use) {
    const drawn = JSON.stringify(use || {})
    if (drawn === privDrawn) return
    privDrawn = drawn
    privBox.replaceChildren(
      ...Object.keys(PRIVACY)
        .filter(k => use?.[k]?.length)
        .map(k => {
          const mark = el('span', 'fi', PRIVACY[k])
          mark.dataset.bar = `privacy-${k}`
          mark.title = t(lang, `taskbar.inUse.${k}`, { apps: use[k].join(lang === 'zh' ? '、' : ', ') })
          return mark
        }),
    )
  }
  const clockBox = el('span', 'tclock')
  clockBox.dataset.bar = 'notifications'
  // Show the desktop: a sliver at the screen's right edge, as Windows' own (Win+D).
  const deskEdge = el('span', 'tdesk')
  deskEdge.dataset.bar = 'desktop'
  // The room between the middle run and the right one: what centring the
  // middle moves it into (alignApps).
  const growNode = el('span', 'grow')
  let taskbarBuilt = false

  // --- The taskbar's layout (taskbarLayout) -------------------------------------------
  // Its parts in three runs, as a KDE panel has them: from the left end, in
  // the middle of the screen (moved left when crowded, alignApps), and up to
  // the right end; show-the-desktop's sliver last of all, always (Windows'
  // corner). Her island is one of the parts ('her'): a slot in its run with
  // the island laid over it (placeSlot), growing from it the way there is
  // room (body.isl-right: from its right edge, leftwards). Edited by
  // dragging the parts about on the strip itself (editTaskbar).
  const TB_PARTS = ['her', 'start', 'apps', 'monitor', 'tray', 'keys', 'quick', 'clock']
  const TB_RIGHT = ['monitor', 'tray', 'keys', 'quick', 'clock']
  const TB_PRESETS = {
    win11: { left: ['her'], center: ['start', 'apps'], right: TB_RIGHT },
    left: { left: ['her', 'start', 'apps'], center: [], right: TB_RIGHT },
    herRight: { left: ['start', 'apps'], center: [], right: [...TB_RIGHT, 'her'] },
  }
  const herSlot = el('span', 'tslot')
  // The tray's icons and the marks of what uses the microphone, camera or
  // location: one part.
  const trayPart = el('span', 'tpair')
  trayPart.append(trayBox, privBox)
  const PART_NODES = { her: herSlot, start: startButton, apps: windowsBox, monitor: facesBox, tray: trayPart, keys: keysBox, quick: quickBox, clock: clockBox }
  for (const [part, node] of Object.entries(PART_NODES)) node.dataset.part = part
  const zones = { left: el('span', 'tzone tz-left'), center: el('span', 'tzone tz-center'), right: el('span', 'tzone tz-right') }
  // While the layout is being edited: the one being made, not yet kept.
  let editLayout = null
  let arranged = ''
  let seatSent = null

  // The layout: the one being edited, or as kept, or before any as
  // taskbarAlign had it; each part once, one it lacks (new since) where the
  // default has it; the widgets put there (widgets.js layoutOf). No gear
  // (zhoupengjie/Wakuwaku#38): the settings are a press on her capsule, or
  // her menus', away, as Windows 11's taskbar has none; one kept in an old
  // layout is left out, as a part no longer there.
  function tbLayout(saved = editLayout || config.taskbarLayout) {
    const base = saved && typeof saved === 'object' ? saved : config.taskbarAlign === 'left' ? TB_PRESETS.left : TB_PRESETS.win11
    return Widgets.layoutOf(base, TB_PARTS, TB_PRESETS.win11)
  }
  // Which preset its parts are as ('custom': none), whatever widgets are there.
  const presetOf = layout => {
    const parts = JSON.stringify(Widgets.partsAlone(tbLayout(layout)))
    return Object.keys(TB_PRESETS).find(k => JSON.stringify(tbLayout(TB_PRESETS[k])) === parts) || 'custom'
  }

  // The widgets put there, each its node, kept while it stays there (by its
  // id); dragged by its id as a part by its name while the layout is edited.
  const entryNodes = new Map()
  function entryNode(e) {
    let node = entryNodes.get(e.id)
    if (!node) {
      node = el('span', `tw tw-${e.type}`)
      node.dataset.part = e.id
      node.dataset.entry = e.id
      entryNodes.set(e.id, node)
    }
    node._entry = e
    if (Widgets.isSource(e)) node.dataset.size = e.size === 'S' || isSide() ? 'S' : 'M'
    return node
  }

  // The parts and widgets into their runs, as the layout has them.
  function arrangeTaskbar() {
    const layout = tbLayout()
    const key = JSON.stringify(layout)
    if (key === arranged) return
    arranged = key
    for (const k of Object.keys(zones)) zones[k].replaceChildren(...layout[k].map(p => (typeof p === 'string' ? PART_NODES[p] : entryNode(p))))
    const ids = new Set(Object.values(layout).flatMap(run => run.filter(p => typeof p !== 'string').map(p => p.id)))
    for (const id of [...entryNodes.keys()]) if (!ids.has(id)) entryNodes.delete(id)
    // First at the strip's left end: her island at its very edge, as ever.
    herSlot.classList.toggle('first', layout.left[0] === 'her')
    // A run with a flexible spacer takes the room there is (the middle one
    // not in the middle of the screen then: its spacer has the say).
    const stretch = k => layout[k].some(p => p.type === 'flex')
    for (const k of Object.keys(zones)) zones[k].classList.toggle('stretch', stretch(k))
    barRest.classList.toggle('tb-stretch', Object.keys(zones).some(stretch))
    // What the strip shows takes no turns in the island too.
    takeWidgets(allWidgets)
  }

  // Each widget put there, as big as it is set: S its icon and value; M its
  // value over its name, as the clock has the date under the time. One with
  // nothing to say (not yet, or gone) greyed, with its name. A plugin with
  // several (stocks, CI runs) shows them in turn.
  const ENTRY_TURN_MS = 5000
  function drawEntries() {
    const turn = Math.floor(Date.now() / ENTRY_TURN_MS)
    const detailed = isDetailed()
    for (const node of entryNodes.values()) {
      const e = node._entry
      const name = Widgets.nameOf(lang, e, allWidgets, detailed)
      node.dataset.label = name
      if (!Widgets.isSource(e)) continue
      const from = Widgets.sourcesOf(e, allWidgets)
      const w = from.length ? from[turn % from.length] : null
      const said = w ? Widgets.words(lang, w, detailed) : null
      const colour = (w && stripInk(w.color)) || stripHue().idle
      // A plugin with nothing to say: maybe it wants its settings first.
      if (!w && e.type === 'plugin') askPlugins()
      const needs = !w && needsSettings(e)
      const drawn = JSON.stringify([node.dataset.size, name, said, w?.icon || e.icon, colour, needs])
      if (node._drawn === drawn) continue
      node._drawn = drawn
      const icon = el('span', 'twi')
      icon.innerHTML = Widgets.icon(w?.icon || e.icon || 'dot')
      icon.style.color = colour
      const none = t(lang, needs ? 'taskbar.w.needs' : 'taskbar.w.none')
      node.classList.toggle('empty', !w)
      node.title = w ? [said.label, said.value].filter(Boolean).join(' · ') : `${name} · ${none}`
      if (node.dataset.size === 'S') node.replaceChildren(icon, el('b', 'twv', said?.value || '—'))
      else {
        const text = el('span', 'twt')
        text.append(el('b', 'twv', said?.value || none), el('i', '', name))
        node.replaceChildren(icon, text)
      }
    }
    if (wgFor) drawWidgetPop()
  }

  // The plugins she runs as settings_get says, for which want their
  // settings filled in first (missing: a parameter they need): asked for
  // while a plugin on the strip has nothing to say, twice a minute at most,
  // and once the layout is kept.
  const pluginsSaid = { at: 0, list: [] }
  function askPlugins(now = false) {
    if (!now && Date.now() - pluginsSaid.at < 30000) return
    pluginsSaid.at = Date.now()
    window.pet.settings.get().then(got => tellPlugins(got?.plugins))
  }
  function tellPlugins(list) {
    if (!Array.isArray(list)) return
    pluginsSaid.list = list
    for (const node of entryNodes.values()) node._drawn = ''
    if (taskbarBuilt && home() === 'taskbar') drawEntries()
  }
  const needsSettings = e => e.type === 'plugin' && !!pluginsSaid.list.find(p => p.id === e.of)?.missing
  setInterval(() => {
    if (taskbarBuilt && home() === 'taskbar' && entryNodes.size) drawEntries()
  }, ENTRY_TURN_MS)

  // Her island over its slot, as wide as it is: from the strip's left, or,
  // its slot past the middle, from the right (what it grows into goes the
  // way there is room). Where it lies without OLED's shift, which moves
  // them both after. Her seat (where she is reached for, and flies back
  // to) told to main once it moves. (Named placeHer at first, it took the
  // name of the one that shapes her in the island, which was never called
  // again: she was an empty ring, 2026-10-12.)
  function placeSlot() {
    if (isSide()) return placeSideSlot()
    herSlot.style.height = ''
    herSlot.style.marginBottom = ''
    body.classList.remove('isl-low')
    const pad = isCapsule() ? CAPSULE_X : 0
    herSlot.style.width = `${onPixels(sizeOf('compact').width + 2 * pad)}px`
    herSlot.style.marginRight = `${pad ? 2 : 4}px`
    const shifted = (oledOn && parseFloat(document.documentElement.style.getPropertyValue('--oled-x'))) || 0
    const r = herSlot.getBoundingClientRect()
    const left = r.left - shifted + pad
    const atRight = (r.left + r.right) / 2 - shifted > innerWidth / 2
    body.classList.toggle('isl-right', atRight)
    document.documentElement.style.setProperty('--isl-x', `${onPixels(atRight ? innerWidth - (r.right - shifted - pad) : left)}px`)
    const seat = Math.round(left + taskCompactH() / 2)
    if (seat !== seatSent) {
      seatSent = seat
      window.pet.seat(seat)
    }
  }
  // On a side bar: the slot as tall as her upright capsule, from the bar's
  // top, or from its foot in its lower half (isl-low: what it grows into
  // goes up); her seat her portrait, along the bar from its top.
  function placeSideSlot() {
    const pad = isCapsule() ? CAPSULE_X : 0
    const cap = sizeOf('compact')
    herSlot.style.width = ''
    herSlot.style.marginRight = ''
    herSlot.style.height = `${onPixels(cap.height + 2 * pad)}px`
    herSlot.style.marginBottom = `${pad ? 2 : 4}px`
    const shifted = (oledOn && parseFloat(document.documentElement.style.getPropertyValue('--oled-y'))) || 0
    const r = herSlot.getBoundingClientRect()
    const top = r.top - shifted + pad
    const low = (r.top + r.bottom) / 2 - shifted > innerHeight / 2
    body.classList.remove('isl-right')
    body.classList.toggle('isl-low', low)
    document.documentElement.style.setProperty('--isl-y', `${onPixels(low ? innerHeight - (r.bottom - shifted - pad) : top)}px`)
    const seat = Math.round(top + cap.width / 2)
    if (seat !== seatSent) {
      seatSent = seat
      window.pet.seat(seat)
    }
  }

  // The settings beside a side bar, from her capsule's top (or foot, in the
  // bar's lower half), kept 8 px off the screen's ends.
  function placeSideSettings(height) {
    const at = parseFloat(document.documentElement.style.getPropertyValue('--isl-y')) || TASKBAR_GAP
    const y = Math.max(TASKBAR_GAP, Math.min(at, innerHeight - TASKBAR_GAP - height))
    document.documentElement.style.setProperty('--set-y', `${Math.round(y)}px`)
  }

  // The tray's icons as last sent ({ key, png, tip, exe }), and as drawn.
  let trayIcons = []
  let trayDrawn = ''
  let trayRectsSent = ''
  // The windows' buttons as last sent (taskbar.rs), and as drawn.
  let taskWindows = []
  let windowsDrawn = ''
  // When each window (by its handle) began flashing for attention: its
  // button is made anew with every change to the list, and the flashes
  // carry on from where they were (style.css .flash, --flash-delay)
  // instead of starting over.
  const flashSince = new Map()

  function fillTaskbar() {
    if (!taskbarBuilt) {
      // As wide as the strip: her island lies over its slot in it (placeSlot).
      barRest.style.left = '0px'
      barRest.replaceChildren(glide, zones.left, zones.center, growNode, zones.right, deskEdge, pop, wgPop, imePop)
      taskbarBuilt = true
      trayDrawn = ''
      windowsDrawn = ''
    }
    arrangeTaskbar()
    drawEntries()
    // The programs' buttons as icons alone (taskbarButtons), or with their titles.
    barRest.classList.toggle('icons', isSide() || config.taskbarButtons !== 'labels')
    startButton.title = t(lang, 'taskbar.start')
    quickBox.title = t(lang, 'taskbar.quick')
    deskEdge.title = t(lang, 'taskbar.desktop')
    facesBox.replaceChildren(...widgetFaces())
    drawWindows()
    drawTray()
    tickClock()
    alignApps()
    placeSlot()
    snapStart()
    // Editing: the chosen one's tools over it, where it is now.
    if (editLayout && chosen) drawTools()
  }

  // The middle run in the middle of the screen, as Windows 11 has Start and
  // the programs' buttons (the default layout): moved along by its margin,
  // never back past where it starts nor into the right run (the room before
  // it is all it may take). Moved, it slides; what hangs over a button (its
  // windows' list) is placed again once it is there, and her island too.
  // The room always kept between the middle run and the last (the empty
  // .grow): crowded, the programs give way before it does (style.css).
  const RUNS_APART = 12

  function alignApps() {
    if (!taskbarBuilt) return
    fadeWins()
    const middle = zones.center
    // Along the bar: its left margin, a side bar's top one (the other none).
    const margin = isSide() ? 'marginTop' : 'marginLeft'
    middle.style[isSide() ? 'marginLeft' : 'marginTop'] = ''
    // Measured by the margin as it is now (it may be sliding), set to the one wanted.
    const target = parseFloat(middle.style[margin]) || 0
    let want = 0
    // Empty, it is still a place to drop a part while the layout is edited;
    // with a flexible spacer anywhere, the spacer places it.
    if ((middle.childElementCount || editLayout) && !barRest.classList.contains('tb-stretch')) {
      const now = parseFloat(getComputedStyle(middle)[margin]) || 0
      const start = middle.getBoundingClientRect()
      const length = len(start)
      const room = Math.max(0, len(growNode.getBoundingClientRect()) - RUNS_APART) + now
      // Where it lies without OLED's shift (which moves the bar after): the
      // margin taken from where it is drawn undid the shift, and the middle
      // ones stayed put across.
      const shifted = (oledOn && parseFloat(document.documentElement.style.getPropertyValue(isSide() ? '--oled-y' : '--oled-x'))) || 0
      want = Math.round(Math.max(0, Math.min(((isSide() ? innerHeight : innerWidth) - length) / 2 - (lo(start) - shifted - now), room)))
      // A side bar's middle run gives way when crowded (its programs then
      // scroll), so its own height and the room left are worked out from
      // what everything wants, not from what it has been squeezed to.
      if (isSide()) {
        // Its programs' column scrolls: its whole height, not what it shows.
        const natural = [...middle.children].reduce((a, n) => {
          const cs = getComputedStyle(n)
          return a + (n === windowsBox ? n.scrollHeight : n.offsetHeight) + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0)
        }, 0)
        const others = [...barRest.children].filter(n => n !== middle && n !== growNode && getComputedStyle(n).position !== 'absolute')
        const used = others.reduce((a, n) => {
          const cs = getComputedStyle(n)
          return a + n.offsetHeight + (parseFloat(cs.marginTop) || 0) + (parseFloat(cs.marginBottom) || 0)
        }, 0)
        const free = barRest.clientHeight - (parseFloat(getComputedStyle(barRest).paddingTop) || 0) - used - natural - RUNS_APART
        want = Math.round(Math.max(0, Math.min((innerHeight - natural) / 2 - (start.top - shifted - now), free)))
      }
    }
    if (want !== Math.round(target)) middle.style[margin] = `${want}px`
  }
  zones.center.addEventListener('transitionend', e => {
    if (e.target !== zones.center || (e.propertyName !== 'margin-left' && e.propertyName !== 'margin-top')) return
    if (popFor) drawPop()
    if (glideOn?.isConnected) glideTo(glideOn, true)
    placeSlot()
    snapStart()
    if (editLayout && chosen) drawTools()
    if (wgFor) drawWidgetPop()
    requestAnimationFrame(sendTrayRects)
  })

  // --- The pointer's glide --------------------------------------------------------

  // Under the pointer, one highlight for all the taskbar's buttons that
  // slides from one to the next as her island's drop moves: its leading
  // edge first, the trailing one after, so it stretches and gathers, a
  // little squashed on the way (style.css .hdrop). It comes and goes in
  // place, jumps rather than slides a long way, and gives a little under a
  // press. Not over what opens above the buttons, nor while a tray icon or
  // a program's button is being dragged.
  const GLIDE_ON = '.start, .win, .tout .ticon, .tchev, .tpriv, .kbd .ime, .tquick, .face.mon, .tw-widget, .tw-plugin, .tclock'
  const GLIDE_FAR = 360
  const glide = el('span', 'hdrop')
  let glideOn = null
  let glideTimer

  function glideTo(node, still = false) {
    clearTimeout(glideTimer)
    const bar = barRest.getBoundingClientRect()
    const r = node.getBoundingClientRect()
    // Along the bar its two ends slide (left and right, or a side bar's top
    // and bottom); across it, as the button is.
    const [from, to, crossFrom, crossSize] = isSide() ? ['top', 'bottom', 'left', 'width'] : ['left', 'right', 'top', 'height']
    const left = lo(r) - lo(bar)
    const right = hi(bar) - hi(r)
    const was = glideOn && glide.classList.contains('on') && glide.dataset.axis === from ? parseFloat(glide.style[from]) : null
    if (glide.dataset.axis !== from) {
      for (const p of ['left', 'right', 'top', 'bottom', 'width', 'height']) glide.style[p] = ''
      glide.dataset.axis = from
    }
    glide.style[crossFrom] = `${crossLo(r) - crossLo(bar)}px`
    glide.style[crossSize] = `${isSide() ? r.width : r.height}px`
    if (still || was === null || Math.abs(left - was) > GLIDE_FAR) {
      glide.style.transition = 'none'
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      void glide.offsetWidth
      glide.style.transition = ''
    } else if (left !== was) {
      const lead = left > was ? to : from
      const trail = lead === to ? from : to
      glide.style.transition = `${lead} 0.22s cubic-bezier(0.3, 1.3, 0.5, 1), ${trail} 0.42s cubic-bezier(0.3, 1.45, 0.5, 1), opacity 0.16s ease, transform 0.3s cubic-bezier(0.3, 1.45, 0.5, 1)`
      glide.style[from] = `${left}px`
      glide.style[to] = `${right}px`
      glide.classList.remove('squash')
      void glide.offsetWidth
      glide.classList.add('squash')
    }
    glide.classList.add('on')
    glideOn = node
  }

  function glideOff() {
    clearTimeout(glideTimer)
    glideTimer = setTimeout(() => {
      glide.classList.remove('on', 'down')
      glideOn = null
    }, 90)
  }

  const isGlideOn = node => node?.matches?.(GLIDE_ON) && !node.closest('.wpop, .wgpop, .tfly')
  barRest.addEventListener(
    'mouseenter',
    e => {
      if (!isOn() || home() !== 'taskbar' || !isGlideOn(e.target) || body.classList.contains('tray-dragging') || body.classList.contains('win-dragging')) return
      glideTo(e.target)
    },
    true,
  )
  barRest.addEventListener(
    'mouseleave',
    e => {
      if (e.target === barRest || isGlideOn(e.target)) glideOff()
    },
    true,
  )
  barRest.addEventListener('pointerdown', e => {
    if (e.button === 0 && glideOn && e.target.closest(GLIDE_ON) === glideOn) glide.classList.add('down')
  })
  window.addEventListener('pointerup', () => glide.classList.remove('down'))
  window.addEventListener('resize', () => {
    if (!isOn() || home() !== 'taskbar') return
    alignApps()
    placeSlot()
  })

  // A button for each program (taskbar.rs): those kept on the taskbar
  // first, then the others as their windows came. Its icon; the title of its
  // one window, or its name and how many; none while it does not run (its
  // name then under the pointer). Icons alone (style.css #bar-rest.icons):
  // no title, no count, its windows listed when the pointer rests on it. A
  // line under it, longer for the one in front; lit while a window of it
  // flashes for attention. No marks for the sessions in its windows: the
  // island says how they are.
  // The programs' buttons in the order dragged (taskbarOrder, their keys,
  // those not running kept), the rest after, as they came.
  const appOrder = () => (Array.isArray(config.taskbarOrder) ? config.taskbarOrder.map(String) : [])
  function orderedApps() {
    const order = appOrder()
    const rank = a => (order.includes(a.app) ? order.indexOf(a.app) : Infinity)
    return taskWindows
      .map((a, n) => [a, n])
      .sort((x, y) => rank(x[0]) - rank(y[0]) || x[1] - y[1])
      .map(([a]) => a)
  }

  // A side bar's programs crowded into their column: faded where more of
  // them lies past an end (12 px), as it scrolls.
  function fadeWins() {
    const side = isSide()
    const more = windowsBox.scrollHeight - windowsBox.clientHeight
    windowsBox.classList.toggle('fade-start', side && more > 0 && windowsBox.scrollTop > 0)
    windowsBox.classList.toggle('fade-end', side && more > 0 && windowsBox.scrollTop < more - 1)
  }
  windowsBox.addEventListener('scroll', fadeWins, { passive: true })

  function drawWindows() {
    const apps = orderedApps()
    const drawn = JSON.stringify([apps, lang])
    if (drawn === windowsDrawn) return
    windowsDrawn = drawn
    const now = Date.now()
    const flashing = new Set(apps.flatMap(a => (a.windows || []).filter(w => w.flash).map(w => w.id)))
    for (const id of [...flashSince.keys()]) if (!flashing.has(id)) flashSince.delete(id)
    for (const id of flashing) if (!flashSince.has(id)) flashSince.set(id, now)
    windowsBox.replaceChildren(
      ...apps.map(app => {
        const windows = app.windows || []
        const button = el('span', 'win')
        button.dataset.app = app.app
        // As Windows' own buttons say themselves to a screen reader.
        button.setAttribute('role', 'button')
        button.setAttribute('aria-label', t(lang, !windows.length ? 'taskbar.ariaPinned' : windows.length > 1 ? 'taskbar.ariaMany' : 'taskbar.ariaOne', { name: app.name || '', n: windows.length }))
        button.classList.toggle('front', windows.some(w => w.front))
        button.classList.toggle('min', windows.length > 0 && windows.every(w => w.min))
        const flashes = windows.filter(w => w.flash)
        button.classList.toggle('flash', flashes.length > 0)
        // From the longest-flashing window's start: a negative delay puts
        // the animation that far along (past its end: the last frame).
        if (flashes.length) button.style.setProperty('--flash-delay', `${-Math.max(...flashes.map(w => now - flashSince.get(w.id)))}ms`)
        button.classList.toggle('idle', !windows.length)
        if (!windows.length) button.title = app.name || ''
        if (app.png) {
          const img = el('img')
          img.src = app.png
          img.draggable = false
          button.append(img)
        } else {
          button.append(el('span', 'letter', (app.name || '?').slice(0, 1).toUpperCase()))
        }
        if (windows.length) button.append(el('span', 'wt', windows.length === 1 ? windows[0].title : app.name))
        if (windows.length > 1) button.append(el('span', 'count', String(windows.length)))
        return button
      }),
    )
    if (popFor) drawPop()
  }

  // Above a program's button: its windows to pick from (on a press with
  // several, or the pointer resting on it), or what can be done with it (a
  // right press): open another, keep it on the taskbar or no longer, close.
  const pop = el('div', 'wpop')
  pop.hidden = true
  barRest.append(pop)
  // { app, kind: 'list' | 'menu' } while open, and what it showed last.
  let popFor = null
  let popDrawn = ''
  let popTimer
  const appOf = key => taskWindows.find(a => a.app === key)
  // File Explorer's button, by its program: explorer.exe, the shell itself.
  const isExplorer = path => /(^|[\\/])explorer\.exe$/i.test(path || '')

  function openPop(key, kind) {
    if (wgFor) closeWidgetPop()
    if (imeOpen) closeImeList()
    clearTimeout(popTimer)
    popFor = { app: key, kind }
    drawPop()
    watchPresses()
    // Windows' End task as its setting is now (the look tells it only every
    // half minute): the menu drawn again should it have changed.
    if (kind === 'menu')
      window.pet.taskbar.endTask().then(on => {
        if (on === look.endTask) return
        look.endTask = on === true
        if (popFor?.kind === 'menu') drawPop()
      })
  }

  function closePop() {
    clearTimeout(popMoveTimer)
    popMoving = false
    popShown = null
    pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    clearTimeout(popTimer)
    popFor = null
    pop.hidden = true
    sendThumbs()
    watchPresses()
  }

  // While the windows' list (or menu) or the folded tray icons are open, a
  // press anywhere else closes them, as Windows' do: told by the host
  // (taskbar.rs press hook), for ours never take the focus whose loss would.
  let watching = false
  // The widget on the taskbar whose card is open (openWidgetPop); the input
  // methods' list open (openImeList).
  let wgFor = null
  let imeOpen = false
  function watchPresses() {
    const want = !!popFor || trayOpen || !!wgFor || imeOpen
    if (want === watching) return
    watching = want
    window.pet.taskbar.watch(want)
  }
  window.pet.onPress(at => {
    if (!isOn() || home() !== 'taskbar' || !at) return
    const over = document.elementFromPoint(at.x, at.y)
    if (popFor && !(over && (pop.contains(over) || windowsBox.contains(over)))) closePop()
    if (trayOpen && !(over && trayBox.contains(over))) openTray(false)
    if (wgFor && !(over && (wgPop.contains(over) || entryNodes.get(wgFor)?.contains(over)))) closeWidgetPop()
    if (imeOpen && !(over && (imePop.contains(over) || imeMark.contains(over)))) closeImeList()
  })

  // --- A widget on the taskbar, opened ------------------------------------------------
  // A press on one (or a right press) opens its card above it, as the
  // clock's opens Windows' calendar: what it says (each of a plugin's
  // widgets), where it comes from, and its settings, its mail, or taking it
  // off the taskbar. A press anywhere else closes it (watchPresses).
  const wgPop = el('div', 'wgpop')
  wgPop.hidden = true
  barRest.append(wgPop)
  // The input methods to pick from (openImeList), as a menu of the windows' kind.
  const imePop = el('div', 'wpop menu imepop')
  imePop.hidden = true
  barRest.append(imePop)

  function openWidgetPop(id) {
    if (popFor) closePop()
    if (imeOpen) closeImeList()
    wgFor = id
    drawWidgetPop()
    watchPresses()
  }

  function closeWidgetPop() {
    wgFor = null
    wgPop.hidden = true
    watchPresses()
  }

  function drawWidgetPop() {
    const node = wgFor && entryNodes.get(wgFor)
    const e = node?._entry
    if (!e || !node.isConnected || editLayout || home() !== 'taskbar') return closeWidgetPop()
    const detailed = isDetailed()
    const from = Widgets.sourcesOf(e, allWidgets)
    const first = from[0]
    const head = el('div', 'wghead')
    head.innerHTML = Widgets.icon(first?.icon || e.icon || 'dot')
    head.firstChild.style.color = (first && stripInk(first.color)) || stripHue().idle
    const kind = e.type === 'plugin' ? 'plugin' : first?.from || 'widget'
    head.append(el('b', '', Widgets.nameOf(lang, e, allWidgets, detailed)), el('span', 'wgfrom', t(lang, `taskbar.w.from.${kind}`)))
    const rows = from.map(w => {
      const said = Widgets.words(lang, w, detailed)
      const row = el('div', 'wgrow')
      const value = el('b', '', said.value)
      if (Widgets.partsOf(w)) value.innerHTML = Widgets.partsHTML(w)
      // A plugin's several: each by its own label.
      row.append(...(from.length > 1 && said.label ? [el('span', '', said.label)] : []), value)
      return row
    })
    if (!rows.length) rows.push(el('div', 'wgnone', t(lang, needsSettings(e) ? 'taskbar.lib.needs' : 'taskbar.w.none')))
    const acts = el('div', 'wgacts')
    const button = (what, label) => {
      const b = el('button', 'wgbtn', label)
      b.type = 'button'
      b.dataset.wg = what
      return b
    }
    if (kind === 'plugin' || kind === 'builtIn') acts.append(button('settings', t(lang, 'taskbar.w.settings')))
    acts.append(button('remove', t(lang, 'taskbar.w.remove')))
    wgPop.replaceChildren(head, ...rows, acts)
    wgPop.hidden = false
    const at = besideOrOver(node, wgPop, isSide() ? 8 : 4)
    wgPop.style.left = at.left === undefined ? '' : `${at.left}px`
    wgPop.style.top = at.top === undefined ? '' : `${at.top}px`
  }

  // Off the taskbar, and kept so.
  function removeEntry(id) {
    const layout = tbLayout()
    for (const k of Object.keys(layout)) layout[k] = layout[k].filter(p => Widgets.keyOf(p) !== id)
    if (editLayout) editLayout = layout
    else {
      config.taskbarLayout = layout
      window.pet.settings.set({ taskbarLayout: layout })
    }
    if (wgFor === id) closeWidgetPop()
    arranged = ''
    update()
    if (libOpen) drawLibrary()
  }

  wgPop.addEventListener('click', e => {
    const what = e.target.closest('[data-wg]')?.dataset.wg
    const entry = wgFor && entryNodes.get(wgFor)?._entry
    if (!what || !entry) return
    if (what === 'remove') return removeEntry(entry.id)
    closeWidgetPop()
    openWidgetSettings(entry.type === 'plugin' ? { plugin: entry.of } : { id: entry.of })
  })

  // A widget's place in the settings: its row on the plugins page, its own
  // settings folded out (the monitor's, a plugin's she runs).
  function openWidgetSettings(w) {
    const key = w.plugin || w.id
    if (w.plugin || key === 'monitor') window.Settings?.openPlugin?.(key)
    window.pet.openSettings(key ? `widgets:${key}` : 'widgets')
  }
  wgPop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  function drawPop() {
    const app = popFor && appOf(popFor.app)
    const button = app && windowsBox.querySelector(`[data-app="${CSS.escape(app.app)}"]`)
    if (!app || !button) return closePop()
    const windows = app.windows || []
    const rows = []
    if (popFor.kind === 'list') {
      // A card for each: its icon, title and ✕, and room under them for
      // its live picture (taskbar.rs thumbs).
      for (const w of windows) {
        const card = el('div', w.front ? 'wcard front' : 'wcard')
        card.dataset.win = w.id
        const top = el('div', 'wtop')
        if (w.png || app.png) {
          const img = el('img')
          img.src = w.png || app.png
          top.append(img)
        }
        top.append(el('span', 'wt', w.title))
        const x = el('span', 'wx', '✕')
        x.dataset.close = w.id
        top.append(x)
        const room = el('div', 'wthumb')
        room.dataset.thumb = w.id
        card.append(top, room)
        rows.push(card)
      }
    } else {
      const item = (text, act) => {
        const row = el('div', 'witem', text)
        row.dataset.act = act
        return row
      }
      const head = el('div', 'whead', app.name)
      rows.push(head, item(t(lang, windows.length ? 'taskbar.newWindow' : 'taskbar.open'), 'launch'), item(t(lang, app.pinned ? 'taskbar.unpin' : 'taskbar.pin'), app.pinned ? 'unpin' : 'pin'))
      // Windows' End task while its setting is on (zhoupengjie/Wakuwaku#36),
      // over the close as on Windows' menu: the program's windows' processes
      // ended (tasks.rs end_task). None for File Explorer, as Windows offers
      // none: its windows are the shell's own (explorer.exe).
      if (windows.length && look.endTask && !isExplorer(app.path)) rows.push(item(t(lang, windows.length > 1 ? 'taskbar.endAll' : 'taskbar.end'), 'end'))
      if (windows.length) rows.push(item(t(lang, windows.length > 1 ? 'taskbar.closeAll' : 'taskbar.close'), 'close'))
    }
    if (!rows.length) return closePop()
    // Another program's windows taking the place of the last's (the pointer
    // gone along the buttons): where the list was and how big, to slide from.
    const isList = popFor.kind === 'list'
    const from = isList && !pop.hidden && pop.classList.contains('list') && popShown && popShown !== app.app ? { [isSide() ? 'top' : 'left']: isSide() ? pop.offsetTop : pop.offsetLeft, width: pop.offsetWidth, height: pop.offsetHeight } : null
    const opening = pop.hidden
    // Drawn again only when what it shows changed: a card replaced under a
    // press is never clicked (the windows' list comes again often).
    const drawn = JSON.stringify([popFor.app, popFor.kind, app.name, app.pinned, windows.map(w => [w.id, w.title, w.front, (w.png || '').length]), lang, look.endTask])
    if (drawn !== popDrawn || pop.hidden) {
      popDrawn = drawn
      pop.replaceChildren(...rows)
    }
    pop.classList.toggle('menu', popFor.kind === 'menu')
    pop.classList.toggle('list', isList)
    pop.hidden = false
    popShown = app.app
    // Mid-slide, the list keeps going where it was going.
    if (popMoving && !from) return
    // Over its button (beside it on a side bar, 6 px off the bar), kept
    // within the screen, at its own size.
    pop.classList.remove('sliding', 'rising')
    pop.style.width = ''
    pop.style.height = ''
    const to = { ...besideOrOver(button, pop, isSide() ? 8 : 4), width: pop.offsetWidth, height: pop.offsetHeight }
    if (from) return slidePop(from, to)
    pop.style.left = to.left === undefined ? '' : `${to.left}px`
    pop.style.top = to.top === undefined ? '' : `${to.top}px`
    if (opening && isList) return risePop()
    requestAnimationFrame(sendThumbs)
  }

  // The list moving from one program's button to the next as Windows' own
  // does: sliding over and taking its new size, the new cards coming in from
  // the side it moves to. Windows draws the live pictures, which cannot
  // slide with it: they go for the slide and come back once it is there.
  // Opening, it rises a little into place, the pictures after it.
  const POP_SLIDE_MS = 300
  let popShown = null
  let popMoving = false
  let popMoveTimer

  function holdThumbs() {
    popMoving = true
    clearTimeout(popMoveTimer)
    thumbsSent = '[]'
    window.pet.taskbar.thumbs([])
  }

  function settlePop(ms) {
    popMoveTimer = setTimeout(() => {
      popMoving = false
      // A slide not quite done by now would hold the list at its old size.
      pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
      pop.classList.remove('sliding', 'rising')
      pop.style.width = ''
      pop.style.height = ''
      sendThumbs()
    }, ms)
  }

  // Where a pop goes in the bar (its px): centred over a node along the bar,
  // or beside it on a side bar (its place there by style.css), kept this
  // far from the bar's ends.
  function besideOrOver(node, popNode, keep) {
    const r = node.getBoundingClientRect()
    const base = barRest.getBoundingClientRect()
    if (isSide()) return { top: Math.round(Math.max(keep, Math.min(r.top - base.top + r.height / 2 - popNode.offsetHeight / 2, barRest.offsetHeight - popNode.offsetHeight - keep))) }
    return { left: Math.round(Math.max(keep, Math.min(r.left - base.left + r.width / 2 - popNode.offsetWidth / 2, barRest.offsetWidth - popNode.offsetWidth - keep))) }
  }

  function slidePop(from, to) {
    holdThumbs()
    // Along the bar: across a bottom or top one, up and down a side one.
    const key = 'top' in to ? 'top' : 'left'
    pop.style.setProperty('--come', `${to[key] > from[key] ? 16 : -16}px`)
    pop.classList.add('sliding')
    const box = b => ({ [key]: `${b[key]}px`, width: `${b.width}px`, height: `${b.height}px` })
    Object.assign(pop.style, box(to))
    pop.getAnimations().forEach(a => a.id === 'slide' && a.cancel())
    const slide = pop.animate([box(from), box(to)], { duration: POP_SLIDE_MS, easing: 'cubic-bezier(0.3, 1.2, 0.5, 1)' })
    slide.id = 'slide'
    settlePop(POP_SLIDE_MS)
  }

  function risePop() {
    holdThumbs()
    pop.classList.add('rising')
    settlePop(180)
  }

  // The live pictures where the cards left room for them; none once closed.
  let thumbsSent = ''
  function sendThumbs() {
    // Not while the list slides or rises: they come once it is in place.
    if (popMoving) return
    const items = popFor?.kind === 'list' ? [...pop.querySelectorAll('[data-thumb]')].map(r => {
      const b = r.getBoundingClientRect()
      return [Number(r.dataset.thumb), b.left, b.top, b.width, b.height]
    }) : []
    const sent = JSON.stringify(items)
    if (sent === thumbsSent) return
    thumbsSent = sent
    window.pet.taskbar.thumbs(items)
  }
  window.addEventListener('resize', () => {
    thumbsSent = ''
    requestAnimationFrame(sendThumbs)
  })

  // A press on a program's button: one not running starts; one window goes
  // to the front, or is minimized when in front; several, their list at
  // once to pick one from, as Windows 11's own (zhoupengjie/Wakuwaku#34:
  // they all came to the front together, or all went down), and pressed
  // again it goes. One the pointer rested on stays open for the press.
  // With the keys and the other buttons as on Windows' own
  // (zhoupengjie/Wakuwaku#36, presses.js pressOf): the middle button or
  // Shift, another of it; Ctrl and Shift, another as administrator; Ctrl on
  // one with several windows, the next of them; a right press, its menu,
  // with Shift its window's own menu (one for all its windows when it has
  // several).
  function pressApp(app, what) {
    const windows = app.windows || []
    if (!what) return
    if (what === 'menu') return openPop(app.app, 'menu')
    if (what !== 'press') closePop()
    if (what === 'launch' || what === 'admin') return window.pet.taskbar.app(app.path, what, app.id)
    if (what === 'windowMenu') {
      menuUp = true
      return window.pet.taskbar.windowMenu(windows.map(w => w.id)).finally(() => (menuUp = false))
    }
    if (what === 'next') {
      const last = cycled?.app === app.app && Date.now() - cycled.at < window.Presses.CYCLED_MS ? cycled.id : null
      const next = window.Presses.nextWindow(windows, last)
      cycled = { app: app.app, id: next.id, at: Date.now() }
      return window.pet.taskbar.window(next.id, 'front')
    }
    if (!windows.length) return window.pet.taskbar.app(app.path, 'launch', app.id)
    if (windows.length > 1) {
      const listed = popFor?.kind === 'list' && popFor.app === app.app && !pop.hidden
      if (listed && popFor.pressed) return closePop()
      if (!listed) openPop(app.app, 'list')
      popFor.pressed = true
      return
    }
    closePop()
    window.pet.taskbar.window(windows[0].id, 'press')
  }
  // The window Ctrl and a press last brought, for the next to go on from
  // it while its mark in front has not come yet ({ app, id, at }).
  let cycled = null
  // A window's own menu open (Shift and a right press): no list of windows
  // on the pointer resting on the button, which came up over the menu.
  let menuUp = false
  // Shift and Ctrl as Windows has them at the press (taskbar_held), there
  // by the time the button comes up: her window never takes the keyboard,
  // and what the page's own events say of them may be behind. Theirs only
  // when it could not say.
  let heldAtPress = null
  windowsBox.addEventListener('pointerdown', e => {
    if (e.target.closest('[data-app]')) heldAtPress = window.pet.taskbar.held()
  })
  function pressAt(e) {
    const key = e.target.closest('[data-app]')?.dataset.app
    const press = { type: e.type, button: e.button }
    const own = { shift: e.shiftKey, ctrl: e.ctrlKey }
    const held = heldAtPress
    heldAtPress = null
    Promise.resolve(held).then(keys => {
      const app = appOf(key)
      if (app) pressApp(app, window.Presses.pressOf(press, keys || own, (app.windows || []).length))
    })
  }
  windowsBox.addEventListener('click', e => {
    if (!appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.stopPropagation()
    // The end of a drag, not a press.
    if (winDragged) {
      winDragged = false
      return
    }
    pressAt(e)
  })
  windowsBox.addEventListener('auxclick', e => {
    if (e.button !== 1 || !appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.stopPropagation()
    pressAt(e)
  })
  // The middle button pressed: no scrolling by the pointer (Chromium's), on
  // a side bar's column of buttons.
  windowsBox.addEventListener('mousedown', e => e.button === 1 && e.preventDefault())
  windowsBox.addEventListener('contextmenu', e => {
    if (!appOf(e.target.closest('[data-app]')?.dataset.app)) return
    e.preventDefault()
    e.stopPropagation()
    pressAt(e)
  })
  // A program's button dragged along the taskbar: put where it is let go
  // among the others, before the one under the pointer (after it past its
  // middle), the order kept (taskbarOrder). { app, x, y, ghost } while held.
  let winPress = null
  let winDragged = false
  windowsBox.addEventListener('pointerdown', e => {
    const key = e.target.closest('[data-app]')?.dataset.app
    if (e.button !== 0 || !key) return
    winPress = { app: key, x: e.clientX, y: e.clientY, ghost: null }
  })
  windowsBox.addEventListener('pointermove', e => {
    if (!winPress) return
    // Let go of off the buttons before a drag began: nothing held now.
    if (!(e.buttons & 1)) return cancelWinDrag()
    if (!winPress.ghost && Math.hypot(e.clientX - winPress.x, e.clientY - winPress.y) > 6) {
      // Held from here on, wherever the pointer goes; only now, for a
      // captured press clicks on the box, not on its button.
      try {
        windowsBox.setPointerCapture(e.pointerId)
      } catch {}
      const from = windowsBox.querySelector(`[data-app="${CSS.escape(winPress.app)}"] img`)
      winPress.ghost = el('img', 'tghost')
      if (from) winPress.ghost.src = from.src
      body.append(winPress.ghost)
      body.classList.add('win-dragging')
      clearTimeout(popTimer)
      closePop()
    }
    if (winPress.ghost) {
      Object.assign(winPress.ghost.style, { left: `${e.clientX - 10}px`, top: `${e.clientY - 10}px` })
      markWinDrop(winDropAt(e.clientX, e.clientY, winPress.app))
    }
  })
  windowsBox.addEventListener('pointerup', e => {
    const press = winPress
    winPress = null
    if (!press?.ghost) return
    press.ghost.remove()
    body.classList.remove('win-dragging')
    markWinDrop(null)
    // The click that follows the release is the drag's.
    winDragged = true
    setTimeout(() => (winDragged = false), 0)
    const drop = winDropAt(e.clientX, e.clientY, press.app)
    if (!drop) return
    const order = appOrder()
    for (const a of orderedApps()) if (!order.includes(a.app)) order.push(a.app)
    order.splice(order.indexOf(press.app), 1)
    order.splice(order.indexOf(drop.target) + (drop.after ? 1 : 0), 0, press.app)
    window.pet.settings.set({ taskbarOrder: order.slice(0, 200) })
  })
  // Cancelled, or the pointer let go of before the button came up (the
  // window letting the mouse through once it left the taskbar): no drop.
  const cancelWinDrag = () => {
    winPress?.ghost?.remove()
    winPress = null
    body.classList.remove('win-dragging')
    markWinDrop(null)
  }
  windowsBox.addEventListener('pointercancel', cancelWinDrag)
  windowsBox.addEventListener('lostpointercapture', () => winPress && cancelWinDrag())

  // Where a dragged button would land at (x, y): beside the button whose
  // span holds x (past the last, after it); null when let go off the
  // taskbar, or on itself.
  function winDropAt(x, y, key) {
    const strip = windowsBox.getBoundingClientRect()
    const [a, c] = [along(x, y), across(x, y)]
    if (c < crossLo(strip) - 24 || c > crossHi(strip) + 24) return null
    const buttons = [...windowsBox.querySelectorAll('[data-app]')]
    const self = buttons.find(b => b.dataset.app === key)
    const r0 = self?.getBoundingClientRect()
    if (r0 && a >= lo(r0) && a <= hi(r0)) return null
    const others = buttons.filter(b => b !== self)
    if (!others.length) return null
    for (const b of others) {
      const r = b.getBoundingClientRect()
      if (a < hi(r)) return { target: b.dataset.app, after: a > lo(r) + len(r) / 2, slot: b }
    }
    const last = others[others.length - 1]
    return { target: last.dataset.app, after: true, slot: last }
  }

  let winMarked = null
  function markWinDrop(drop) {
    if (winMarked) winMarked.classList.remove('drop-before', 'drop-after')
    winMarked = drop?.slot || null
    if (winMarked) winMarked.classList.add(drop.after ? 'drop-after' : 'drop-before')
  }

  // The pointer resting on a running program's button: its windows.
  windowsBox.addEventListener('mouseover', e => {
    const app = appOf(e.target.closest('[data-app]')?.dataset.app)
    clearTimeout(popTimer)
    if (winPress?.ghost || menuUp) return
    if (!app || popFor?.kind === 'menu') return
    if (popFor?.app === app.app) return
    popTimer = setTimeout(() => (app.windows || []).length && openPop(app.app, 'list'), popFor ? 0 : 500)
  })
  // Gone from the button and the list a moment: it closes.
  for (const node of [windowsBox, pop]) {
    node.addEventListener('mouseleave', () => {
      clearTimeout(popTimer)
      popTimer = setTimeout(closePop, 400)
    })
    node.addEventListener('mouseenter', () => popFor && clearTimeout(popTimer))
  }

  // A file dragged from another program and held over a running program's
  // button half a second: its window to the front, or with several their
  // list open, a card in it held over as long bringing that one, for the
  // file to be let go of in the window, as on Windows' own
  // (zhoupengjie/Wakuwaku#36). The drag's place comes from Tauri
  // (bridge.js onFileDrag, which also plays it as the pointer's: the list
  // goes once the drag has left it and the buttons a moment, as for the
  // pointer). What it is held over: 'win:<id>', 'app:<key>', or null.
  let fileHeld = null
  let fileTimer
  window.pet.onFileDrag((type, at) => {
    const live = at && (type === 'enter' || type === 'over') && isOn() && home() === 'taskbar' && !editLayout
    const node = live ? document.elementFromPoint(at.x, at.y) : null
    const card = node && pop.contains(node) ? node.closest('[data-win]') : null
    const button = !card && node && windowsBox.contains(node) ? node.closest('[data-app]') : null
    const held = card ? `win:${card.dataset.win}` : button ? `app:${button.dataset.app}` : null
    if (held === fileHeld) return
    fileHeld = held
    clearTimeout(fileTimer)
    if (!held) return
    fileTimer = setTimeout(() => {
      if (card) return window.pet.taskbar.window(card.dataset.win, 'drag')
      const app = appOf(button.dataset.app)
      const windows = app?.windows || []
      if (windows.length === 1) {
        closePop()
        window.pet.taskbar.window(windows[0].id, 'drag')
      } else if (windows.length > 1 && !(popFor?.kind === 'list' && popFor.app === app.app && !pop.hidden)) {
        openPop(app.app, 'list')
      }
    }, window.Presses.DRAG_HOLD_MS)
  })

  // On the press, not the click: what was pressed may be drawn anew before
  // the button comes up.
  pop.addEventListener('click', e => e.stopPropagation())
  pop.addEventListener('pointerdown', e => {
    if (e.button !== 0) return
    e.stopPropagation()
    const app = popFor && appOf(popFor.app)
    if (!app) return
    const close = e.target.closest('[data-close]')
    if (close) return window.pet.taskbar.window(close.dataset.close, 'close')
    const row = e.target.closest('[data-win]')
    // A window picked: to the front, never minimized (taskbar.rs 'front').
    if (row) {
      closePop()
      return window.pet.taskbar.window(row.dataset.win, 'front')
    }
    const act = e.target.closest('[data-act]')?.dataset.act
    if (!act) return
    closePop()
    if (act === 'close' || act === 'end') for (const w of app.windows || []) window.pet.taskbar.window(w.id, act)
    else window.pet.taskbar.app(app.path, act, app.id)
  })
  pop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // The script's mark by the layout's language: its own and plain letters.
  const SCRIPTS = { 0x04: ['中', '英'], 0x11: ['あ', 'A'], 0x12: ['한', 'A'] }
  const LANGS = { 0x09: 'EN', 0x07: 'DE', 0x0c: 'FR', 0x0a: 'ES', 0x10: 'IT', 0x16: 'PT', 0x19: 'RU', 0x13: 'NL', 0x1f: 'TR' }
  function langMark(langId, native) {
    const primary = (langId || 0) & 0x3ff
    const script = SCRIPTS[primary]
    return { script, text: script ? script[native === false ? 1 : 0] : LANGS[primary] || primary.toString(16).toUpperCase() }
  }
  // The mode's word in the tip where Windows gives none of its own.
  const MODE_WORDS = { 中: '中文', 英: '英文' }
  // The input methods' icons (taskbar.rs taskbar:imeIcons), by key: each
  // sent once, named by the keys after it.
  const imeIcons = {}
  let keysNow = null
  let imeDrawn = ''
  // As Windows' own taskbar shows the input method in front (which one it
  // is, inputs.rs): Microsoft's own, and a plain layout, by the script's
  // mark (中 or 英), as Windows draws its glyph; one of another maker
  // (Sogou's, WeChat's) by the picture it gives of its mode, as Windows
  // shows it, or else its own icon beside 中 or 英, so two of one language
  // can be told apart. Its name, its mode and its language in the tip.
  function showIme(keys) {
    const { script, text } = langMark(keys?.lang, keys?.native)
    const ime = keys?.ime || null
    const theirs = !!ime && !ime.own
    const modePic = (theirs && ime.mode?.icon && imeIcons[ime.mode.icon]) || ''
    const ownPic = (theirs && !modePic && ime.icon && imeIcons[ime.icon]) || ''
    const mode = ime?.mode?.tip || (script ? MODE_WORDS[text] || text : '')
    const tip = ime?.name ? [[ime.name, mode].filter(Boolean).join(' · '), ime.lang].filter(Boolean).join('\n') : ''
    const drawn = JSON.stringify([text, modePic, ownPic, tip])
    if (drawn !== imeDrawn) {
      imeDrawn = drawn
      const pic = modePic || ownPic
      const img = pic ? el('img', 'pic') : null
      if (img) img.src = pic
      imeMark.replaceChildren(...(img ? [img] : []), ...(modePic ? [] : [el('span', 'script', text)]))
      imeMark.title = tip
    }
    imeMark.classList.toggle('unknown', !modePic && !!script && (keys?.native == null || !!ime?.mode?.off))
  }
  window.pet.onImeIcons(icons => {
    Object.assign(imeIcons, icons || {})
    if (keysNow) showIme(keysNow)
  })
  window.pet.onKeys(keys => {
    keysNow = keys
    showIme(keys)
    capsMark.classList.toggle('on', !!keys?.caps)
    capsMark.title = keys?.caps ? 'Caps Lock' : ''
    showNet(keys?.net)
    showPrivacy(keys?.use)
    showVolume(keys?.volume)
    showBattery(keys?.battery)
  })

  // A right press on the mark: the input methods to pick from, as Windows'
  // own list has them (Win+Space), in its order, each with its icon (a
  // layout by its language's mark), the one in front marked; a press on one
  // switches the window in front to it (inputs.rs pick), and the last row
  // opens Windows' languages. Windows' own list where none can be had.
  async function openImeList() {
    if (popFor) closePop()
    if (wgFor) closeWidgetPop()
    const list = await window.pet.taskbar.inputs()
    if (!Array.isArray(list) || !list.length) return window.pet.taskbar.open('inputs')
    const rows = list.map(m => {
      const row = el('div', m.current ? 'wrow irow front' : 'wrow irow')
      row.dataset.input = m.id
      if (m.icon) {
        const img = el('img')
        img.src = m.icon
        row.append(img)
      } else row.append(el('span', 'itile', langMark(m.lang, true).text))
      const lines = el('span', 'ilines')
      // As Windows' list has them: the language, the input method under it.
      lines.append(el('span', 'wt', m.langName), el('span', 'iname', m.name))
      row.append(lines)
      return row
    })
    const more = el('div', 'witem', t(lang, 'taskbar.imeMore'))
    more.dataset.input = ''
    imePop.replaceChildren(...rows, more)
    imePop.hidden = false
    imeOpen = true
    const at = besideOrOver(imeMark, imePop, isSide() ? 8 : 4)
    imePop.style.left = at.left === undefined ? '' : `${at.left}px`
    imePop.style.top = at.top === undefined ? '' : `${at.top}px`
    watchPresses()
  }

  function closeImeList() {
    imeOpen = false
    imePop.hidden = true
    watchPresses()
  }

  imePop.addEventListener('click', e => {
    const row = e.target.closest('[data-input]')
    if (!row) return
    closeImeList()
    if (row.dataset.input) window.pet.taskbar.input(row.dataset.input)
    else window.pet.taskbar.open('languages')
  })
  imePop.addEventListener('contextmenu', e => {
    e.preventDefault()
    e.stopPropagation()
  })

  // --- The taskbar's look: Mica, light or dark, Windows' accent -------------------------

  // Behind the strip's parts: the desktop's picture under it, blurred and
  // tinted (mica.js) for Windows' mode, as taskbar.rs sent it last; drawn
  // again only when the picture, the mode or the strip's width changes. None
  // on a solid strip. On a clear one (tb-clear) it is drawn too, unseen, if
  // the strip is to fill in as Mica (tb-clear-mica): it fades in at once
  // while a window is maximized or full screen on its display (tb-filled,
  // drawn again as the windows change); filled in solid, tb-solid. The
  // strip light in Windows' light mode (tb-light), her end a capsule
  // wherever the strip is not black (tb-capsule).
  // The strip as thick as it lies on the screen (island.rs tell_strip): its
  // physical px over the scale, a fraction of a CSS px if need be, so its
  // inner edge is the work area's, where maximized windows stop (a hairline
  // gap at 175%: zhoupengjie/Wakuwaku#2). 48 until told.
  let stripThick = 0
  function setStrip() {
    if (!stripThick) return
    document.documentElement.style.setProperty('--strip-h', `${stripThick / (devicePixelRatio || 1)}px`)
    fitPage()
    // Once laid out for it (its edge may have just changed too).
    setTimeout(() => {
      const r = island.parentElement.getBoundingClientRect()
      const dpr = devicePixelRatio || 1
      console.log(`taskbar: strip in the page ${[r.left, r.top, r.right, r.bottom].map(v => +(v * dpr).toFixed(2)).join(', ')} (x ${dpr}, told ${stripThick} px)`)
    }, 400)
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
  // The page as big as its window (told with the strip, its physical size).
  // At 175% Chromium makes the page a little bigger than its window (the
  // window's 1204 px over 1.75 come to 688.00006 in its sums, so 689 CSS
  // px): what lies along the page's foot or right end lay past the window's,
  // the strip 2 px low and a gap above it (zhoupengjie/Wakuwaku#19). The
  // overhang cut off (style.css body.home-taskbar); a few px at most, else
  // it is the window being resized under a size told before.
  let winPx = null
  function fitPage() {
    const root = document.documentElement
    const r = root.getBoundingClientRect()
    const dpr = devicePixelRatio || 1
    const over = (have, want) => (have - want / dpr > 0.01 && have - want / dpr < 4 ? have - want / dpr : 0)
    root.style.setProperty('--page-cut-x', `${winPx ? over(r.width, winPx[0]) : 0}px`)
    root.style.setProperty('--page-cut-y', `${winPx ? over(r.height, winPx[1]) : 0}px`)
  }
  window.pet.onStrip(({ thick, win } = {}) => {
    if (!(thick > 0)) return
    stripThick = thick
    if (Array.isArray(win) && win[0] > 0 && win[1] > 0) winPx = win
    setStrip()
  })
  window.addEventListener('resize', () => {
    if (!stripThick) return
    document.documentElement.style.setProperty('--strip-h', `${stripThick / (devicePixelRatio || 1)}px`)
    fitPage()
  })

  const micaBox = el('div')
  micaBox.id = 'mica'
  island.parentElement.prepend(micaBox)
  const mica = Mica.mount(micaBox)
  let paper = null

  function drawMica() {
    const clearMica = isClear() && clearWhen() === 'mica'
    body.classList.toggle('tb-mica', isMica())
    body.classList.toggle('tb-light', isLight())
    body.classList.toggle('tb-capsule', isCapsule())
    body.classList.toggle('tb-clear', isClear())
    body.classList.toggle('tb-clear-mica', clearMica)
    body.classList.toggle('tb-filled', isFilled())
    body.classList.toggle('tb-solid', isFilled() && clearWhen() === 'solid')
    if (isMica() || clearMica) mica.set(paper, isSide() ? TASKBAR_H : innerWidth, isSide() ? innerHeight : TASKBAR_H, look.mode)
    else mica.clear()
    drawIslandLook()
  }

  // Windows' mode: as taskbar.rs tells the taskbar (taskbar:look), and in
  // pet:update at the top of the screen (winLight), where there is no
  // taskbar to hear it from.
  const modeNow = () => (home() === 'taskbar' ? look.mode : now.winLight === true ? 'light' : 'dark')
  // The look as the settings give it now (none: the defaults): on the
  // taskbar where her end is a capsule (islandLook), at the top of the
  // screen its own (islandTopLook, zhoupengjie/Wakuwaku#27); and its
  // ground's colour (null: none given), how solid (0.2 to 1; solid when no
  // colour is given) and blurred.
  const lookNow = () => (home() === 'taskbar' ? (isCapsule() && config.islandLook?.[look.mode]) || {} : config.islandTopLook?.[modeNow()] || {})
  function groundOf(mine) {
    const bg = /^#[0-9a-f]{6}$/i.test(mine.bg || '') ? parseInt(mine.bg.slice(1), 16) : null
    const op = bg !== null && Number.isFinite(mine.op) ? Math.min(100, Math.max(20, mine.op)) / 100 : 1
    const blur = Number.isFinite(mine.blur) ? Math.min(60, Math.max(0, mine.blur)) : 0
    return { bg, op, blur }
  }

  // Her island's look on the taskbar, as the settings may set it for each of
  // Windows' modes (islandLook: { dark, light }, each { bg: '#rrggbb', op: 20
  // to 100, blur: 0 to 60, edge: 'line' | 'ring' | 'none', bare }), over
  // Windows' own flyout grey (style.css --i-bg, --i-blur, --i-edge,
  // island-bare); a part left out keeps its default. The ring is the
  // session's colour (--ring, as the island draws). At the top of the
  // screen the same from its own (islandTopLook), over black; there the
  // page has nothing behind the island to blur: only Windows' acrylic
  // blurs it (placePlate).
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
    if (blur && home() === 'taskbar') island.style.setProperty('--i-blur', `blur(${blur}px) saturate(1.25)`)
    else island.style.removeProperty('--i-blur')
    const edge = {
      ring: '0 0 0 1.5px var(--ring), 0 0 12px color-mix(in srgb, var(--ring) 35%, transparent)',
      none: '0 0 0 0 transparent',
    }[mine.edge]
    if (edge) island.style.setProperty('--i-edge', edge)
    else island.style.removeProperty('--i-edge')
    // Bare unless said otherwise in the dark mode (the default since
    // 2026-10-13, settings.js ILOOK_DEFAULT), not in the light.
    body.classList.toggle('island-bare', isCapsule() && (mine.bare ?? look.mode === 'dark') === true)
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
  // ground: Windows' light mode's, or a light one of the settings' own
  // (groundLight; null: none given); in the settings, theirs, as Windows'
  // mode.
  function setInk() {
    const own = view === 'settings' ? null : groundLight
    inkLight = own === null ? isLight() : own
    body.classList.toggle('isl-light', inkLight)
  }

  window.pet.onWallpaper(p => {
    paper = p
    drawMica()
  })

  // Windows' mode and accent colour (taskbar.rs). The accent as its own
  // taskbar draws with it: a lighter one on the dark strip, a darker one on
  // the light (style.css --win-accent, for the monitor's icons and the line
  // under the window in front; its own blue until it comes). Not --accent:
  // that is her mood's colour (pet.js).
  window.pet.onLook(l => {
    look = { mode: l?.mode === 'light' ? 'light' : 'dark', accent: l?.accent || null, endTask: l?.endTask === true }
    // How many times a button flashes for attention (style.css .flash).
    if (l?.flashes !== undefined) barRest.style.setProperty('--flashes', l.flashes === 0 ? 'infinite' : String(l.flashes))
    const colour = look.accent && (look.mode === 'light' ? look.accent.dark : look.accent.light)
    if (colour) document.documentElement.style.setProperty('--win-accent', colour)
    else document.documentElement.style.removeProperty('--win-accent')
    if (isOn() && home() === 'taskbar') {
      drawMica()
      update()
    }
  })
  window.addEventListener('resize', drawMica)

  window.pet.onWindows(list => {
    taskWindows = Array.isArray(list) ? list : []
    if (isOn() && home() === 'taskbar') {
      drawWindows()
      alignApps()
      snapStart()
      drawMica()
      setDeep(true)
    }
  })

  // The time and the date, as Windows' taskbar has them.
  function tickClock() {
    const d = new Date()
    const time = d.toLocaleTimeString(lang === 'zh' ? 'zh-CN' : undefined, { hour: '2-digit', minute: '2-digit' })
    const date = d.toLocaleDateString(lang === 'zh' ? 'zh-CN' : undefined, isSide() ? { month: 'numeric', day: 'numeric' } : { year: 'numeric', month: 'numeric', day: 'numeric' })
    if (clockBox.dataset.at !== time + date) {
      clockBox.dataset.at = time + date
      clockBox.replaceChildren(el('b', '', time), el('i', '', date))
      tipClock()
    }
  }
  setInterval(() => isOn() && home() === 'taskbar' && tickClock(), 5000)
  // Its tip as Windows' own (zhoupengjie/Wakuwaku#36, clock.rs): the date in
  // full with its weekday, in the person's own format, and Windows'
  // additional clocks; read anew each minute and as the pointer comes.
  function tipClock() {
    window.pet.taskbar.clockTip().then(tip => {
      if (typeof tip === 'string' && tip) clockBox.title = tip
    })
  }
  clockBox.addEventListener('pointerenter', tipClock)

  // The tray, as Windows' has it: the icons kept out on the taskbar, and
  // the rest folded away behind ^, open above it on a press. Where each is
  // kept: the person's choice (dragged from one to the other, trayPinned),
  // else Windows' own (windowsOut). In what order: as dragged (trayOrder,
  // names, those not running kept too), the rest after, as they came.
  const isOut = i => config.trayPinned?.[i.name] ?? i.windowsOut === true
  const trayOrder = () => (Array.isArray(config.trayOrder) ? config.trayOrder.map(String) : [])
  function orderedTray() {
    const order = trayOrder()
    const rank = i => (order.includes(i.name) ? order.indexOf(i.name) : Infinity)
    return trayIcons
      .map((i, n) => [i, n])
      .sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1])
      .map(([i]) => i)
  }
  const chevron = el('span', 'tchev')
  chevron.append(el('span', 'fi', ''))
  chevron.setAttribute('role', 'button')
  const outBox = el('span', 'tout')
  const flyout = el('span', 'tfly')
  let trayOpen = false

  function trayIcon(i) {
    const slot = el('span', 'ticon')
    slot.dataset.tray = i.key
    slot.dataset.name = i.name
    slot.title = i.tip || i.exe
    slot.setAttribute('role', 'button')
    slot.setAttribute('aria-label', i.tip || i.name || i.exe || '')
    if (i.png) {
      const img = el('img')
      img.src = i.png
      img.draggable = false
      slot.append(img)
      markMono(slot, img)
    }
    return slot
  }

  // A tray icon in one colour only, white or black, as some programs and
  // Windows' own (safely remove hardware, the microphone in use) draw them
  // for one kind of taskbar whatever Windows' mode: marked (data-mono), so
  // the other kind turns it over to read (style.css). Each picture looked at
  // once.
  const monoOf = new Map()
  function markMono(slot, img) {
    const mark = mono => {
      if (mono) slot.dataset.mono = mono
      else delete slot.dataset.mono
    }
    if (monoOf.has(img.src)) return mark(monoOf.get(img.src))
    const look = () => {
      const c = document.createElement('canvas')
      c.width = img.naturalWidth
      c.height = img.naturalHeight
      const g = c.getContext('2d')
      g.drawImage(img, 0, 0)
      const d = g.getImageData(0, 0, c.width, c.height).data
      let seen = 0
      let grey = 0
      let lum = 0
      for (let p = 0; p < d.length; p += 4) {
        if (d[p + 3] < 96) continue
        seen++
        lum += 0.299 * d[p] + 0.587 * d[p + 1] + 0.114 * d[p + 2]
        if (Math.max(d[p], d[p + 1], d[p + 2]) - Math.min(d[p], d[p + 1], d[p + 2]) < 24) grey++
      }
      const mono = seen && grey / seen > 0.9 ? (lum / seen > 200 ? 'light' : lum / seen < 60 ? 'dark' : '') : ''
      if (monoOf.size > 300) monoOf.clear()
      monoOf.set(img.src, mono)
      mark(mono)
    }
    if (img.complete && img.naturalWidth) look()
    else img.addEventListener('load', look, { once: true })
  }

  // Starting, the tray kept as wide as it was last time until as many icons
  // are back (the programs add theirs one by one once the tray is ours,
  // seconds after the rest is drawn), or a few seconds: what is left of it
  // never jumps leftwards as the last ones come. Kept here, as it is, while
  // on the taskbar.
  const TRAY_KEPT = 'trayWidth'
  let trayHold = null
  try {
    const kept = JSON.parse(localStorage.getItem(TRAY_KEPT) || 'null')
    if (kept?.w > 0 && kept.w < 2000 && kept.n > 0) trayHold = kept
  } catch {}
  let trayKept = ''
  function holdTray() {
    if (trayHold && (trayIcons.length >= trayHold.n || performance.now() > 8000)) trayHold = null
    trayBox.style.minWidth = trayHold ? `${trayHold.w}px` : ''
    trayBox.style.justifyContent = trayHold ? 'flex-end' : ''
    if (trayHold) return
    requestAnimationFrame(() => {
      if (home() !== 'taskbar' || !trayIcons.length) return
      const w = Math.round(trayBox.getBoundingClientRect().width)
      const kept = JSON.stringify({ w, n: trayIcons.length })
      if (w <= 0 || kept === trayKept) return
      trayKept = kept
      try {
        localStorage.setItem(TRAY_KEPT, kept)
      } catch {}
    })
  }
  setTimeout(holdTray, 8100)

  function drawTray() {
    const icons = orderedTray()
    const folded = icons.filter(i => !isOut(i))
    if (!folded.length) trayOpen = false
    const drawn = JSON.stringify([icons.map(i => [i.key, i.png.length, i.png.slice(-24), i.tip, isOut(i)]), trayOpen])
    if (drawn !== trayDrawn) {
      trayDrawn = drawn
      chevron.hidden = !folded.length
      chevron.classList.toggle('open', trayOpen)
      chevron.setAttribute('aria-label', t(lang, 'taskbar.showHidden'))
      chevron.setAttribute('aria-expanded', String(trayOpen))
      outBox.replaceChildren(...icons.filter(isOut).map(trayIcon))
      flyout.replaceChildren(...folded.map(trayIcon))
      flyout.hidden = !trayOpen
      if (trayBox.firstChild !== flyout) trayBox.replaceChildren(flyout, chevron, outBox)
    }
    holdTray()
    requestAnimationFrame(sendTrayRects)
  }

  function openTray(open) {
    if (trayOpen === open) return
    trayOpen = open
    trayBox.classList.toggle('upper', isSide() && trayBox.getBoundingClientRect().top < innerHeight / 2)
    drawTray()
    watchPresses()
  }
  chevron.addEventListener('click', () => openTray(!trayOpen))
  // Folded away again once the pointer has been gone from it a moment.
  let trayCloseTimer
  trayBox.addEventListener('mouseleave', () => {
    clearTimeout(trayCloseTimer)
    trayCloseTimer = setTimeout(() => openTray(false), 1200)
  })
  trayBox.addEventListener('mouseenter', () => clearTimeout(trayCloseTimer))

  // Where each icon is, for the programs that ask (taskbar.rs): a folded
  // one, while folded, is where ^ is.
  function sendTrayRects() {
    if (home() !== 'taskbar') return
    const at = node => {
      const r = node.getBoundingClientRect()
      return [r.left, r.top, r.width, r.height]
    }
    const rects = trayIcons.map(i => {
      const shown = trayBox.querySelector(`[data-tray="${i.key}"]`)
      const visible = shown && (isOut(i) || trayOpen)
      return [Number(i.key), ...at(visible ? shown : chevron)]
    })
    const sent = JSON.stringify(rects)
    if (sent === trayRectsSent) return
    trayRectsSent = sent
    window.pet.taskbar.trayRects(rects)
  }
  window.addEventListener('resize', () => {
    trayRectsSent = ''
    requestAnimationFrame(sendTrayRects)
  })

  // The icons' pictures kept here: taskbar.rs (send_icons) sends each once,
  // and again only when it is another; a list without one's png means the
  // one kept.
  const trayPngs = new Map()
  window.pet.onTray(list => {
    trayIcons = (Array.isArray(list) ? list : []).map(i => {
      if (i.png === undefined) i.png = trayPngs.get(i.key) ?? ''
      trayPngs.set(i.key, i.png)
      return i
    })
    for (const key of [...trayPngs.keys()]) if (!trayIcons.some(i => i.key === key)) trayPngs.delete(key)
    if (isOn() && home() === 'taskbar') drawTray()
  })

  // A press on a tray icon goes to its program, as Windows' taskbar tells it:
  // the second press of a double one as a double press; the pointer coming,
  // moving (at most every 100 ms) and going. A left press is told once it
  // is let go, for it may be a drag instead: an icon dragged onto ^ or into
  // the folded ones is folded away, one dragged onto the taskbar kept out,
  // either put where it is let go among the others there.
  const trayKey = e => e.target.closest('[data-tray]')?.dataset.tray
  let trayOver = null
  let trayMovedAt = 0
  // A left press on an icon not yet let go: { key, name, x, y, ghost }.
  let trayPress = null
  trayBox.addEventListener('pointerdown', e => {
    if (e.button === 0 && trayKey(e)) {
      try {
        trayBox.setPointerCapture(e.pointerId)
      } catch {}
    }
  })
  trayBox.addEventListener('mousedown', e => {
    const key = trayKey(e)
    if (!key) return
    e.stopPropagation()
    if (e.button === 2) return window.pet.taskbar.tray(key, 'rightDown')
    if (e.button !== 0) return
    if (e.detail === 2) {
      trayPress = null
      return window.pet.taskbar.tray(key, 'double')
    }
    trayPress = { key, name: e.target.closest('[data-tray]').dataset.name, x: e.clientX, y: e.clientY, ghost: null }
  })
  trayBox.addEventListener('mouseup', e => {
    const key = trayKey(e)
    if (e.button === 2 && key) {
      e.stopPropagation()
      return window.pet.taskbar.tray(key, 'rightUp')
    }
    if (e.button !== 0 || !trayPress) return
    e.stopPropagation()
    const press = trayPress
    trayPress = null
    if (!press.ghost) {
      window.pet.taskbar.tray(press.key, 'leftDown')
      window.pet.taskbar.tray(press.key, 'leftUp')
      if (flyout.contains(e.target)) openTray(false)
      return
    }
    press.ghost.remove()
    body.classList.remove('tray-dragging')
    markDrop(null)
    const icon = trayIcons.find(i => String(i.key) === press.key)
    const drop = dropAt(e.clientX, e.clientY, press.name)
    if (!icon || !drop) return
    // Kept out or folded, and where among the others: before the icon
    // under the pointer (after it past its middle), else last there.
    const order = trayOrder()
    for (const i of orderedTray()) if (!order.includes(i.name)) order.push(i.name)
    order.splice(order.indexOf(press.name), 1)
    let at
    if (drop.target) {
      at = order.indexOf(drop.target) + (drop.after ? 1 : 0)
    } else {
      const there = orderedTray().filter(i => i.name !== press.name && isOut(i) === drop.out)
      at = there.length ? order.indexOf(there[there.length - 1].name) + 1 : order.length
    }
    order.splice(at, 0, press.name)
    window.pet.taskbar.trayPin(press.name, isOut(icon) === drop.out ? null : drop.out, order.slice(0, 200))
  })

  // Where a dragged icon would land at (x, y): kept out (anywhere on the
  // taskbar) or folded (the folded ones, ^), and the icon there it goes
  // beside (none: last); null, nowhere.
  function dropAt(x, y, name) {
    const over = document.elementFromPoint(x, y)
    if (!over) return null
    const out = !(flyout.contains(over) || chevron.contains(over))
    if (out && !barRest.contains(over)) return null
    const slot = over.closest('[data-tray]')
    // Let go on itself: left where it was.
    if (slot?.dataset.name === name) return null
    const mine = slot && (out ? outBox : flyout).contains(slot) ? slot : null
    if (!mine) return { out, target: null, after: false, slot: null }
    const r = mine.getBoundingClientRect()
    // The folded ones in their grid; those kept out along the bar.
    const a = out ? along(x, y) : x
    const [start, size] = out && isSide() ? [r.top, r.height] : [r.left, r.width]
    return { out, target: mine.dataset.name, after: a > start + size / 2, slot: mine }
  }

  // While dragging: a bar on the side of the icon it would land beside.
  let dropMarked = null
  function markDrop(drop) {
    if (dropMarked) dropMarked.classList.remove('drop-before', 'drop-after')
    dropMarked = drop?.slot || null
    if (dropMarked) dropMarked.classList.add(drop.after ? 'drop-after' : 'drop-before')
  }
  window.addEventListener('mousemove', e => {
    if (!trayPress) return
    if (!trayPress.ghost && Math.hypot(e.clientX - trayPress.x, e.clientY - trayPress.y) > 6) {
      const from = trayBox.querySelector(`[data-tray="${trayPress.key}"] img`)
      trayPress.ghost = el('img', 'tghost')
      if (from) trayPress.ghost.src = from.src
      body.append(trayPress.ghost)
      body.classList.add('tray-dragging')
      // Dragged from the taskbar: the folded ones open to take it.
      openTray(true)
    }
    if (trayPress.ghost) {
      Object.assign(trayPress.ghost.style, { left: `${e.clientX - 10}px`, top: `${e.clientY - 10}px` })
      markDrop(dropAt(e.clientX, e.clientY, trayPress.name))
    }
  })
  trayBox.addEventListener('mousemove', e => {
    const key = trayKey(e) || null
    if (key !== trayOver) {
      if (trayOver) window.pet.taskbar.tray(trayOver, 'out')
      if (key) window.pet.taskbar.tray(key, 'in')
      trayOver = key
    }
    if (key && Date.now() - trayMovedAt > 100) {
      trayMovedAt = Date.now()
      window.pet.taskbar.tray(key, 'move')
    }
  })
  trayBox.addEventListener('mouseleave', () => {
    if (trayOver) window.pet.taskbar.tray(trayOver, 'out')
    trayOver = null
  })

  // The taskbar's own buttons open what Windows' do: Start, and from the
  // clock the notifications and the calendar (once the button is up).
  barRest.addEventListener('click', e => {
    const what = e.target.closest('[data-bar]')?.dataset.bar
    if (what && what !== 'settings') window.pet.taskbar.open(what)
    if (what === 'ime' && imeOpen) closeImeList()
    // A widget put there: its card, or closed again.
    const widget = e.target.closest('.tw-widget, .tw-plugin')
    if (widget) wgFor === widget.dataset.entry ? closeWidgetPop() : openWidgetPop(widget.dataset.entry)
  })
  // A right press: a tray icon's program has it; on the input method, the
  // input methods to pick from; on a widget put there, its card; on the
  // volume, the network or the clock, the menu Windows' own taskbar has
  // there (zhoupengjie/Wakuwaku#36: tray.rs part_menu); elsewhere, her menu.
  barRest.addEventListener('contextmenu', e => {
    e.preventDefault()
    if (trayKey(e)) return
    if (wgPop.contains(e.target)) return
    const widget = e.target.closest('.tw-widget, .tw-plugin')
    if (widget) return openWidgetPop(widget.dataset.entry)
    if (e.target.closest('[data-bar="ime"]')) return imeOpen ? closeImeList() : openImeList()
    const mark = quickBox.contains(e.target) ? quickMarkAt(e.clientX, e.clientY) : null
    window.pet.menu(mark === volMark ? 'volume' : mark === netMark ? 'network' : clockBox.contains(e.target) ? 'clock' : null)
  })
  // Which of the quick settings' marks a press on its button is for: the
  // nearest along the strip, each having the button's height (its width
  // along a side), as Windows' own do. The battery's menu stays hers.
  function quickMarkAt(x, y) {
    const a = along(x, y)
    let nearest = null
    let gap = Infinity
    for (const mark of [netMark, volMark, batMark]) {
      if (mark.hidden) continue
      const r = mark.getBoundingClientRect()
      const d = a < lo(r) ? lo(r) - a : a > hi(r) ? a - hi(r) : 0
      if (d < gap) [nearest, gap] = [mark, d]
    }
    return nearest
  }
  // It takes the pointer while it is under it (the window lets clicks through elsewhere).
  barRest.addEventListener('mouseenter', () => window.pet.hover(true))
  barRest.addEventListener('mouseleave', () => window.pet.hover(false))

  // --- Editing the layout: the parts dragged about on the strip itself -------------------
  // As KDE edits a panel: each part outlined, the three runs shown (an empty
  // one a place to drop into), a part dragged to where a line shows it goes
  // and laid out there at once; a bar above the strip to add widgets, reset,
  // cancel or keep it (taskbarLayout). The widgets come from a library over
  // the strip, the island's other side (by kind, each a card): dragged onto
  // it, or a press puts one at the right run's start. One there is chosen by
  // a press (its size, or off the strip), or dragged off: onto the library,
  // or up away from the strip. The parts' own presses are held back
  // meanwhile; her island lets the pointer through to its slot, to be
  // dragged by it.
  const editBar = el('div', 'tbedit')
  const editMark = el('span', 'tbmark')
  const library = el('div', 'tblib')
  const tools = el('div', 'tbtools')
  let drag = null
  // The library: open, the kind shown, what it knows (settings_get: every
  // widget, on or off, and the plugins), the cards drawn; the widget chosen
  // on the strip.
  let libOpen = false
  let libKind = 'all'
  let libSnap = null
  let libCards = []
  let chosen = null
  // Let go this far above the strip, a widget goes.
  const OFF_STRIP_PX = 90
  // The spacers' marks in the library (a widget's own are widgets.js ICONS).
  const SPACER_MARK = {
    flex: '<path d="M4 12h16M7 9l-3 3 3 3M17 9l3 3-3 3"/>',
    gap: '<path d="M5 7v10M19 7v10M8 12h8"/>',
    sep: '<path d="M12 4v16"/>',
  }

  // A new widget's id, as settings.rs takes it: "w" and up to 15 of a-z 0-9.
  const newId = () => `w${Date.now().toString(36).slice(-6)}${Math.random().toString(36).slice(2, 6)}`

  // Pointer in or out of what editing puts over the strip: the window takes it.
  for (const node of [editBar, library, tools]) {
    node.addEventListener('mouseenter', () => window.pet.hover(true))
    node.addEventListener('mouseleave', () => window.pet.hover(false))
  }

  function editTaskbar() {
    if (home() !== 'taskbar' || editLayout) return
    if (isSetting()) window.Settings?.close?.('edit')
    if (wgFor) closeWidgetPop()
    if (popFor) closePop()
    editLayout = JSON.parse(JSON.stringify(tbLayout()))
    // Each part's name: shown on one with nothing in it now, and dragged by.
    for (const [part, node] of Object.entries(PART_NODES)) node.dataset.label = t(lang, `taskbar.part.${part}`)
    // Closed, should it have been open under the pointer (its menu came
    // from it): it lets the pointer through now, and would hear no leaving.
    clearTimeout(hoverTimer)
    isHover = false
    body.classList.add('tb-editing')
    libOpen = false
    chosen = null
    drawEditBar()
    document.body.append(editBar)
    arranged = ''
    update()
  }

  function drawEditBar() {
    const button = (what, label = t(lang, `taskbar.edit.${what}`)) => {
      const b = el('button', `tbe tbe-${what}`, label)
      b.dataset.edit = what
      return b
    }
    editBar.replaceChildren(el('span', 'hint', t(lang, isSide() ? 'taskbar.edit.hintSide' : 'taskbar.edit.hint')), button('add', t(lang, libOpen ? 'taskbar.edit.hideAdd' : 'taskbar.edit.add')), button('reset'), button('cancel'), button('done'))
  }

  function endEdit(keep) {
    if (!editLayout) return
    const layout = editLayout
    editLayout = null
    endDrag()
    body.classList.remove('tb-editing')
    editBar.remove()
    library.remove()
    tools.remove()
    libOpen = false
    chosen = null
    for (const node of entryNodes.values()) node.classList.remove('tb-chosen')
    if (keep) {
      config.taskbarLayout = layout
      const patch = { taskbarLayout: layout }
      // A plugin put there that was not running: it runs now.
      const plugins = config.plugins && typeof config.plugins === 'object' ? config.plugins : {}
      const off = [...new Set(Object.values(layout).flatMap(run => run.filter(p => p.type === 'plugin').map(p => p.of)))].filter(id => plugins[id]?.on !== true)
      if (off.length) patch.plugins = { ...plugins, ...Object.fromEntries(off.map(id => [id, { ...(plugins[id] || {}), on: true }])) }
      // Then which of the plugins put there want their settings first.
      Promise.resolve(window.pet.settings.set(patch)).then(() => askPlugins(true))
    }
    arranged = ''
    update()
  }

  editBar.addEventListener('click', e => {
    const what = e.target.closest('[data-edit]')?.dataset.edit
    if (what === 'done') endEdit(true)
    else if (what === 'cancel') endEdit(false)
    else if (what === 'add') showLibrary(!libOpen)
    else if (what === 'reset') {
      // The parts as Windows 11 has them; the widgets kept, where they were.
      editLayout = tbLayout(Widgets.withParts(TB_PRESETS.win11, tbLayout()))
      arranged = ''
      update()
    }
  })

  // --- The library -------------------------------------------------------------------
  function showLibrary(open) {
    libOpen = open
    drawEditBar()
    if (!open) return library.remove()
    drawLibrary()
    document.body.append(library)
    window.pet.settings.get().then(got => {
      libSnap = got
      tellPlugins(got?.plugins)
      if (libOpen) drawLibrary()
    })
  }

  // Its cards: the built-in widgets (not the monitor, a part of its own),
  // the plugins she runs, those of anyone else's
  // scripts there now, and the spacers.
  function cardsOf(all) {
    const cards = []
    const named = w => (w.private ? {} : { name: [...String(Widgets.words(lang, w).label)].slice(0, 40).join('') })
    for (const w of all.filter(w => w.builtIn && w.id !== 'monitor')) cards.push({ kind: 'builtIn', entry: { type: 'widget', of: w.id, icon: w.icon }, w })
    for (const p of libSnap?.plugins || []) cards.push({ kind: 'plugin', entry: { type: 'plugin', of: p.id, icon: p.icon }, w: all.find(w => w.plugin === p.id), plugin: p })
    for (const w of all.filter(w => w.from === 'script' && !w.plugin)) cards.push({ kind: 'script', entry: { type: 'widget', of: w.id, icon: w.icon, ...named(w) }, w })
    for (const type of ['flex', 'gap', 'sep']) cards.push({ kind: 'layout', entry: { type } })
    return cards
  }

  function drawLibrary() {
    const all = libSnap?.widgets || allWidgets
    const detailed = isDetailed()
    const placed = new Set(Object.values(tbLayout()).flatMap(run => run.filter(p => typeof p !== 'string' && Widgets.isSource(p)).map(p => `${p.type}:${p.of}`)))
    libCards = cardsOf(all)
    const kinds = ['all', ...new Set(libCards.map(c => c.kind))]
    if (!kinds.includes(libKind)) libKind = 'all'
    const tabs = el('div', 'tblkinds')
    for (const kind of kinds) {
      const b = el('button', kind === libKind ? 'tblkind on' : 'tblkind', t(lang, `taskbar.lib.${kind}`))
      b.dataset.kind = kind
      tabs.append(b)
    }
    const grid = el('div', 'tblgrid')
    for (const [i, c] of libCards.entries()) {
      if (libKind !== 'all' && c.kind !== libKind) continue
      const isOn = Widgets.isSource(c.entry) && placed.has(`${c.entry.type}:${c.entry.of}`)
      const card = el('div', isOn ? 'tblcard placed' : 'tblcard')
      card.dataset.card = i
      const mark = el('span', 'tblmark')
      mark.innerHTML = SPACER_MARK[c.entry.type] ? `<svg class="wicon" viewBox="0 0 24 24" aria-hidden="true">${SPACER_MARK[c.entry.type]}</svg>` : Widgets.icon(c.w?.icon || c.entry.icon || 'dot')
      mark.style.color = (c.w && stripInk(c.w.color)) || stripHue().idle
      const said = c.w ? Widgets.words(lang, c.w, detailed).value : ''
      const note = isOn
        ? t(lang, 'taskbar.lib.placed')
        : c.plugin?.missing
          ? t(lang, 'taskbar.lib.needs')
          : c.plugin && !c.plugin.on
            ? t(lang, 'taskbar.lib.starts')
            : said || t(lang, `taskbar.lib.note.${c.kind === 'layout' ? c.entry.type : c.kind}`)
      card.append(mark, el('b', '', Widgets.nameOf(lang, { ...c.entry, id: 'w0' }, all, detailed)), el('span', 'tblnote', note))
      grid.append(card)
    }
    const head = el('div', 'tblhead')
    head.append(el('b', '', t(lang, 'taskbar.lib.title')), el('span', 'tblhint', t(lang, 'taskbar.lib.hint')))
    library.replaceChildren(head, tabs, grid)
  }

  library.addEventListener('click', e => {
    const kind = e.target.closest('[data-kind]')?.dataset.kind
    if (!kind) return
    libKind = kind
    drawLibrary()
  })

  // A card picked up: a new widget, dragged onto the strip (or put at the
  // right run's start by a press). One already there is not again.
  library.addEventListener('pointerdown', e => {
    const card = e.target.closest('[data-card]')
    if (!card || e.button !== 0 || card.classList.contains('placed')) return
    const c = libCards[card.dataset.card]
    if (!c) return
    e.preventDefault()
    const fresh = { ...c.entry, id: newId(), ...(Widgets.isSource(c.entry) ? { size: 'M' } : {}) }
    drag = { fresh, node: card, label: card.querySelector('b')?.textContent || '', x: e.clientX, y: e.clientY, moved: false }
    try {
      library.setPointerCapture(e.pointerId)
    } catch {}
  })

  // --- The one chosen: its size, or off ------------------------------------------------
  function choose(id) {
    chosen = id
    drawTools()
  }

  function drawTools() {
    const node = chosen && entryNodes.get(chosen)
    const e = node?._entry
    for (const n of entryNodes.values()) n.classList.toggle('tb-chosen', n === node)
    if (!editLayout || !e || !node.isConnected) {
      chosen = null
      return tools.remove()
    }
    const button = (what, label, on) => {
      const b = el('button', on ? 'tbt on' : 'tbt', label)
      b.type = 'button'
      b.dataset.tool = what
      return b
    }
    const items = [el('span', 'tbtname', node.dataset.label || '')]
    if (Widgets.isSource(e)) items.push(button('S', t(lang, 'taskbar.w.size.S'), node.dataset.size === 'S'), button('M', t(lang, 'taskbar.w.size.M'), node.dataset.size === 'M'), el('span', 'tbtsep'))
    items.push(button('remove', t(lang, 'taskbar.w.remove')))
    tools.replaceChildren(...items)
    if (!tools.isConnected) document.body.append(tools)
    const r = node.getBoundingClientRect()
    // Over it, or beside a side bar (its place there by style.css).
    tools.style.left = isSide() ? '' : `${Math.round(Math.max(4, Math.min(r.left + r.width / 2 - tools.offsetWidth / 2, innerWidth - tools.offsetWidth - 4)))}px`
    tools.style.top = isSide() ? `${Math.round(Math.max(8, Math.min(r.top + r.height / 2 - tools.offsetHeight / 2, innerHeight - tools.offsetHeight - 8)))}px` : ''
  }

  tools.addEventListener('click', e => {
    const what = e.target.closest('[data-tool]')?.dataset.tool
    const id = chosen
    if (!what || !id) return
    if (what === 'remove') {
      choose(null)
      return removeEntry(id)
    }
    const layout = tbLayout()
    for (const run of Object.values(layout)) for (const p of run) if (p.id === id) p.size = what
    editLayout = layout
    arranged = ''
    update()
  })

  // Where a part let go at x goes: the run under it (or the nearest), and
  // before the first part of it whose middle is past x; the line's place.
  // (Named dropAt at first, it took the name of the tray's: an icon dragged
  // in the tray was dropped as a part, 2026-10-12.)
  function partDropAt(x) {
    let near = null
    for (const [zone, node] of Object.entries(zones)) {
      const r = node.getBoundingClientRect()
      const d = x < lo(r) ? lo(r) - x : x > hi(r) ? x - hi(r) : 0
      if (!near || d < near.d) near = { zone, d, r }
    }
    const parts = [...zones[near.zone].children].filter(n => n !== drag.node)
    let index = parts.findIndex(n => {
      const r = n.getBoundingClientRect()
      return x < lo(r) + len(r) / 2
    })
    if (index < 0) index = parts.length
    const line = index < parts.length ? lo(parts[index].getBoundingClientRect()) : parts.length ? hi(parts[parts.length - 1].getBoundingClientRect()) : lo(near.r) + len(near.r) / 2
    return { zone: near.zone, index, line }
  }

  // A part (by its name) or a widget (by its id) moved; a new widget put in.
  function movePart(key, zone, index, fresh) {
    const layout = tbLayout()
    let item = fresh || key
    for (const k of Object.keys(layout))
      layout[k] = layout[k].filter(p => {
        if (Widgets.keyOf(p) !== key) return true
        item = p
        return false
      })
    layout[zone].splice(index, 0, item)
    editLayout = layout
    arranged = ''
    update()
    if (libOpen) drawLibrary()
  }

  function endDrag() {
    if (!drag) return
    drag.ghost?.remove()
    drag.node.classList.remove('tb-dragging')
    editMark.remove()
    drag = null
  }

  // Over the library (or let go there).
  const overLibrary = (x, y) => {
    if (!library.isConnected) return false
    const r = library.getBoundingClientRect()
    return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
  }

  // While editing, the parts' own presses (Start, the buttons, the tray, the
  // menu) are held back: they are being moved, not used.
  for (const type of ['click', 'auxclick', 'dblclick', 'contextmenu', 'pointerup']) {
    barRest.addEventListener(
      type,
      e => {
        if (!editLayout) return
        if (type !== 'pointerup') e.preventDefault()
        e.stopPropagation()
      },
      true,
    )
  }
  barRest.addEventListener(
    'pointerdown',
    e => {
      if (!editLayout) return
      e.preventDefault()
      e.stopPropagation()
      const node = e.target.closest('[data-part]')
      if (e.button !== 0) return
      if (!node) return choose(null)
      drag = { key: node.dataset.part, entry: node._entry || null, node, label: node.dataset.label || '', x: e.clientX, y: e.clientY, moved: false }
      try {
        barRest.setPointerCapture(e.pointerId)
      } catch {}
    },
    true,
  )

  // A drag, from the strip or from the library: where it goes shown by a
  // line in the run; a widget dragged off the strip (onto the library, or
  // up away from it) goes, a new one there is not put in.
  document.addEventListener(
    'pointermove',
    e => {
      if (!drag) return
      if (!drag.moved) {
        if (Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 5) return
        drag.moved = true
        drag.ghost = el('span', 'tbghost', drag.label)
        document.body.append(drag.ghost, editMark)
        drag.node.classList.add('tb-dragging')
        choose(null)
      }
      drag.ghost.style.left = `${e.clientX}px`
      drag.ghost.style.top = `${e.clientY}px`
      const strip = barRest.getBoundingClientRect()
      const away = { bottom: e.clientY < strip.top - OFF_STRIP_PX, top: e.clientY > strip.bottom + OFF_STRIP_PX, left: e.clientX > strip.right + OFF_STRIP_PX, right: e.clientX < strip.left - OFF_STRIP_PX }[tbEdge()]
      const off = (!!drag.entry || !!drag.fresh) && (overLibrary(e.clientX, e.clientY) || away)
      drag.off = off
      drag.ghost.classList.toggle('off', off)
      drag.ghost.textContent = off && drag.entry ? t(lang, 'taskbar.w.drop', { name: drag.label }) : drag.label
      editMark.hidden = off
      drag.at = off ? null : partDropAt(along(e.clientX, e.clientY))
      if (drag.at) {
        editMark.style.left = isSide() ? '' : `${Math.round(drag.at.line) - 1}px`
        editMark.style.top = isSide() ? `${Math.round(drag.at.line) - 1}px` : ''
      }
    },
    true,
  )
  document.addEventListener(
    'pointerup',
    e => {
      if (!drag || e.button !== 0) return
      const d = drag
      endDrag()
      if (!d.moved) {
        // A press: a card goes to the right run's start; a widget on the
        // strip is chosen (again: no longer); elsewhere, none is.
        if (d.fresh) return movePart(d.fresh.id, 'right', 0, d.fresh)
        return choose(d.entry && chosen !== d.entry.id ? d.entry.id : null)
      }
      if (d.off) return d.entry ? removeEntry(d.entry.id) : undefined
      if (d.at) movePart(d.fresh ? d.fresh.id : d.key, d.at.zone, d.at.index, d.fresh)
    },
    true,
  )
  document.addEventListener('pointercancel', endDrag, true)

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
    // Closing on the taskbar, the blur under it goes first, then it shrinks.
    const late = plateSent && home() === 'taskbar' && view === 'compact' && was !== 'compact' && !stillMotion() ? PLATE_OUT_MS : 0
    holdGround(was, late)
    // The deeper ground at once as it opens; changing, over a moment.
    setDeep(was === view || view === 'compact')
    setInk()
    island.style.setProperty('--ring', hue()[now.mood])
    island.classList.toggle('wants', now.mood === 'waiting')
    seatPanel()
    body.classList.toggle('has-her', isHome())
    for (const e of ['top', 'left', 'right']) body.classList.toggle(`tb-${e}`, tbEdge() === e)
    body.classList.toggle('hinting', isHinting() && isHome())
    document.documentElement.style.setProperty('--hint-accent', hintAccent())
    body.classList.toggle('tb-side', isSide())
    body.classList.toggle('compact-her', view === 'compact' || view === 'settings')
    if (spriteUrl) sheetImage(herSheet, spriteUrl)
    fillCompact()
    fillExpanded()
    fillBar()

    const want = sizeOf(view)
    if (view === 'settings' && isSide()) placeSideSettings(want.height)
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
    else if (late) {
      hidePlate(PLATE_OUT_MS)
      setTimeout(grow, late)
    } else grow()

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
  // drop below the island hangs from its lower edge, and one above the
  // taskbar stands on its upper edge.
  function edgeNear(r, x, y) {
    const [ix, iy] = isSide() ? [14, 24] : [24, 14]
    return [Math.min(Math.max(x, r.left + ix), r.right - ix), Math.min(Math.max(y, r.top + iy), r.bottom - iy)]
  }

  // Her seat in her home, in the page: where she springs back to.
  function seatIn(r) {
    if (home() === 'taskbar' && isSide()) return [r.left + r.width / 2, r.top + r.width / 2]
    if (home() === 'taskbar') return [r.left + r.height / 2, r.top + r.height / 2]
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
    // From the top of the screen (her island, a top bar) she can only come
    // down, from a bottom bar only up, from a side bar only into the screen.
    const edge = home() === 'taskbar' ? tbEdge() : 'top'
    const cx = edge === 'left' ? Math.max(x, r.right - 10) : edge === 'right' ? Math.min(x, r.left + 10) : x
    const cy = edge === 'bottom' ? Math.min(y + 30, r.top + 10) : edge === 'top' ? Math.max(y + 30, r.bottom - 10) : y
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
    body.classList.remove('home-island', 'home-taskbar')
    if (isOn()) body.classList.add(`home-${home()}`)
    drawMica()
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
  // of seconds, main.rs redraw_widgets): on the strip, the readings' node
  // alone; the island drawn anew where it shows them in its lines, or when
  // another widget's words changed (the capsule shows them in turn).
  window.pet.onWidgets(({ widgets: all }) => {
    const others = JSON.stringify(widgets)
    takeWidgets(all)
    if (!isOn()) return
    if (home() === 'taskbar' && view === 'compact' && JSON.stringify(widgets) === others) {
      facesBox.replaceChildren(...widgetFaces())
      drawEntries()
    } else update()
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

  // Risen over an app full screen for the prompts alone: her island and its
  // panel, not the strip under them (style.css over-full).
  window.pet.onOverFull(({ on } = {}) => body.classList.toggle('over-full', on === true))

  // panel.js tells the island when its prompt changes.
  // The most the settings may be high: above the taskbar, its room (the
  // window never moves for them); elsewhere, none said (the screen's).
  const settingsMax = () => (home() === 'taskbar' ? (isSide() ? innerHeight - 2 * TASKBAR_GAP - TASKBAR_SHADE : TASKBAR_ROOM - TASKBAR_GAP - TASKBAR_SHADE) : null)

  // The taskbar's layout, for the settings: its presets, which one it is, editing it on the strip.
  window.pet.onTaskbarEdit(() => editTaskbar())
  window.Island = { accent: () => look.accent || null, isOn, changed: update, reach, absorb, settingsMax, tbPresets: TB_PRESETS, tbPreset: () => presetOf(config.taskbarLayout), tbWithPreset: name => TB_PRESETS[name] && Widgets.withParts(TB_PRESETS[name], tbLayout(config.taskbarLayout)), editTaskbar }
})()
