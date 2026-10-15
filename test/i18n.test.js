const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const { test } = require('node:test')

const { STRINGS, t, render, detectLang, list } = require('../src/shared/i18n')

test('both Chinese and English have the same keys, and the same {placeholders} and markup', () => {
  const en = Object.keys(STRINGS.en).sort()
  const holes = s => (s.match(/\{\w+\}/g) || []).sort().join(',')
  const tags = s => (s.match(/<[^>]+>/g) || []).join('')
  for (const lang of ['zh', 'zh-TW']) {
    assert.deepEqual(Object.keys(STRINGS[lang]).sort(), en, lang)
    for (const key of en) {
      assert.equal(holes(STRINGS[lang][key]), holes(STRINGS.en[key]), `${lang} ${key}`)
      assert.equal(tags(STRINGS[lang][key]), tags(STRINGS.zh[key]), `${lang} ${key}`)
    }
  }
})

// Characters only Simplified Chinese writes: none in the Traditional words
// (docs/zh-TW-glossary.md), nor its mainland words written in Traditional
// characters.
test("Traditional Chinese is Traditional, in Taiwan's words", () => {
  const simplified =
    '们这说时会对为过还没发现动设运网络内软点击认标灵宠话开关后来让请错读写显张长门问题应务机项实际与从两经样种换选择装载头边见觉览视转输钟钮键图数据报节级统计订闭页签复销卸获链户号码帮档讯个盖额账录创删将双仅优传则调缓夹质达递云杂乱线纸终结给绝细约红纯续总态声卫广库归当扫护担拥损无旧晓暂术杀权条极构栏树检楼欢气汇测济浏渐温灭灯热环电画监础确离积称稳笔签简紧纪纳练组织绕绘统继维综编职联脑脚艺药虑虽补规触训议记讲许论访证评识诉词译试该详语误谁课谢谈负责败货购贴费资赶跃踪车轮轻较辑违连迟适选逻遗邮释针钱铁银锁闲间闻阅队阶陆陈随隐难顺须预领频风飞馆马验鱼鸟齐么尽办启备'
  const mainland = ['信息', '文件', '程序', '插件', '批准', '屏幕', '鼠標', '默認', '會話', '設置', '運行', '網絡', '內存', '軟件', '點擊', '視窗口', '崩潰', '加載', '用戶', '支持']
  for (const [key, text] of Object.entries(STRINGS['zh-TW'])) {
    for (const ch of text) assert.ok(!simplified.includes(ch), `${key}: ${ch} in ${text}`)
    for (const word of mainland) assert.ok(!text.includes(word), `${key}: ${word} in ${text}`)
  }
})

test('every key the code uses is there', () => {
  // Keys named in the source as 'xxx.yyy' after t( / T( / { key: , or built
  // from a mood / menu / where name.
  const root = path.join(__dirname, '..', 'src')
  const files = []
  const walk = dir => {
    for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, d.name)
      if (d.isDirectory()) walk(p)
      else if (p.endsWith('.js') && !p.endsWith('i18n.js')) files.push(p)
    }
  }
  walk(root)
  const used = new Set()
  for (const file of files) {
    const text = fs.readFileSync(file, 'utf8')
    for (const m of text.matchAll(/(?:\bt\(\s*\w+,\s*|\bT\(\s*|key:\s*)'([a-z]+\.[A-Za-z]+)'/g)) used.add(m[1])
  }
  for (const mood of ['idle', 'working', 'waiting', 'done', 'review', 'error']) used.add(`mood.${mood}`)
  for (const mood of ['waiting', 'done', 'review', 'error']) used.add(`notify.${mood}`)
  for (const size of ['small', 'medium', 'large']) used.add(`menu.${size}`)
  for (const where of ['session', 'localSettings', 'projectSettings', 'userSettings']) used.add(`where.${where}`)

  assert.ok(used.size > 60, `found only ${used.size} keys: the scan is broken`)
  for (const lang of ['zh', 'zh-TW', 'en']) for (const key of used) assert.ok(key in STRINGS[lang], `missing in ${lang}: ${key}`)
})

test('t fills placeholders and falls back to English, then the key', () => {
  assert.equal(t('zh', 'panel.needsApproval', { tool: 'Bash' }), 'Bash 需要你批准')
  assert.equal(t('en', 'panel.needsApproval', { tool: 'Bash' }), 'Bash needs your approval')
  assert.equal(t('fr', 'panel.allow'), 'Allow')
  assert.equal(t('zh', 'no.such.key'), 'no.such.key')
  assert.equal(t('zh-TW', 'panel.needsApproval', { tool: 'Bash' }), 'Bash 需要你核准')
  assert.equal(t('en', 'panel.more'), '{n} more')
})

test('render takes plain text or a key', () => {
  assert.equal(render('zh', 'Bash'), 'Bash')
  assert.equal(render('en', { key: 'say.taskDone', vars: { task: 'tests' } }), 'Done: tests')
  assert.equal(render('en', ''), '')
  assert.equal(render('en', null), '')
})

test('Traditional Chinese by zh-TW or zh-Hant; any other Chinese locale Chinese; anything else English', () => {
  for (const l of ['zh-TW', 'zh-Hant', 'zh-Hant-TW', 'zh_TW.UTF-8', 'ZH-tw']) assert.equal(detectLang(l), 'zh-TW', l)
  for (const l of ['zh', 'zh-CN', 'zh_CN.UTF-8', 'zh-Hans-CN']) assert.equal(detectLang(l), 'zh', l)
  for (const l of ['en', 'en-US', 'ja-JP', 'C.UTF-8', '', undefined]) assert.equal(detectLang(l), 'en', String(l))
})

test('names in a row as one says them', () => {
  assert.equal(list('zh', ['Claude Code', 'Codex', 'Antigravity']), 'Claude Code、Codex 或 Antigravity')
  assert.equal(list('zh', ['Claude Code', 'Antigravity'], 'and'), 'Claude Code 和 Antigravity')
  assert.equal(list('en', ['Claude Code', 'Codex', 'Antigravity'], 'and'), 'Claude Code, Codex and Antigravity')
  assert.equal(list('en', ['Codex', 'Antigravity']), 'Codex or Antigravity')
  assert.equal(list('en', ['Codex']), 'Codex')
  assert.equal(list('zh', []), '')
  assert.equal(list('zh-TW', ['Claude Code', 'Codex', 'Grok'], 'and'), 'Claude Code、Codex 和 Grok')
})
