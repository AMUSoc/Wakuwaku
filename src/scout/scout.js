// The game's ten (scout.rs): each player's champion, rank and this season's
// wins, their mastery of the champion, and how their last games went, the
// two teams side by side. Its window lets every click through.
;(function () {
  const { t } = window.I18n
  let data = { state: 'none' }
  const T = (key, vars) => t(data.lang || 'zh-TW', key, vars)
  const $ = id => document.getElementById(id)

  function el(tag, cls, ...kids) {
    const node = document.createElement(tag)
    if (cls) node.className = cls
    for (const kid of kids.flat()) if (kid != null && kid !== false && kid !== '') node.append(kid)
    return node
  }

  const pct = (w, n) => (n ? Math.round((w / n) * 100) : 0)

  // 123456 points: 12.3萬, or 123.5k.
  function points(n) {
    if (!n) return '0'
    if ((data.lang || '').startsWith('zh')) return n >= 10000 ? `${(n / 10000).toFixed(1)}${data.lang === 'zh' ? '万' : '萬'}` : String(n)
    return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : String(n)
  }

  function rankText(r) {
    if (!r) return el('span', 'wait', '…')
    if (r.unranked) return el('span', null, T('lol.unranked'))
    const tier = T(`lol.tier.${r.tier}`)
    const div = ['MASTER', 'GRANDMASTER', 'CHALLENGER'].includes(r.tier) ? '' : ` ${r.rank}`
    const n = r.wins + r.losses
    const rate = pct(r.wins, n)
    return el('span', null, `${tier}${div} ${r.lp} LP · `, el('span', rate >= 50 ? 'good' : 'bad', T('scout.season', { wins: r.wins, losses: r.losses, rate })))
  }

  function playerView(p) {
    const icon = el('div', 'icon')
    if (p.championId) icon.style.backgroundImage = `url("${ICON(p.championId)}")`
    const r = p.recent
    const m = p.mastery
    const recent = []
    if (r) {
      recent.push(el('span', pct(r.wins, r.games) >= 50 ? 'good' : 'bad', T('scout.recent', { games: r.games, wins: r.wins })))
      recent.push(el('span', null, T('scout.kda', { k: r.k, d: r.d, a: r.a })))
      if (r.champGames) recent.push(el('span', null, T('scout.champ', { games: r.champGames, wins: r.champWins })))
      if (r.streak >= 3) recent.push(el('span', 'good', T('scout.winStreak', { n: r.streak })))
      if (r.streak <= -3) recent.push(el('span', 'bad', T('scout.loseStreak', { n: -r.streak })))
    }
    // No champion (her own, out of a game): no mastery to tell.
    const mastery = !p.championId ? null : m ? (m.level ? el('span', null, T('scout.mastery', { level: m.level, points: points(m.points) })) : el('span', 'bad', T('scout.firstTime'))) : el('span', 'wait', '…')
    return el(
      'div',
      `player${p.me ? ' me' : ''}${p.championId ? '' : ' no-champ'}`,
      p.championId ? icon : null,
      el(
        'div',
        null,
        el('div', 'top', el('span', 'champ', p.champion || ''), el('span', 'name', p.me ? `${p.name} · ${T('scout.me')}` : p.name), el('span', 'rank', rankText(p.rank))),
        mastery ? el('div', 'line', mastery) : null,
        el('div', 'line', recent.length ? recent : el('span', 'wait', '…')),
      ),
    )
  }

  const ICON = id => `https://raw.communitydragon.org/latest/plugins/rcp-be-lol-game-data/global/default/v1/champion-icons/${id}.png`

  // Her alone: her last games one by one, the latest first.
  function gamesView(list) {
    return el(
      'div',
      'games',
      (list || []).map(g => {
        const icon = el('div', 'mini')
        icon.style.backgroundImage = `url("${ICON(g.champion)}")`
        return el('div', `game ${g.win ? 'won' : 'lost'}`, icon, el('span', 'result', T(g.win ? 'scout.won' : 'scout.lost')), el('span', 'who', (data.names || {})[g.champion] || ''), el('span', 'kda', `${g.k}/${g.d}/${g.a}`))
      }),
    )
  }

  function draw() {
    $('panel').hidden = false
    const alone = data.state === 'ready' && data.solo
    $('title').textContent = T(alone ? 'scout.mine' : 'scout.title')
    $('hint').textContent = T('scout.hint', { key: data.key || 'Alt+X' })
    $('us').textContent = T('scout.us')
    $('them').textContent = T('scout.them')
    const ready = data.state === 'ready'
    // Her alone: why, and no other team.
    const solo = ready && data.solo
    $('say').textContent = !ready ? T(`scout.${data.state || 'none'}`) : solo ? T(data.solo === 'nogame' ? 'scout.soloNogame' : 'scout.soloOutside') : ''
    document.querySelector('.them').hidden = !!solo
    $('teams').classList.toggle('alone', !!solo)
    $('progress').textContent = ready && data.total > 1 && data.done < data.total ? T('scout.reading', { done: data.done, total: data.total }) : ''
    $('us').hidden = !!solo
    const mine = solo ? data.us.find(p => p.me) : null
    document.querySelector('.us .rows').replaceChildren(...(ready ? data.us.map(playerView) : []), ...(mine && mine.recent ? [gamesView(mine.recent.list)] : []))
    document.querySelector('.them .rows').replaceChildren(...(ready ? data.them.map(playerView) : []))
    $('teams').hidden = !ready
  }

  // A click anywhere on it puts it away.
  $('panel').addEventListener('click', () => window.pet.scoutClose())

  window.pet.onScout(payload => {
    data = payload || { state: 'none' }
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
  })
})()
