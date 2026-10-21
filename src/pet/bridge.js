// window.pet: what the page asks of the Rust side and hears from it, over
// Tauri's IPC, in her window (role=pet) and the island's (role=island).
// Loaded before the page's own scripts. The Rust side knows which window a
// call comes from.
//
// A window that lets clicks through hears no mouse moves. So while the
// pointer is near and the window lets clicks through, the Rust side sends
// where it is (pet:pointer), and this plays the mouseenter / mouseleave the
// page would have heard.
;(function () {
  const { invoke } = window.__TAURI__.core
  const { listen } = window.__TAURI__.event

  // Listeners register over IPC; the Rust side starts talking only once they
  // all have (pet_ready). Each listens for this window alone: listen() by
  // itself hears what is sent to any window (Tauri 2), so her page heard the
  // island's pet:pointer and played it as the pointer over her, at the
  // island's coordinates, and the island heard hers; the windows' mouse
  // modes bounced caught/through 30 times a second (2026-10-11), and every
  // event was handled twice.
  const LABEL = window.__TAURI_INTERNALS__?.metadata?.currentWebview?.label || window.__TAURI__.webviewWindow?.getCurrentWebviewWindow?.()?.label || new URLSearchParams(location.search).get('role') || 'pet'
  const target = { kind: 'WebviewWindow', label: LABEL }
  const pending = []
  const on = channel => fn => {
    pending.push(listen(channel, e => fn(e.payload), { target }))
  }
  const noop = () => {}

  // One at a time, in order: each call is its own request, and a drop must
  // not overtake the release before it.
  let queue = Promise.resolve()
  const send = (cmd, args) => {
    queue = queue.then(() => invoke(cmd, args)).catch(err => console.error(cmd, err))
  }

  window.pet = {
    // On a Mac (WebKit says MacIntel, on Apple silicon too): what is done
    // for its notch and its menus only there, so Windows and macOS never
    // change each other.
    mac: /^Mac/.test(navigator.platform || ''),
    onUpdate: on('pet:update'),
    // Only the widgets' words changed (the monitor's readings): { widgets }.
    onWidgets: on('pet:widgets'),
    // Nobody at the keyboard or mouse for a while, or someone again: { on }.
    onQuiet: on('pet:quiet'),
    onReact: on('pet:react'),
    // A widget asks to open the island for itself (widgets.rs).
    onNudge: on('pet:nudge'),
    onAlert: on('pet:alert'),
    onCursor: on('pet:cursor'),
    onDrag: on('pet:drag'),
    onDragEnd: on('pet:drag-end'),
    onAsks: on('pet:asks'),
    onShift: on('pet:shift'),
    // The side island's rows (dock.rs payload), in its own window.
    onDock: on('dock:update'),
    // The game's ten (scout.rs), in their panel's window.
    onScout: on('scout:update'),
    scoutClose: () => send('scout_close'),
    // Master Duel's helper (duel.rs), in its panel's window.
    onDuel: on('duel:update'),
    duelClose: () => send('duel_close'),
    duelAgain: () => send('duel_again'),
    // The folder the phone's files go to (mobile.rs drop).
    openDrops: () => send('open_drops'),
    // A game kept, whole (its scoreboard), by its id; none if not kept.
    scoutGame: id => invoke('scout_game', { id: String(id || '') }).catch(() => null),
    // Her rows of these games kept (a champion's, a position's), as who in them.
    // A game's timeline (each minute's gold of the ten, kills, items, skills).
    scoutTimeline: id => invoke('scout_timeline', { id: String(id || '') }).catch(() => null),
    // A game reviewed by the agents asked (once each), and its reviews as they are.
    scoutReviewStart: (id, who, agents) => invoke('scout_review_start', { id: String(id || ''), who: String(who || ''), agents: (agents || []).map(String) }).catch(() => null),
    scoutReview: id => invoke('scout_review', { id: String(id || '') }).catch(() => null),
    // A champion's build and matchup from OP.GG, through Claude (kept a day).
    scoutOpgg: (champion, position, opponent) => invoke('scout_opgg', { champion: Number(champion) || 0, position: String(position || ''), opponent: Number(opponent) || null }).catch(() => null),
    // Champion select: a position's strongest champions; her own against an opponent (OP.GG, through Claude).
    scoutOpggMeta: position => invoke('scout_opgg_meta', { position: String(position || '') }).catch(() => null),
    scoutOpggPicks: (opponent, position, candidates) => invoke('scout_opgg_picks', { opponent: Number(opponent) || 0, position: String(position || ''), candidates: (candidates || []).map(Number) }).catch(() => null),
    scoutRows: (ids, who) => invoke('scout_rows', { ids: (ids || []).map(String), who: String(who || '') }).catch(() => []),
    answer: (id, choice) => send('pet_answer', { id, choice }),
    dismiss: id => send('pet_dismiss', { id }),
    keyboard: needs => send('pet_keyboard', { needs: needs === true }),
    panel: measured => send('pet_panel', { measured: measured || null }),
    hover: isOver => {
      // The real pointer has the page now, or has left it: start over.
      inside = []
      send('pet_hover', { isOver: isOver === true })
    },
    dragStart: () => send('pet_drag_start'),
    dragEnd: () => send('pet_drag_end'),
    // Her menu.
    menu: () => send('pet_menu'),
    doubleClick: () => send('pet_double_click'),
    // Out of the queue: it answers when the walk is over.
    walk: (dx, ms) => invoke('pet_walk', { dx, ms }).catch(() => false),
    walkStop: () => send('pet_walk_stop'),
    // The island's page: she broke free of the drop (her middle this far from
    // the cursor), the button was let go, the button is held on her.
    releaseHer: offset => send('island_release', { offset: offset || {} }),
    dropHer: () => send('island_drop'),
    holding: isHolding => send('island_holding', { isHolding: isHolding === true }),
    // A blur of what is behind her island, under it (backplate.rs): where
    // the island is ({ x, y, width, height, radius, vw, vh, dark }), faded
    // in over fade ms; none, faded out.
    plate: (rect, fade) => send('island_plate', { rect: rect || null, fade: Math.max(0, Math.round(Number(fade) || 0)) }),
    onReach: on('island:reach'),
    onAbsorb: on('island:absorb'),
    onLanded: on('island:landed'),
    // A maximized window on her display, at the top of the screen ({ on }, island.rs look_over_max).
    onOverMax: on('island:over-max'),
    // The settings in the island (settings.js): open them here or from
    // anywhere else (the tray, her double-click), what they show, what they change.
    onSettings: on('island:settings'),
    onSettingsChanged: on('settings:changed'),
    // The island's window lost the keyboard (a click elsewhere).
    onBlur: on('island:blur'),
    openSettings: tab => send('settings_open', { tab: tab || null }),
    settingsClosed: () => send('settings_closed'),
    // To a session's window; true when there was one to go to.
    jump: id => invoke('session_jump', { id: String(id || '') }).catch(() => false),
    // A Mac's screen top, the island over its menu bar (notch.rs):
    // { height, notch } in px, the notch's width 0 without one.
    onTop: on('island:top'),
    // Her tray icon clicked on a Mac: the island in the notch drops open, or
    // goes back (island.js peek).
    onPeek: on('island:peek'),
    // A file dragged from another program over the window: (type, at) with
    // type enter, over, leave or drop, at where it is in the page's px (none
    // once it has left). Tauri's drop handler has the window's drags, so the
    // page hears no dragenter of its own.
    onFileDrag: fn => fileDrags.push(fn),
    settings: {
      get: () => invoke('settings_get'),
      set: patch => invoke('settings_set', { patch }),
      showPet: on => invoke('settings_show_pet', { on: on === true }),
      login: on => invoke('settings_login', { on: on === true }),
      hooks: action => invoke('settings_hooks', { action }),
      codexHooks: action => invoke('settings_codex_hooks', { action }),
      agyHooks: action => invoke('settings_agy_hooks', { action }),
      grokHooks: action => invoke('settings_grok_hooks', { action }),
      fetch: reference => invoke('settings_fetch', { reference: String(reference || '') }),
      gallery: (page, sort, query) => invoke('settings_gallery', { page, sort, query }),
      openSite: place => invoke('settings_open_site', { place }),
      // Her updates: 'check', 'page' (the release's), 'update' (updates.rs).
      update: what => invoke('settings_update', { what }),
      // Her crash reports: 'yes', 'no' (her asking answered), 'send', 'open' (the last one's issue; crash.rs).
      crash: what => invoke('settings_crash', { what }),
      // The phone page: 'token' and 'topic' made anew, 'test' a push to try ntfy (mobile.rs).
      mobile: what => invoke('settings_mobile', { what }),
    },
  }

  // --- Mouse moves while clicks pass through ------------------------------------

  // The elements under the pointer, outermost first, as last played.
  let inside = []

  function chainAt(x, y) {
    const chain = []
    for (let el = document.elementFromPoint(x, y); el && el !== document.documentElement; el = el.parentElement) chain.unshift(el)
    return chain
  }

  function play(type, el, x, y) {
    el.dispatchEvent(new MouseEvent(type, { clientX: x, clientY: y, bubbles: false, view: window }))
  }

  pending.push(
    listen(
      'pet:pointer',
      ({ payload: at }) => {
        const next = at ? chainAt(at.x, at.y) : []
        const was = inside
        inside = next
        for (const el of was) if (!next.includes(el)) play('mouseleave', el, at?.x ?? -1, at?.y ?? -1)
        for (const el of next) if (!was.includes(el)) play('mouseenter', el, at.x, at.y)
      },
      { target },
    ),
  )

  // --- A file dragged over the window ---------------------------------------------

  // While a drag of files from another program is over the window (Tauri's
  // drop handler has it: tauri://drag-*, where it is in physical px), the
  // drag holds the mouse and the page hears none. Where it is is played as
  // the pointer's, in a chain of its own (hover() clears the pointer's), so
  // that the window lets the drag through once it is off her things, as it
  // does the pointer: caught, the clear window over the strip took the drop
  // meant for the window under it. Then told the page (onFileDrag).
  const fileDrags = []
  let dragged = []
  for (const type of ['enter', 'over', 'leave', 'drop']) {
    pending.push(
      listen(
        `tauri://drag-${type}`,
        ({ payload }) => {
          const p = type === 'leave' ? null : payload?.position
          const at = p ? { x: p.x / devicePixelRatio, y: p.y / devicePixelRatio } : null
          if (at && type !== 'drop') {
            const next = chainAt(at.x, at.y)
            const was = dragged
            dragged = next
            for (const el of was) if (!next.includes(el)) play('mouseleave', el, at.x, at.y)
            for (const el of next) if (!was.includes(el)) play('mouseenter', el, at.x, at.y)
          } else {
            // Gone, or let go of: the mouse is the page's again.
            dragged = []
          }
          for (const fn of fileDrags) fn(type, at)
        },
        { target },
      ),
    )
  }

  // The page's console.log into debug.log (when it is on).
  const log = console.log.bind(console)
  console.log = (...args) => {
    log(...args)
    invoke('pet_log', { line: args.map(String).join(' ') }).catch(noop)
  }

  // Debug only (/debug/eval): run code, give back what it returns.
  window.__wkEval = (id, fn) => {
    Promise.resolve()
      .then(fn)
      .catch(err => 'error: ' + err.message)
      .then(value => invoke('debug_result', { id, value: value === undefined ? null : value }))
  }

  // Errors too, so a page that never comes up says why.
  window.addEventListener('error', e => console.log('error:', e.message, e.filename, e.lineno))
  window.addEventListener('unhandledrejection', e => console.log('unhandled:', e.reason))

  document.addEventListener('DOMContentLoaded', () => {
    Promise.all(pending)
      .then(() => send('pet_ready'))
      .catch(err => console.log('listeners failed:', err))
  })
})()
