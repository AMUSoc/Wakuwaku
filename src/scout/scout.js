// The game's ten (scout.rs), or her own games where they cannot be found:
// a war room the four agents' pets keep. Her own: Claude tells her ranks,
// Grok her tags and the champions she plays most, Antigravity where she
// plays, Codex her last games; each part in its agent's colour, its pet
// beside a line of what it found. A click anywhere puts it away.
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
  function card(agent, said, cls, ...body) {
    const pet = (data.pets || {})[agent] || {}
    const node = el('section', `card ${cls || ''}`, el('div', 'host', head(agent), el('div', 'speech', el('b', null, T(`scout.part.${agent}`)), said)), ...body)
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
    return card('claude', said, '', main, flex)
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
    return card('grok', said, '', tags, mains)
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
      return el('div', 'role', el('span', null, T(`scout.role.${r.pos}`)), track, el('span', 'n', String(r.games)), el('span', `rate ${rate >= 50 ? 'good' : 'bad'}`, `${rate}%`))
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
    return el(
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
    return node
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
    $('hint').textContent = T('scout.hint', { key: data.key || 'Alt+X' })
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
  }

  // A click anywhere on it puts it away.
  $('panel').addEventListener('click', () => window.pet.scoutClose())

  window.pet.onScout(payload => {
    data = payload || { state: 'none' }
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
