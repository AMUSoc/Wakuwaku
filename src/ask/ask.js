// Asking her (ask.rs): a box, the agent to ask (Claude, with the web;
// Antigravity, with Google's search), the questions and answers so far
// (their Markdown drawn), "正在想…" while one is out. Enter sends, Shift+Enter
// is a new line, Esc puts it away.
;(function () {
  const { t } = window.I18n
  let data = { turns: [], agent: 'claude' }
  let agent = 'claude'
  let timer = 0
  let drawnKey = ''
  const T = (key, vars) => t(data.lang || 'zh-TW', key, vars)
  const $ = id => document.getElementById(id)
  const NAMES = { claude: 'Claude', agy: 'Antigravity' }

  const esc = s => String(s).replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])

  // Markdown, enough of it: code blocks, headings, lists, a table as rows,
  // bold, inline code, links (opened by the system's browser: none here).
  function md(text) {
    const inline = s =>
      esc(s)
        .replace(/`([^`]+)`/g, '<code>$1</code>')
        .replace(/\*\*(.+?)\*\*/g, '<b>$1</b>')
        .replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<span class="link" title="$2">$1</span>')
    const out = []
    let list = null
    const lines = String(text).replace(/\r/g, '').split('\n')
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i]
      const s = line.trim()
      if (s.startsWith('```')) {
        const code = []
        while (++i < lines.length && !lines[i].trim().startsWith('```')) code.push(lines[i])
        out.push(`<pre>${esc(code.join('\n'))}</pre>`)
        list = null
        continue
      }
      const item = /^\s*([-*+]|\d+[.)])\s+(.*)$/.exec(line)
      if (item) {
        const mark = /\d/.test(item[1]) ? item[1] : '•'
        out.push(`<div class="li"><span class="mk">${esc(mark)}</span><span>${inline(item[2])}</span></div>`)
        list = true
        continue
      }
      list = null
      if (!s) out.push('<div class="gap"></div>')
      else if (/^#{1,6}\s/.test(s)) out.push(`<div class="h">${inline(s.replace(/^#+\s*/, ''))}</div>`)
      else if (/^\|?\s*:?-{2,}/.test(s) && s.includes('-|') ) continue
      else if (s.startsWith('|')) out.push(`<div class="row">${s.replace(/^\||\|$/g, '').split('|').map(c => inline(c.trim())).filter(Boolean).join('<i> · </i>')}</div>`)
      else if (/^(-{3,}|\*{3,})$/.test(s)) continue
      else if (s.startsWith('>')) out.push(`<div class="q">${inline(s.replace(/^>\s*/, ''))}</div>`)
      else out.push(`<p>${inline(s)}</p>`)
    }
    return out.join('')
  }

  function drawWho() {
    $('who').innerHTML = Object.keys(NAMES)
      .map(a => `<button type="button" data-agent="${a}" class="${a === agent ? 'on' : ''}">${esc(NAMES[a])}</button>`)
      .join('')
  }

  function draw() {
    clearInterval(timer)
    $('title').textContent = T('ask.title')
    $('clear').textContent = T('ask.new')
    $('send').textContent = T('ask.send')
    $('box').placeholder = T('ask.placeholder', { name: NAMES[agent] })
    $('hint').textContent = T('ask.hint', { key: data.key || 'Alt+Q' })
    drawWho()
    const turns = data.turns || []
    const p = data.pending
    const key = JSON.stringify([turns.length, turns.at(-1)?.a?.length, !!p, data.error])
    const talk = $('talk')
    if (key !== drawnKey) {
      drawnKey = key
      const parts = turns.map(tn => `<div class="me">${esc(tn.q)}</div><div class="who">${esc(NAMES[tn.agent] || tn.agent)}</div><div class="them">${md(tn.a)}</div>`)
      if (p) parts.push(`<div class="me">${esc(p.q)}</div><div class="who">${esc(NAMES[p.agent] || p.agent)}</div><div class="them thinking"><span class="dots"><i></i><i></i><i></i></span> <span id="secs"></span></div>`)
      if (data.error) parts.push(`<div class="err">${esc(T('ask.failed', { why: data.error }))}</div>`)
      if (!parts.length) parts.push(`<div class="empty">${esc(T('ask.empty'))}</div>`)
      talk.innerHTML = parts.join('')
      talk.scrollTop = talk.scrollHeight
    }
    if (p) {
      const tick = () => {
        const s = document.getElementById('secs')
        if (s) s.textContent = T('ask.thinking', { name: NAMES[p.agent] || p.agent, s: Math.max(0, Math.round((Date.now() - p.since) / 1000)) })
      }
      tick()
      timer = setInterval(tick, 1000)
    }
    $('send').disabled = !!p
    $('clear').disabled = !!p || !turns.length
  }

  function sendIt() {
    const text = $('box').value.trim()
    if (!text || data.pending) return
    window.pet.askSend(agent, text).then(ok => {
      if (ok) $('box').value = ''
    })
  }

  $('who').addEventListener('click', e => {
    const b = e.target.closest('[data-agent]')
    if (!b) return
    agent = b.dataset.agent
    draw()
    $('box').focus()
  })
  $('send').addEventListener('click', sendIt)
  $('clear').addEventListener('click', () => window.pet.askClear())
  $('close').addEventListener('click', () => window.pet.askClose())
  $('box').addEventListener('keydown', e => {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault()
      sendIt()
    }
  })
  document.addEventListener('keydown', e => {
    if (e.key === 'Escape') window.pet.askClose()
  })

  window.pet.onAsk(payload => {
    const wasShown = data.shown
    data = payload || data
    if (!wasShown || !agent) agent = data.agent || 'claude'
    document.documentElement.lang = data.lang || 'zh-TW'
    draw()
    if (data.shown && !wasShown) setTimeout(() => $('box').focus(), 50)
  })
})()
