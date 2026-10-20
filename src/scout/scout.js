// The game's ten (scout.rs), or her own games where they cannot be found:
// a war room the four agents' pets keep. Her own: Claude tells her ranks,
// Grok her tags and the champions she plays most, Antigravity where she
// plays, Codex her last games; each part in its agent's colour, its pet
// beside a line of what it found. A game clicked opens its scoreboard; a
// player of the ten clicked, their last games. Its close button or its key
// puts it away.
;(function () {
  const { t } = window.I18n
  const { CELL_W, CELL_H, CLIPS } = window.Sprite
  let data = { state: 'none' }
  const T = (key, vars) => t(data.lang || 'zh-TW', key, vars)
  const $ = id => document.getElementById(id)

  function el(tag, cls, ...kids) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    for (const kid of kids.flat()) if (kid != null && kid !== false && kid !== '') node.append(kid)
    return node
  }

  // An element drawn with a picture (none: left blank).
  function pic(tag, cls, url) {
    const node = el(tag, cls)
    if (url) node.style.backgroundImage = `url("${url}")`
    return node
  }

  const pct = (w, n) => (n ? Math.round((w / n) * 100) : 0)

  // --- Where the pictures are ---------------------------------------------------------

  const CDRAGON = 'https://raw.communitydragon.org/latest/plugins'
  const DDRAGON = 'https://ddragon.leagueoflegends.com/cdn'
  const champIcon = id => (id > 0 ? `${CDRAGON}/rcp-be-lol-game-data/global/default/v1/champion-icons/${id}.png` : '')
  const profileIcon = id => (id != null ? `${CDRAGON}/rcp-be-lol-game-data/global/default/v1/profile-icons/${id}.jpg` : '')
  const emblem = tier => (tier ? `${CDRAGON}/rcp-fe-lol-static-assets/global/default/images/ranked-emblem/emblem-${String(tier).toLowerCase()}.png` : '')
  const item = id => (id > 0 && data.dd ? `${DDRAGON}/${data.dd.v}/img/item/${id}.png` : '')
  const spell = id => (data.dd && data.dd.spells[id] ? `${DDRAGON}/${data.dd.v}/img/spell/${data.dd.spells[id]}.png` : '')
  const perk = id => (data.dd && data.dd.perks[id] ? `${DDRAGON}/img/${data.dd.perks[id]}` : '')
  // Champions' names in her language (Data Dragon's), else as the client names them.
  const champName = id => (data.dd && data.dd.champions && data.dd.champions[id]) || (data.names || {})[id] || ''

  // 123456 points: 12.3萬, or 123.5k.
  function points(n) {
    if (!n) return '0'
    if ((data.lang || '').startsWith('zh')) return n >= 10000 ? `${(n / 10000).toFixed(n >= 1e6 ? 0 : 1)}${data.lang === 'zh' ? '万' : '萬'}` : String(n)
    return n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n)
  }

  const isApex = tier => ['MASTER', 'GRANDMASTER', 'CHALLENGER'].includes(tier)
  const tierText = r => `${T(`lol.tier.${r.tier}`)}${isApex(r.tier) ? '' : ` ${r.rank}`}`

  function ago(ms) {
    const m = Math.max(1, Math.round((Date.now() - ms) / 60000))
    if (m < 60) return T('scout.ago.m', { n: m })
    if (m < 60 * 24) return T('scout.ago.h', { n: Math.round(m / 60) })
    return T('scout.ago.d', { n: Math.round(m / 1440) })
  }

  const clock = s => `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, '0')}`
  const queueName = q => {
    const key = `scout.queue.${q}`
    const name = T(key)
    return name === key ? T('scout.queue.other') : name
  }

  // --- The agents' pets ------------------------------------------------------------------

  // Each head's film, its idle row played (a quarter of the sheet's size).
  const films = []
  let frame = 0
  setInterval(() => {
    frame++
    const clip = CLIPS.idle
    const at = Math.floor(frame / Math.round(clip.ms / 100)) % clip.frames
    for (const film of films) film.style.transform = `translate(${-at * (CELL_W / 4)}px, ${-clip.row * (CELL_H / 4)}px)`
  }, 100)

  function head(agent) {
    const pet = (data.pets || {})[agent] || {}
    const box = el('div', 'head')
    if (pet.sprite) {
      const film = pic('div', 'film', pet.sprite)
      film.style.setProperty('--sheet-h', `${pet.version === 1 ? 1872 : 2288}px`)
      box.append(film)
      films.push(film)
    } else {
      box.append(el('div', 'letter', agent[0].toUpperCase()))
    }
    return box
  }

  const AGENT_COLOR = { claude: '#E8732A', codex: '#8B7CD8', agy: '#4A8FE7', grok: '#D9A441' }

  // A part of the war room: its agent's colour, its pet, what it says, and the rest.
  // agent may name its part too: 'agy:tl' is Antigravity on the timeline.
  function card(who, said, cls, ...body) {
    const [agent, part] = who.split(':')
    const pet = (data.pets || {})[agent] || {}
    const node = el('section', `card ${cls || ''}`, el('div', 'host', head(agent), el('div', 'speech', el('b', null, T(`scout.part.${part ? `${agent}.${part}` : agent}`)), said)), ...body)
    node.style.setProperty('--agent', pet.color || AGENT_COLOR[agent])
    return node
  }

  // --- Her own --------------------------------------------------------------------------

  function barOf(wins, losses) {
    const bar = el('div', 'bar', el('i'))
    bar.firstChild.style.width = `${pct(wins, wins + losses)}%`
    return el('div', null, bar, el('div', 'wl', el('span', 'w', T('scout.w', { n: wins })), ' ', el('span', 'l', T('scout.l', { n: losses })), ` · ${pct(wins, wins + losses)}%`))
  }

  function rankCard(p) {
    const r = p.solo
    const said = r ? T('scout.say.rank', { tier: tierText(r), lp: r.lp, rate: pct(r.wins, r.wins + r.losses) }) : T('scout.say.unranked')
    const main = r
      ? el('div', 'rank-main', pic('div', 'emblem', emblem(r.tier)), el('div', 'rank-text', el('div', 'lp', T('scout.soloQ')), el('div', 'tier', tierText(r)), el('div', 'lp', `${r.lp} LP`), barOf(r.wins, r.losses)))
      : el('div', 'rank-main', el('div', 'tier', T('lol.unranked')))
    const f = p.flex
    const flex = f
      ? el(
          'div',
          'flex-row',
          pic('div', 'emblem small', emblem(f.tier)),
          el('div', null, el('div', 'q', T('scout.flexQ')), el('div', null, `${tierText(f)} · ${f.lp} LP`)),
          el('div', 'wl', el('span', 'w', T('scout.w', { n: f.wins })), ' ', el('span', 'l', T('scout.l', { n: f.losses })), ` · ${pct(f.wins, f.wins + f.losses)}%`),
        )
      : null
    const node = card('claude', said, 'opens', main, flex, el('div', 'more', T('scout.more.season')))
    node.addEventListener('click', () => openPage({ kind: 'season' }))
    return node
  }

  // Her tags, from what is known of her: the facts alone.
  function tagsOf(p) {
    const tags = []
    for (const m of p.top || []) if (m.points >= 1e6) tags.push(['gold', T('scout.tag.millionaire', { champ: champName(m.champion) })])
    const r = p.recent
    if (r && r.list) {
      const count = {}
      for (const g of r.list) count[g.champion] = (count[g.champion] || 0) + 1
      for (const [id, n] of Object.entries(count)) if (n >= 4) tags.push(['', T('scout.tag.lover', { champ: champName(id) })])
      if (r.streak >= 3) tags.push(['hot', T('scout.winStreak', { n: r.streak })])
      if (r.streak <= -3) tags.push(['cold', T('scout.loseStreak', { n: -r.streak })])
      if (r.games >= 5 && pct(r.wins, r.games) >= 70) tags.push(['hot', T('scout.tag.onFire')])
      if (r.games >= 5 && pct(r.wins, r.games) <= 30) tags.push(['cold', T('scout.tag.cold')])
    }
    if (p.solo && p.solo.hot) tags.push(['hot', T('scout.tag.hot')])
    if (p.solo && p.solo.veteran) tags.push(['', T('scout.tag.veteran')])
    return tags
  }

  function tagsCard(p) {
    const top = p.top || []
    const first = top[0]
    const said = first ? T(first.points >= 1e6 ? 'scout.say.millionaire' : 'scout.say.main', { champ: champName(first.champion), points: points(first.points) }) : '…'
    const tags = el(
      'div',
      'tags',
      tagsOf(p).map(([cls, text]) => el('span', `tag ${cls}`, text)),
    )
    const mains = el(
      'div',
      'mains',
      top.map(m => el('div', 'main', pic('div', 'champ-icon', champIcon(m.champion)), el('div', 'n', champName(m.champion)), el('div', 'pts', `${T('scout.masteryShort', { level: m.level })} · ${points(m.points)}`))),
    )
    const node = card('grok', said, 'opens', tags, mains, el('div', 'more', T('scout.more.masteries')))
    node.addEventListener('click', () => openPage({ kind: 'masteries' }))
    return node
  }

  function rolesCard(p) {
    const roles = p.roles || []
    const most = roles[0]
    const said = most ? T('scout.say.roles', { role: T(`scout.role.${most.pos}`), games: most.games, rate: pct(most.wins, most.games) }) : T('scout.say.rolesReading')
    const max = Math.max(1, ...roles.map(r => r.games))
    const rows = roles.slice(0, 5).map(r => {
      const track = el('div', 'track', el('i'))
      track.firstChild.style.width = `${(r.games / max) * 100}%`
      const rate = pct(r.wins, r.games)
      const row = el('div', 'role opens', el('span', null, T(`scout.role.${r.pos}`)), track, el('span', 'n', String(r.games)), el('span', `rate ${rate >= 50 ? 'good' : 'bad'}`, `${rate}%`))
      row.addEventListener('click', () => openPage({ kind: 'role', pos: r.pos }))
      return row
    })
    return card('agy', said, '', rows, p.rolesTotal ? el('div', 'reading', T('scout.rolesOf', { n: p.rolesOf, total: p.rolesTotal })) : null)
  }

  function gameRow(g) {
    const minutes = Math.max(1, (g.duration || 0) / 60)
    const ten = el(
      'div',
      'ten',
      [...(g.blue || []), ...(g.red || [])].map(id => pic('i', id === g.champion ? 'me' : '', champIcon(id))),
    )
    const row = el(
      'div',
      `game ${g.win ? 'won' : 'lost'}`,
      el('div', 'edge'),
      pic('div', 'champ-icon', champIcon(g.champion)),
      el('div', 'loadout', (g.spells || []).map(id => pic('i', null, spell(id))), pic('i', null, perk(g.keystone)), pic('i', null, perk(g.sub))),
      el('div', 'result', el('b', null, T(g.win ? 'scout.win' : 'scout.loss')), el('span', null, queueName(g.queue))),
      el('div', 'kda', el('b', null, `${g.k} / ${g.d} / ${g.a}`), el('span', null, champName(g.champion))),
      el('div', 'stats', el('div', null, T('scout.cs', { cs: g.cs, per: (g.cs / minutes).toFixed(1) })), el('div', null, T('scout.kp', { kp: g.kp }))),
      el('div', 'items', (g.items || []).map(id => pic('i', null, item(id)))),
      ten,
      el('div', 'ago', el('div', null, ago(g.end)), el('div', null, clock(g.duration || 0))),
    )
    row.addEventListener('click', () => openGame(g))
    return row
  }

  function gamesCard(p) {
    const r = p.recent
    const said = r ? T('scout.say.recent', { games: r.games, wins: r.wins, kda: `${r.k}/${r.d}/${r.a}` }) : T('scout.say.recentReading')
    return card('codex', said, 'wide', el('div', 'games', ((r && r.list) || []).map(gameRow)))
  }

  function drawProfile() {
    const p = data.profile
    films.length = 0
    $('profile').replaceChildren(rankCard(p), tagsCard(p), rolesCard(p), gamesCard(p))
    const avatar = pic('div', 'avatar', profileIcon(p.icon))
    if (p.level) avatar.append(el('span', 'lv', String(p.level)))
    $('who').replaceChildren(avatar, el('div', null, p.name || T('scout.mine'), el('span', 'sub', T(data.solo === 'nogame' ? 'scout.soloNogame' : 'scout.soloOutside'))))
  }

  // --- The game's ten ----------------------------------------------------------------------

  const TIERS = ['IRON', 'BRONZE', 'SILVER', 'GOLD', 'PLATINUM', 'EMERALD', 'DIAMOND', 'MASTER', 'GRANDMASTER', 'CHALLENGER']
  const DIVS = ['IV', 'III', 'II', 'I']

  // A rank as a number (each tier four divisions), and back: for a team's average.
  const scoreOf = r => TIERS.indexOf(r.tier) * 4 + (isApex(r.tier) ? 0 : DIVS.indexOf(r.rank))
  function averageOf(players) {
    const ranked = players.map(p => p.rank).filter(r => r && !r.unranked && TIERS.includes(r.tier))
    if (!ranked.length) return null
    const n = Math.round(ranked.reduce((sum, r) => sum + scoreOf(r), 0) / ranked.length)
    const tier = TIERS[Math.min(TIERS.length - 1, Math.floor(n / 4))]
    return { tier, rank: DIVS[n % 4] }
  }

  // Each player's tags: the facts of their last games and their mastery.
  function playerTags(p) {
    const tags = []
    const r = p.recent
    const m = p.mastery
    if (m && m.level === 0) tags.push(['cold', T('scout.firstTime')])
    if (m && m.points >= 1e6) tags.push(['gold', T('scout.tag.millionaire', { champ: champName(p.championId) })])
    if (r && r.list) {
      const on = r.list.filter(g => g.champion === p.championId).length
      if (on >= 6) tags.push(['gold', T('scout.tag.otp')])
      if (r.streak >= 3) tags.push(['hot', T('scout.winStreak', { n: r.streak })])
      if (r.streak <= -3) tags.push(['cold', T('scout.loseStreak', { n: -r.streak })])
      const roles = {}
      for (const g of r.list) if (g.pos) roles[g.pos] = (roles[g.pos] || 0) + 1
      const main = Object.entries(roles).sort((a, b) => b[1] - a[1])[0]
      if (main && main[1] >= Math.ceil(r.list.length / 2)) tags.push(['', T('scout.tag.mainRole', { role: T(`scout.role.${main[0]}`) })])
    }
    return tags
  }

  function rankBlock(r) {
    if (!r) return el('div', 'p-rank', el('span', 'wait', '…'))
    if (r.unranked) return el('div', 'p-rank', el('div', 'p-tier muted', T('scout.unrankedShort')))
    const n = r.wins + r.losses
    const rate = pct(r.wins, n)
    const bar = el('div', 'bar thin', el('i'))
    bar.firstChild.style.width = `${rate}%`
    return el(
      'div',
      'p-rank',
      pic('div', 'emblem tiny', emblem(r.tier)),
      el('div', null, el('div', 'p-tier', `${tierText(r)} · ${r.lp} LP`), bar, el('div', 'wl', el('span', 'w', T('scout.w', { n: r.wins })), ' ', el('span', 'l', T('scout.l', { n: r.losses })), ` · ${rate}%`)),
    )
  }

  function playerView(p) {
    const r = p.recent
    const m = p.mastery
    const dots = el(
      'div',
      'dots',
      ((r && r.list) || []).map(g => el('i', g.win ? 'w' : 'l')),
    )
    const facts = []
    if (m) facts.push(m.level ? el('span', null, T('scout.mastery', { level: m.level, points: points(m.points) })) : null)
    if (r) {
      facts.push(el('span', null, T('scout.kda', { k: r.k, d: r.d, a: r.a })))
      if (r.champGames) facts.push(el('span', null, T('scout.champ', { games: r.champGames, wins: r.champWins })))
    }
    const icon = pic('div', 'p-icon', champIcon(p.championId))
    if (p.group) icon.append(el('span', `party g${p.group}`, String(p.group)))
    const node = el(
      'div',
      `player${p.me ? ' me' : ''}${p.group ? ` in-party g${p.group}` : ''}`,
      icon,
      el('div', 'loadout', (p.spells || []).map(id => pic('i', null, spell(id))), pic('i', null, perk(p.keystone)), pic('i', null, perk(p.sub))),
      el(
        'div',
        'p-main',
        el('div', 'p-name', el('b', null, champName(p.championId) || p.champion || ''), el('span', null, p.me ? `${p.name} · ${T('scout.me')}` : p.name)),
        el('div', 'tags small', playerTags(p).map(([cls, text]) => el('span', `tag ${cls}`, text))),
        el('div', 'p-facts', r ? dots : el('span', 'wait', '…'), facts),
      ),
      rankBlock(p.rank),
    )
    if (p.group) node.title = T('scout.group', { n: p.group })
    node.addEventListener('click', () => openPlayer(p))
    // Hers: OP.GG's build against her lane opponent.
    if (p.me) {
      const button = el('button', 'og-open', T('scout.og.open'))
      button.addEventListener('click', e => {
        e.stopPropagation()
        openOpgg(p.championId, laneOf(p, p.championId), laneOpponent(p), data.them.map(x => x.championId))
      })
      node.querySelector('.p-main').append(button)
    }
    return node
  }

  // Summoner's Rift's queues (Solo/Duo, Flex, Normal, Quickplay): where positions mean something.
  const RIFT = [420, 440, 400, 430, 490]

  // The position a player plays a champion in, by their last games on the
  // Rift (else the one they play most there); none known, null (OP.GG then
  // takes the champion's usual one).
  function laneOf(p, champion) {
    const list = ((p.recent && p.recent.list) || []).filter(g => RIFT.includes(g.queue))
    const count = (games) => {
      const by = {}
      for (const g of games) if (g.pos) by[g.pos] = (by[g.pos] || 0) + 1
      return Object.entries(by).sort((a, b) => b[1] - a[1])[0]
    }
    const on = count(list.filter(g => g.champion === champion)) || count(list)
    return on ? on[0] : null
  }

  // Her lane opponent: the other team's player who plays her position most.
  function laneOpponent(p) {
    const lane = laneOf(p, p.championId)
    const rival = lane && (data.them || []).find(x => laneOf(x, x.championId) === lane)
    return rival ? rival.championId : null
  }

  // The four pets' lines over the two teams.
  function hostsOfTen() {
    const all = [...data.us, ...data.them]
    const us = averageOf(data.us)
    const them = averageOf(data.them)
    const claude = us && them ? T('scout.ten.claude', { us: tierText(us), them: tierText(them) }) : T('scout.ten.claudeWait')
    let grok = T('scout.ten.grokNone')
    const who = p => ({ champ: champName(p.championId) || p.champion, name: (p.name || '').split('#')[0] })
    const streaker = all.filter(p => p.recent && p.recent.streak >= 3).sort((a, b) => b.recent.streak - a.recent.streak)[0]
    const master = all.filter(p => p.mastery && p.mastery.points).sort((a, b) => b.mastery.points - a.mastery.points)[0]
    const first = all.find(p => p.mastery && p.mastery.level === 0)
    if (streaker) grok = T('scout.ten.grokStreak', { ...who(streaker), n: streaker.recent.streak })
    else if (master && master.mastery.points >= 100000) grok = T('scout.ten.grokMastery', { ...who(master), points: points(master.mastery.points) })
    else if (first) grok = T('scout.ten.grokFirst', who(first))
    const read = all.some(p => p.recent)
    const parties = list => new Set(list.filter(p => p.group).map(p => p.group)).size
    const agy = !read ? T('scout.ten.agyWait') : parties(all) ? T('scout.ten.agy', { us: parties(data.us), them: parties(data.them) }) : T('scout.ten.agyNone')
    const codex = data.total > 1 && data.done < data.total ? T('scout.ten.codex', { done: data.done, total: data.total }) : T('scout.ten.codexDone')
    return ['claude', 'grok', 'agy', 'codex'].map((agent, i) => {
      const node = el('div', 'mini-host', head(agent), el('div', 'speech', el('b', null, T(`scout.part.${agent}`).split(' · ')[0]), [claude, grok, agy, codex][i]))
      node.style.setProperty('--agent', ((data.pets || {})[agent] || {}).color || AGENT_COLOR[agent])
      return node
    })
  }

  function drawTen() {
    films.length = 0
    $('who').replaceChildren(el('div', null, T('scout.title')))
    $('hosts').replaceChildren(...hostsOfTen())
    $('us').textContent = T('scout.us')
    $('them').textContent = T('scout.them')
    document.querySelector('.us .rows').replaceChildren(...data.us.map(playerView))
    document.querySelector('.them .rows').replaceChildren(...data.them.map(playerView))
  }

  function draw() {
    $('panel').hidden = false
    $('hint').textContent = T('scout.hint2', { key: data.key || 'Alt+X' })
    if (data.state === 'select') {
      $('say').textContent = ''
      $('progress').textContent = ''
      $('profile').hidden = false
      $('profile').classList.add('select')
      $('teams').hidden = true
      $('hosts').hidden = true
      drawSelect()
      if (opened.length) drawDetail()
      return
    }
    $('profile').classList.remove('select')
    const ready = data.state === 'ready'
    const own = ready && !!data.profile
    $('say').textContent = ready ? '' : T(`scout.${data.state || 'none'}`)
    if (!ready) $('who').replaceChildren(el('div', null, T('scout.title')))
    $('progress').textContent = ''
    $('profile').hidden = !own
    $('teams').hidden = !ready || own
    $('hosts').hidden = !ready || own
    if (own) drawProfile()
    else if (ready) drawTen()
    // The page opened over them is drawn again too, its pets moving.
    if (opened.length) drawDetail()
  }

  // --- Opened: a player's games, a game's scoreboard ----------------------------------------

  // What is open, the latest last: { kind: 'player' | 'game', ... }.
  const opened = []

  function back() {
    opened.pop()
    drawDetail()
  }

  function openPlayer(p) {
    opened.push({ kind: 'player', player: p })
    drawDetail()
  }

  function openGame(g) {
    const page = { kind: 'game', row: g, game: null }
    opened.push(page)
    drawDetail()
    window.pet.scoutGame(g.id).then(game => {
      page.game = game
      if (opened[opened.length - 1] === page) drawDetail()
    })
  }

  function backBar(...rest) {
    const button = el('button', 'back', T('scout.back'))
    button.addEventListener('click', back)
    // Its own close button: the page covers the panel's.
    const close = el('button', 'close', '✕')
    close.id = ''
    close.addEventListener('click', () => window.pet.scoutClose())
    return el('div', 'detail-bar', button, ...rest, close)
  }

  function playerPage(opened) {
    // As now read: their games keep coming in while the ten are looked up.
    const p = (data.us && [...data.us, ...data.them].find(x => x.name === opened.name)) || opened
    const r = p.recent
    const said = r ? T('scout.say.recent', { games: r.games, wins: r.wins, kda: `${r.k}/${r.d}/${r.a}` }) : T('scout.say.recentReading')
    const top = el(
      'div',
      'who-card',
      pic('div', 'p-icon big', champIcon(p.championId)),
      el('div', 'p-main', el('div', 'p-name', el('b', null, champName(p.championId) || p.champion || ''), el('span', null, p.name)), el('div', 'tags small', playerTags(p).map(([cls, text]) => el('span', `tag ${cls}`, text)))),
      rankBlock(p.rank),
    )
    return [backBar(), top, card('codex', said, 'wide grow', el('div', 'games', ((r && r.list) || []).map(gameRow)))]
  }

  function boardRow(q, game, row, best) {
    const dmg = el('div', 'track dmg', el('i'))
    dmg.firstChild.style.width = `${best ? (q.dmg / best) * 100 : 0}%`
    const minutes = Math.max(1, (game.duration || 0) / 60)
    return el(
      'div',
      `board-row${q.puuid === row.who ? ' me' : ''}`,
      pic('div', 'champ-icon', champIcon(q.champion)),
      el('div', 'loadout', (q.spells || []).map(id => pic('i', null, spell(id))), pic('i', null, perk(q.keystone)), pic('i', null, perk(q.sub))),
      el('div', 'b-name', el('b', null, (q.name || '').split('#')[0] || champName(q.champion)), el('span', null, `${champName(q.champion)}${q.pos ? ` · ${T(`scout.role.${q.pos}`)}` : ''}`)),
      el('div', 'kda', el('b', null, `${q.k} / ${q.d} / ${q.a}`), el('span', null, T('scout.kp', { kp: q.kp }))),
      el('div', 'b-dmg', el('span', null, (q.dmg || 0).toLocaleString()), dmg),
      el('div', 'stats', el('div', null, T('scout.board.gold', { gold: ((q.gold || 0) / 1000).toFixed(1) })), el('div', null, T('scout.cs', { cs: q.cs, per: (q.cs / minutes).toFixed(1) }))),
      el('div', 'items', (q.items || []).map(id => pic('i', null, item(id)))),
    )
  }

  function gamePage(page) {
    const g = page.row
    const head = backBar(el('b', `res ${g.win ? 'good' : 'bad'}`, T(g.win ? 'scout.win' : 'scout.loss')), el('span', null, `${queueName(g.queue)} · ${clock(g.duration || 0)} · ${ago(g.end)}`))
    const game = page.game
    if (!game || !game.players) return [head, el('div', 'say', game === null ? '…' : T('scout.board.none'))]
    const best = Math.max(1, ...game.players.map(q => q.dmg || 0))
    const top = game.players.reduce((a, b) => ((b.dmg || 0) > (a.dmg || 0) ? b : a))
    const said = T('scout.board.say', { champ: champName(top.champion), name: (top.name || '').split('#')[0], dmg: (top.dmg || 0).toLocaleString() })
    const team = id => {
      const list = game.players.filter(q => q.team === id)
      const won = list.length && list[0].win
      const kills = list.reduce((n, q) => n + (q.k || 0), 0)
      return el('div', `board-team ${won ? 'won' : 'lost'}`, el('h3', null, `${T(won ? 'scout.win' : 'scout.loss')} · ${T('scout.board.kills', { n: kills })}`), list.map(q => boardRow(q, game, g, best)))
    }
    if (page.timeline === undefined) {
      page.timeline = null
      window.pet.scoutTimeline(g.id).then(t => {
        page.timeline = t || false
        if (opened[opened.length - 1] === page) drawDetail()
      })
    }
    // Her own games are reviewed (not another player's: her tokens).
    const own = data.profile && g.who === data.profile.me
    return [head, card('codex', said, 'wide', el('div', 'board', team(100), team(200))), own ? reviewCard(page) : null, timelineCard(game, g, page.timeline)].filter(Boolean)
  }

  // --- The four agents' review of a game ----------------------------------------------------

  const REVIEWERS = ['claude', 'codex', 'agy', 'grok']

  function pollReview(page) {
    if (page.polling) return
    page.polling = true
    const look = () =>
      window.pet.scoutReview(page.row.id).then(r => {
        page.review = r || {}
        if (opened[opened.length - 1] === page) drawDetail()
        const busy = REVIEWERS.some(a => page.review[a] && page.review[a].state === 'running')
        if (busy && opened.includes(page)) setTimeout(look, 2000)
        else page.polling = false
      })
    look()
  }

  function reviewCard(page) {
    if (page.review === undefined) {
      page.review = null
      page.chosen = new Set(REVIEWERS)
      pollReview(page)
    }
    const r = page.review || {}
    const rows = REVIEWERS.map(agent => {
      const done = r[agent] || {}
      const box = el('input')
      box.type = 'checkbox'
      box.checked = page.chosen.has(agent)
      box.disabled = done.state === 'done' || done.state === 'running'
      box.addEventListener('click', e => e.stopPropagation())
      box.addEventListener('change', () => (box.checked ? page.chosen.add(agent) : page.chosen.delete(agent)))
      const text =
        done.state === 'done' ? done.text : done.state === 'running' ? T('scout.rv.thinking') : done.state === 'failed' ? T('scout.rv.failed', { why: done.why || '' }) : T('scout.rv.idle')
      // **bold** as the agents write it, bold; the rest as text.
      const words = String(text).split('**').map((part, i) => (i % 2 ? el('strong', null, part) : part))
      const node = el('div', `rv ${done.state || 'idle'}`, el('label', 'rv-pick', box, head(agent)), el('div', 'speech', el('b', null, T(`scout.rv.part.${agent}`)), el('div', 'rv-text', words)))
      node.style.setProperty('--agent', ((data.pets || {})[agent] || {}).color || AGENT_COLOR[agent])
      return node
    })
    const todo = REVIEWERS.filter(a => page.chosen.has(a) && !['done', 'running'].includes((r[a] || {}).state))
    const go = el('button', 'rv-go', T('scout.rv.go', { n: todo.length }))
    go.disabled = !todo.length
    go.addEventListener('click', () => {
      go.disabled = true
      window.pet.scoutReviewStart(page.row.id, page.row.who, todo).then(now => {
        page.review = now || page.review
        page.polling = false
        pollReview(page)
      })
    })
    const all = REVIEWERS.every(a => (r[a] || {}).state === 'done')
    return card('claude:rv', T(all ? 'scout.rv.allDone' : 'scout.rv.say'), 'wide', el('div', 'rv-list', rows), all ? null : el('div', 'rv-foot', go, el('span', null, T('scout.rv.cost'))))
  }

  // --- A game's timeline: its curves, her build, skills and kills --------------------------

  const SVG = 'http://www.w3.org/2000/svg'
  function svg(tag, attrs, ...kids) {
    const node = document.createElementNS(SVG, tag)
    for (const [k, v] of Object.entries(attrs || {})) node.setAttribute(k, v)
    for (const kid of kids.flat()) if (kid) node.append(kid)
    return node
  }

  // A line chart: series [{ values, color, width, dash, fill }] over minutes, a zero line if asked.
  function chart(series, { height = 170, zero = false, marks = [] } = {}) {
    const W = 1000
    const H = height
    const pad = { l: 46, r: 10, t: 10, b: 22 }
    const n = Math.max(...series.map(s => s.values.length))
    const all = series.flatMap(s => s.values)
    let lo = Math.min(0, ...all)
    let hi = Math.max(0, ...all)
    if (zero) {
      const m = Math.max(Math.abs(lo), Math.abs(hi), 1)
      lo = -m
      hi = m
    }
    if (hi === lo) hi = lo + 1
    const x = i => pad.l + (i / Math.max(1, n - 1)) * (W - pad.l - pad.r)
    const y = v => pad.t + (1 - (v - lo) / (hi - lo)) * (H - pad.t - pad.b)
    const grid = []
    for (let i = 0; i < n; i += 5) grid.push(svg('line', { x1: x(i), x2: x(i), y1: pad.t, y2: H - pad.b, class: 'grid' }), svg('text', { x: x(i), y: H - 6, class: 'axis', 'text-anchor': 'middle' }, document.createTextNode(String(i))))
    for (const v of zero ? [lo, 0, hi] : [lo, (lo + hi) / 2, hi]) grid.push(svg('text', { x: pad.l - 6, y: y(v) + 4, class: 'axis', 'text-anchor': 'end' }, document.createTextNode(Math.abs(v) >= 1000 ? `${(v / 1000).toFixed(1)}k` : String(Math.round(v)))))
    if (zero) grid.push(svg('line', { x1: pad.l, x2: W - pad.r, y1: y(0), y2: y(0), class: 'zero' }))
    const lines = series.map(s => {
      const d = s.values.map((v, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('')
      return svg('path', { d, fill: 'none', stroke: s.color, 'stroke-width': s.width || 2.5, 'stroke-dasharray': s.dash || '', 'stroke-linejoin': 'round' })
    })
    // Ahead and behind, filled to the zero line.
    const fills = series
      .filter(s => s.fill)
      .flatMap(s => {
        const path = `M${x(0)},${y(0)}` + s.values.map((v, i) => `L${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('') + `L${x(s.values.length - 1)},${y(0)}Z`
        const id = `clip${Math.random().toString(36).slice(2)}`
        return [
          svg('defs', null, svg('clipPath', { id: `${id}a` }, svg('rect', { x: 0, y: 0, width: W, height: y(0) })), svg('clipPath', { id: `${id}b` }, svg('rect', { x: 0, y: y(0), width: W, height: H }))),
          svg('path', { d: path, fill: s.fill[0], 'clip-path': `url(#${id}a)` }),
          svg('path', { d: path, fill: s.fill[1], 'clip-path': `url(#${id}b)` }),
        ]
      })
    const dots = marks.map(m => svg('g', null, svg('circle', { cx: x(m.t / 60), cy: H - pad.b - 6, r: 6, fill: m.color }), svg('text', { x: x(m.t / 60), y: H - pad.b - 2.5, class: 'mark', 'text-anchor': 'middle' }, document.createTextNode(m.label))))
    return svg('svg', { viewBox: `0 0 ${W} ${H}`, class: 'chart' }, grid, fills, lines, dots)
  }

  const CONSUMABLES = new Set([2003, 2031, 2033, 2055, 2138, 2139, 2140, 3340, 3363, 3364, 3330, 2010, 2009, 2052, 3513])

  function timelineCard(game, row, t) {
    const legend = (...items) => el('div', 'legend', items.map(([color, text]) => el('span', null, el('i', null), text)).map((node, i) => ((node.firstChild.style.background = items[i][0]), node)))
    if (t === null || t === undefined) return card('agy:tl', T('scout.tl.loading'), 'wide')
    if (!t || !t.frames) return card('agy:tl', T('scout.tl.none'), 'wide')
    // Each of the ten by their place in the timeline (1 to 10).
    const who = t.puuids.map(puuid => game.players.find(q => q.puuid === puuid) || {})
    const mine = who.findIndex(q => q.puuid === row.who)
    if (mine < 0) return card('agy:tl', T('scout.tl.none'), 'wide')
    const myTeam = who[mine].team
    const ours = i => who[i].team === myTeam
    const diff = t.frames.map(f => f.gold.reduce((s, g, i) => s + (ours(i) ? g : -g), 0))
    const biggest = diff.reduce((b, v, i) => (Math.abs(v) > Math.abs(diff[b]) ? i : b), 0)
    const said = T('scout.tl.say', { m: biggest, who: diff[biggest] >= 0 ? T('scout.tl.us') : T('scout.tl.them'), gold: Math.abs(diff[biggest]).toLocaleString() })
    const marks = t.monsters.map(m => ({
      t: m.t,
      color: m.team === myTeam ? 'rgba(79,216,143,0.9)' : 'rgba(255,107,107,0.9)',
      label: T(`scout.tl.${m.type === 'DRAGON' ? 'dragon' : m.type === 'BARON_NASHOR' ? 'baron' : m.type === 'RIFTHERALD' ? 'herald' : 'other'}`),
    }))
    const goldDiff = el('div', 'tl-block', el('h4', null, T('scout.tl.goldDiff')), chart([{ values: diff, color: '#cfd8ff', width: 2, fill: ['rgba(79,216,143,0.35)', 'rgba(255,107,107,0.35)'] }], { zero: true, marks }))
    // Her lane opponent: the other team's player in her position.
    const opp = who.findIndex((q, i) => !ours(i) && q.pos && q.pos === who[mine].pos)
    const meGold = t.frames.map(f => f.gold[mine])
    const meCs = t.frames.map(f => f.cs[mine])
    const vs = [{ values: meGold, color: '#e6c36a', width: 3 }]
    const vsCs = [{ values: meCs, color: '#e6c36a', width: 3 }]
    if (opp >= 0) {
      vs.push({ values: t.frames.map(f => f.gold[opp]), color: '#ff6b6b', width: 2.5, dash: '6 4' })
      vsCs.push({ values: t.frames.map(f => f.cs[opp]), color: '#ff6b6b', width: 2.5, dash: '6 4' })
    }
    const oppName = opp >= 0 ? T('scout.tl.opp', { champ: champName(who[opp].champion) }) : null
    const lane = el(
      'div',
      'tl-pair',
      el('div', 'tl-block', el('h4', null, T('scout.tl.vsGold')), legend(['#e6c36a', T('scout.tl.me')], ...(oppName ? [['#ff6b6b', oppName]] : [])), chart(vs, { height: 150 })),
      el('div', 'tl-block', el('h4', null, T('scout.tl.vsCs')), legend(['#e6c36a', T('scout.tl.me')], ...(oppName ? [['#ff6b6b', oppName]] : [])), chart(vsCs, { height: 150 })),
    )
    // Her build: items bought, each with its minute; potions and wards left out.
    const bought = t.items.filter(e => e.p === mine + 1 && !CONSUMABLES.has(e.item))
    const build = el(
      'div',
      'tl-block',
      el('h4', null, T('scout.tl.build')),
      el(
        'div',
        'build',
        bought.map(e => el('div', 'buy', pic('i', null, item(e.item)), el('span', null, `${Math.floor(e.t / 60)}:${String(Math.round(e.t % 60)).padStart(2, '0')}`))),
      ),
    )
    // Her skills: which of Q W E R at each level.
    const order = t.skills.filter(e => e.p === mine + 1).map(e => e.slot)
    const skills = el(
      'div',
      'tl-block',
      el('h4', null, T('scout.tl.skills')),
      el(
        'div',
        'skills',
        ['Q', 'W', 'E', 'R'].map((key, slot) => el('div', 'skill-row', el('b', null, key), order.map((s, lv) => el('i', s === slot + 1 ? 'on' : '', s === slot + 1 ? String(lv + 1) : '')))),
      ),
    )
    // Her kills, deaths and assists where they were.
    const MAP = 15000
    const map = el('div', 'minimap', pic('div', 'map-bg', data.dd ? `${DDRAGON}/${data.dd.v}/img/map/map11.png` : ''))
    for (const k of t.kills) {
      const kind = k.killer === mine + 1 ? 'kill' : k.victim === mine + 1 ? 'death' : (k.assists || []).includes(mine + 1) ? 'assist' : null
      if (!kind) continue
      const dot = el('i', `dot ${kind}`)
      dot.style.left = `${(k.x / MAP) * 100}%`
      dot.style.top = `${(1 - k.y / MAP) * 100}%`
      dot.title = `${Math.floor(k.t / 60)}:${String(Math.round(k.t % 60)).padStart(2, '0')}`
      map.append(dot)
    }
    const mapBlock = el('div', 'tl-block', el('h4', null, T('scout.tl.map')), legend(['#4fd88f', T('scout.tl.kill')], ['#ff6b6b', T('scout.tl.death')], ['#5aa2ff', T('scout.tl.assist')]), map)
    return card('agy:tl', said, 'wide', goldDiff, lane, el('div', 'tl-pair', el('div', null, build, skills), mapBlock))
  }

  function openPage(page) {
    opened.push(page)
    drawDetail()
  }

  // Her mastery of a champion, from every one's (the profile's masteries).
  const masteryOf = id => ((data.profile && data.profile.masteries) || []).find(m => m.champion === Number(id))

  // A table of champions: each row its icon and name, games, won and lost
  // as a bar, K/D/A, minions a minute, her mastery; a click, its games.
  function champTable(champs) {
    const head = el('div', 'c-row c-head', el('span'), el('span', null, T('scout.col.champ')), el('span', null, T('scout.col.games')), el('span', null, T('scout.col.rate')), el('span', null, T('scout.col.kda')), el('span', null, T('scout.col.cs')), el('span', null, T('scout.col.mastery')))
    const rows = champs.map(c => {
      const rate = pct(c.wins, c.games)
      const bar = el('div', 'bar thin', el('i'))
      bar.firstChild.style.width = `${rate}%`
      const kda = c.d ? ((c.k + c.a) / c.d).toFixed(2) : '∞'
      const m = masteryOf(c.champion)
      const row = el(
        'div',
        'c-row opens',
        pic('div', 'champ-icon', champIcon(c.champion)),
        el('b', null, champName(c.champion)),
        el('span', null, String(c.games)),
        el('div', null, el('div', 'wl', el('span', 'w', T('scout.w', { n: c.wins })), ' ', el('span', 'l', T('scout.l', { n: c.games - c.wins })), ' · ', el('b', rate >= 50 ? 'good' : 'bad', `${rate}%`)), bar),
        el('div', null, el('b', null, kda), el('div', 'sub', `${(c.k / c.games).toFixed(1)} / ${(c.d / c.games).toFixed(1)} / ${(c.a / c.games).toFixed(1)}`)),
        el('span', null, (c.cs / Math.max(1, c.minutes)).toFixed(1)),
        el('span', 'gold-text', m ? `${T('scout.masteryShort', { level: m.level })} · ${points(m.points)}` : '—'),
      )
      row.addEventListener('click', () => openPage({ kind: 'champ', champion: c.champion, ids: c.ids, rows: null }))
      return row
    })
    return el('div', 'c-table', head, rows)
  }

  function seasonPage() {
    const p = data.profile
    const s = p.season
    const champs = (s && s.champs) || []
    const r = p.solo
    const said = champs.length ? T('scout.say.season', { games: p.rolesOf, champs: champs.length, champ: champName(champs[0].champion), n: champs[0].games }) : T('scout.say.rolesReading')
    const reading = p.rolesOf < p.rolesTotal ? el('span', null, T('scout.rolesOf', { n: p.rolesOf, total: p.rolesTotal })) : null
    return [backBar(el('b', null, T('scout.page.season')), r ? el('span', null, `${tierText(r)} · ${T('scout.w', { n: r.wins })} ${T('scout.l', { n: r.losses })}`) : null, reading), card('claude', said, 'wide grow', champTable(champs))]
  }

  function rolePage(page) {
    const p = data.profile
    const role = (p.roles || []).find(r => r.pos === page.pos) || { games: 0, wins: 0, champs: [] }
    const said = T('scout.say.role', { role: T(`scout.role.${page.pos}`), games: role.games, rate: pct(role.wins, role.games), champs: (role.champs || []).length })
    return [backBar(el('b', null, T(`scout.role.${page.pos}`))), card('agy', said, 'wide grow', champTable(role.champs || []))]
  }

  function masteriesPage() {
    const p = data.profile
    const all = p.masteries || []
    const total = all.reduce((n, m) => n + (m.points || 0), 0)
    const said = T('scout.say.masteries', { n: all.length, points: points(total) })
    const champs = ((p.season && p.season.champs) || [])
    const grid = el(
      'div',
      'm-grid',
      all.map(m => {
        const c = champs.find(x => x.champion === m.champion)
        const tile = el(
          'div',
          `m-tile${c ? ' opens' : ''}${m.points >= 1e6 ? ' rich' : ''}`,
          pic('div', 'champ-icon', champIcon(m.champion)),
          el('div', 'm-name', champName(m.champion)),
          el('div', 'm-level', T('scout.masteryShort', { level: m.level })),
          el('div', 'm-points', points(m.points)),
          c ? el('div', `m-season ${pct(c.wins, c.games) >= 50 ? 'good' : 'bad'}`, `${c.games} · ${pct(c.wins, c.games)}%`) : null,
        )
        if (c) tile.addEventListener('click', () => openPage({ kind: 'champ', champion: c.champion, ids: c.ids, rows: null }))
        return tile
      }),
    )
    return [backBar(el('b', null, T('scout.page.masteries'))), card('grok', said, 'wide grow', grid)]
  }

  function champPage(page) {
    const p = data.profile
    const c = (((p.season && p.season.champs) || []).find(x => x.champion === page.champion)) || null
    if (!page.rows) {
      page.rows = []
      window.pet.scoutRows(page.ids, p.me).then(rows => {
        page.rows = rows || []
        if (opened[opened.length - 1] === page) drawDetail()
      })
    }
    const said = c ? T('scout.say.champ', { champ: champName(page.champion), games: c.games, rate: pct(c.wins, c.games), kda: c.d ? ((c.k + c.a) / c.d).toFixed(2) : '∞' }) : '…'
    const roles = Object.entries((c && c.roles) || {}).sort((a, b) => b[1] - a[1])
    const og = el('button', 'og-open', T('scout.og.open'))
    og.addEventListener('click', () => openOpgg(page.champion, roles.length ? roles[0][0] : 'MIDDLE', null, null))
    return [backBar(pic('div', 'champ-icon', champIcon(page.champion)), el('b', null, champName(page.champion)), og), card('codex', said, 'wide grow', el('div', 'games', page.rows.map(gameRow)))]
  }

  function openOpgg(champion, pos, opponent, enemies) {
    openPage({ kind: 'opgg', champion, pos, opponent, enemies, result: undefined })
  }

  // An OP.GG lookup (page: { champion, pos, opponent, enemies, result,
  // redraw }): asked once for each opponent; drawn again when it comes.
  function askOpgg(page) {
    page.result = null
    const asked = page.opponent
    window.pet.scoutOpgg(page.champion, page.pos || '', page.opponent).then(r => {
      if (page.opponent !== asked) return
      page.result = r || { error: 'no answer' }
      ;(page.redraw || drawDetail)()
    })
  }

  const OPGG_ROLE = { TOP: 'TOP', JUNGLE: 'JUNGLE', MID: 'MIDDLE', ADC: 'BOTTOM', SUPPORT: 'UTILITY' }
  const pct1 = v => (typeof v === 'number' ? `${(v * 100).toFixed(1)}%` : '—')
  const shard = id => (data.dd && data.dd.shards && data.dd.shards[id]) || ''

  // The position looked up: the one asked, or the one OP.GG took.
  function roleOfPage(page) {
    const r = page.result
    return page.pos || (r && OPGG_ROLE[String(r.position || '').toUpperCase()]) || ''
  }

  // What OP.GG has of a champion, as much as it gives: its standing, the
  // whole build, the whole rune page, spells and skills, the hardest and the
  // easiest matchups (a click looks one up), tips. [the other team to pick
  // from, the card].
  function opggCard(page) {
    if (page.result === undefined) askOpgg(page)
    const r = page.result
    const vsName = page.opponent ? champName(page.opponent) : ''
    const setOpponent = id => {
      page.opponent = page.opponent === id ? null : id
      askOpgg(page)
      ;(page.redraw || drawDetail)()
    }
    const pick = (page.enemies || []).length
      ? el(
          'div',
          'og-pick',
          el('span', null, T('scout.og.against')),
          page.enemies.map(id => {
            const i = pic('i', `og-enemy${id === page.opponent ? ' on' : ''}`, champIcon(id))
            i.title = champName(id)
            i.addEventListener('click', () => setOpponent(id))
            return i
          }),
        )
      : null
    if (r === null || r === undefined) return [pick, card('claude:og', T('scout.og.asking'), 'wide')]
    if (r.error) return [pick, card('claude:og', T('scout.og.none', { why: r.error }), 'wide')]
    const roleName = roleOfPage(page) ? T(`scout.role.${roleOfPage(page)}`) : ''
    const said = T(r.vs != null && vsName ? 'scout.og.sayVs' : 'scout.og.say', { champ: champName(page.champion), role: roleName, wr: pct1(r.winrate), vs: vsName, vwr: pct1(r.vs), patch: r.patch || '' })
    const block = (title, ...kids) => el('div', 'og-block', el('h4', null, title), el('div', 'og-row', ...kids))
    const icons = (ids, f, cls = 'og-icon') => (ids || []).filter(id => id != null).map(id => pic('i', cls, f(id)))
    const stat = (label, value) => el('div', 'og-stat', el('span', null, label), el('b', null, value))
    const stats = el(
      'div',
      'og-stats',
      r.tier ? stat(T('scout.og.tier'), `T${r.tier}`) : null,
      stat(T('scout.col.rate'), pct1(r.winrate)),
      stat(T('scout.og.pick'), pct1(r.pickrate)),
      stat(T('scout.og.ban'), pct1(r.banrate)),
      vsName ? stat(T('scout.og.vsStat', { vs: vsName }), pct1(r.vs)) : null,
    )
    const arrow = () => el('span', 'og-plus', '→')
    const options = (label, ids) => ((ids || []).length ? el('div', 'og-option', el('span', null, label), icons(ids, item, 'og-icon small')) : null)
    const build = el(
      'div',
      'og-block wide',
      el('h4', null, T('scout.og.build')),
      el('div', 'og-row', icons(r.starting, item, 'og-icon small'), (r.starting || []).length ? arrow() : null, icons(r.core, item), el('span', 'og-plus', '+'), icons([r.boots], item)),
      el('div', 'og-options', options(T('scout.og.fourth'), r.fourth), options(T('scout.og.fifth'), r.fifth), options(T('scout.og.sixth'), r.sixth)),
    )
    const runes = r.runes || { keystone: r.keystone, secondary: r.sub }
    const runeBlock = el(
      'div',
      'og-block',
      el('h4', null, T('scout.og.runes')),
      el(
        'div',
        'og-runes',
        el('div', 'og-tree', icons([runes.primary], perk, 'og-icon tiny'), icons([runes.keystone], perk, 'og-icon big'), icons(runes.primaries, perk, 'og-icon small')),
        el('div', 'og-tree', icons([runes.secondary], perk, 'og-icon tiny'), icons(runes.secondaries, perk, 'og-icon small')),
        el('div', 'og-tree', icons(runes.shards, shard, 'og-icon tiny round')),
      ),
    )
    const order = String(r.skillOrder || '').split(/[,，\s>]+/).filter(Boolean)
    const skillBlock = el(
      'div',
      'og-block',
      el('h4', null, T('scout.og.spellsSkills')),
      el('div', 'og-row', icons(r.spells, spell), el('span', 'og-sep'), el('b', 'og-skills', r.skills || '—')),
      order.length ? el('div', 'og-order', order.map((k, n) => el('span', `k-${k.toLowerCase()}`, el('i', null, String(n + 1)), k))) : null,
    )
    const champs = (list, cls) =>
      (list || []).map(c => {
        // Earlier lookups kept names alone (counterIds beside them).
        const id = typeof c === 'object' ? c.id : null
        if (!id) return null
        const node = el('div', `og-counter ${cls}${id === page.opponent ? ' on' : ''}`, pic('i', 'og-icon', champIcon(id)), el('span', null, champName(id)), typeof c.winrate === 'number' ? el('b', null, pct1(c.winrate)) : null)
        node.addEventListener('click', () => setOpponent(id))
        return node
      })
    const old = (r.counterIds || []).map(id => ({ id }))
    const matchups = el(
      'div',
      'og-block wide',
      el('div', 'og-two', el('div', null, el('h4', null, T('scout.og.counters')), el('div', 'og-row', champs((r.counters || []).some(c => typeof c === 'object') ? r.counters : old, 'hard'))), el('div', null, el('h4', null, T('scout.og.easy')), el('div', 'og-row', champs(r.easy, 'easy')))),
    )
    const tips = (r.tips || []).length ? el('div', 'og-tips', el('h4', null, vsName ? T('scout.og.tipsVs', { vs: vsName }) : T('scout.og.tips')), (r.tips || []).map(t => el('div', 'og-tip', t))) : null
    return [pick, card('claude:og', said, 'wide', stats, el('div', 'og-grid', build, runeBlock, skillBlock, matchups), tips, el('div', 'og-source', T('scout.og.source')))]
  }

  function opggPage(page) {
    const roleName = roleOfPage(page) ? T(`scout.role.${roleOfPage(page)}`) : ''
    const vsName = page.opponent ? champName(page.opponent) : ''
    const head = backBar(pic('div', 'champ-icon', champIcon(page.champion)), el('b', null, champName(page.champion)), roleName ? el('span', null, roleName) : null, vsName ? el('span', null, `vs ${vsName}`) : null)
    return [head, ...opggCard(page)].filter(Boolean)
  }

  // --- Champion select -------------------------------------------------------------------------

  // What is kept of this champion select: the build of her champion, the
  // picks against an opponent, the position's strongest; for as long as it lasts.
  let sel = null

  function selectState(s) {
    if (!sel || sel.pos !== s.pos) sel = { pos: s.pos, build: null, picks: null, meta: undefined }
    const redraw = () => draw()
    // Her champion locked in: OP.GG's build for it, by itself.
    if (s.champion && (!sel.build || sel.build.champion !== s.champion)) sel.build = { champion: s.champion, pos: s.pos, opponent: null, enemies: [], result: undefined, redraw }
    if (sel.build) sel.build.enemies = s.enemies
    // The position's strongest, once.
    if (sel.meta === undefined && s.pos) {
      sel.meta = null
      window.pet.scoutOpggMeta(s.pos).then(r => {
        sel.meta = r || { error: 'no answer' }
        draw()
      })
    }
    return redraw
  }

  // Her champions, the most mastered first, none picked or taken yet.
  function candidatesOf(s) {
    const taken = new Set([...(s.allies || []), ...(s.enemies || [])])
    return ((data.pool || []).filter(m => !taken.has(m.champion)) || []).slice(0, 10)
  }

  function askPicks(s, opponent) {
    const candidates = candidatesOf(s)
    sel.picks = { opponent, result: null, pool: candidates }
    window.pet.scoutOpggPicks(opponent, s.pos, candidates.map(m => m.champion)).then(r => {
      if (!sel.picks || sel.picks.opponent !== opponent) return
      sel.picks.result = r || { error: 'no answer' }
      draw()
    })
    draw()
  }

  function picksCard(s) {
    const enemies = s.enemies || []
    const p = sel.picks
    const row = el(
      'div',
      'og-pick',
      el('span', null, T('scout.sel.against')),
      enemies.length
        ? enemies.map(id => {
            const i = pic('i', `og-enemy${p && p.opponent === id ? ' on' : ''}`, champIcon(id))
            i.title = champName(id)
            i.addEventListener('click', () => askPicks(s, id))
            return i
          })
        : el('span', 'muted', T('scout.sel.noEnemies')),
    )
    if (!data.pool) return card('grok:sel', T('scout.sel.pool'), '', row)
    if (!p) return card('grok:sel', T('scout.sel.pickOne'), '', row)
    const vs = champName(p.opponent)
    if (!p.result) return card('grok:sel', T('scout.sel.asking', { vs, n: p.pool.length }), '', row)
    if (p.result.error) return card('grok:sel', T('scout.og.none', { why: p.result.error }), '', row)
    const mastery = id => (data.pool || []).find(m => m.champion === id) || {}
    // Those with games enough to tell (100 or more) first, by win rate; the few after.
    const few = x => (x.games || 0) < 100
    const ranked = (p.result.picks || []).filter(x => typeof x.winrate === 'number' && x.id).sort((a, b) => few(a) - few(b) || b.winrate - a.winrate).slice(0, 5)
    const said = ranked.length ? T('scout.sel.best', { vs, champ: champName(ranked[0].id), wr: pct1(ranked[0].winrate) }) : T('scout.sel.nothing', { vs })
    const list = el(
      'div',
      'sel-picks',
      ranked.map((x, n) => {
        const m = mastery(x.id)
        const isFew = few(x)
        const node = el(
          'div',
          'sel-pick opens',
          el('span', 'sel-n', String(n + 1)),
          pic('i', 'og-icon', champIcon(x.id)),
          el('div', 'sel-name', el('b', null, champName(x.id)), el('span', null, m.level ? `${T('scout.masteryShort', { level: m.level })} · ${points(m.points)}` : '')),
          el('div', 'sel-wr', el('b', x.winrate >= 0.5 ? 'good' : 'bad', pct1(x.winrate)), el('span', isFew ? 'few' : null, isFew ? T('scout.sel.few', { n: x.games || 0 }) : T('scout.sel.games', { n: x.games }))),
        )
        node.addEventListener('click', () => openOpgg(x.id, s.pos, p.opponent, enemies))
        return node
      }),
    )
    return card('grok:sel', said, '', row, list)
  }

  function metaCard(s) {
    const m = sel.meta
    if (!s.pos) return card('agy:sel', T('scout.sel.noPos'), '')
    if (!m) return card('agy:sel', T('scout.sel.metaAsking', { role: T(`scout.role.${s.pos}`) }), '')
    if (m.error) return card('agy:sel', T('scout.og.none', { why: m.error }), '')
    const mine = id => (data.pool || []).find(x => x.champion === id)
    const rows = (m.champs || []).filter(c => c.id).slice(0, 10).map(c => {
      const own = mine(c.id)
      const node = el(
        'div',
        'meta-row opens',
        el('span', `meta-tier t${c.tier}`, `T${c.tier}`),
        pic('i', 'og-icon small', champIcon(c.id)),
        el('b', null, champName(c.id)),
        el('span', 'good', pct1(c.winrate)),
        el('span', 'muted', `${T('scout.og.pick')} ${pct1(c.pickrate)}`),
        el('span', 'gold-text', own && own.level ? T('scout.masteryShort', { level: own.level }) : ''),
      )
      node.addEventListener('click', () => openOpgg(c.id, s.pos, null, s.enemies || []))
      return node
    })
    return card('agy:sel', T('scout.sel.meta', { role: T(`scout.role.${s.pos}`) }), '', el('div', 'meta-list', rows))
  }

  function drawSelect() {
    films.length = 0
    const s = data.select || {}
    selectState(s)
    const role = s.pos ? T(`scout.role.${s.pos}`) : ''
    $('who').replaceChildren(el('div', null, T('scout.sel.title'), el('span', 'sub', role ? T('scout.sel.your', { role }) : T('scout.sel.noPos'))))
    const mine = s.champion || s.intent
    const buildPart = sel.build
      ? opggCard(sel.build)
      : [card('claude:og', mine ? T('scout.sel.hover', { champ: champName(mine) }) : T('scout.sel.lockIn'), 'wide')]
    $('profile').replaceChildren(el('div', 'sel-side', picksCard(s), metaCard(s)), el('div', 'sel-main', ...buildPart.filter(Boolean)))
  }

  function drawDetail() {
    const page = opened[opened.length - 1]
    $('detail').hidden = !page
    if (!page) return
    films.length = 0
    const pages = { player: () => playerPage(page.player), game: () => gamePage(page), season: seasonPage, role: () => rolePage(page), masteries: masteriesPage, champ: () => champPage(page), opgg: () => opggPage(page) }
    $('detail').replaceChildren(...pages[page.kind]())
  }

  $('close').addEventListener('click', () => window.pet.scoutClose())

  window.pet.onScout(payload => {
    const before = data
    data = payload || { state: 'none' }
    // Another game, or her own instead of the ten: what was open goes.
    if (!!before.profile !== !!data.profile || before.state !== data.state) {
      opened.length = 0
      drawDetail()
    }
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
