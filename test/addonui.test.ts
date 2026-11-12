

// Other people's plugins on the Plugins page and in the picker (addonui.js;
// settings.js and picker.js draw what it says): the row's words, the
// picker's kinds and order, the steps of installing from a file and the
// layer each shows, a folder that did not install; and their icons where
// widgets go (widgets.js, wcards.js).
import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as A from '../src/pet/addonui.ts'
import * as P from '../src/pet/placing.ts'
import * as Widgets from '../src/pet/widgets.ts'
import * as WCards from '../src/pet/wcards.ts'
import { t, list, STRINGS } from '../src/shared/i18n.ts'
import F from '../tools/preview/fixtures.js'
import * as wdeskModule from '../src/pet/wdesk.ts'

const zh = (key, vars?) => t('zh', key, vars)
const en = (key, vars?) => t('en', key, vars)
const andZh = names => list('zh', names, 'and')

const rain = {
  id: 'rain-alert',
  kind: 'plugin',
  name: '下雨提醒',
  author: 'lin',
  version: '1.2.0',
  category: 'info',
  perms: { net: ['api.open-meteo.com', 'geocoding-api.open-meteo.com'], agents: false, hardware: false, notify: true },
  reviewed: true,
  on: true,
  state: 'running',
  missing: null,
  said: null,
  settings: [{ key: 'City', type: 'text', label: '城市', required: true, value: 'Dresden' }],
}
const widget = { id: 'rain-alert', plugin: 'rain-alert', from: 'addon', label: '下雨提醒', value: '70%' }

test('a row says how it is, then who made it and which version (the spec’s 运行中 · @作者 · v版本)', () => {
  const at = (a, o) => A.statusOf({ ...rain, ...a }, { T: zh, ...o })
  assert.deepEqual(at({}, { placed: true, mine: [widget] }), { text: '运行中 · @lin · v1.2.0' })
  // Put nowhere: 没放 (placed is on: nowhere is off), whatever its state.
  assert.deepEqual(at({ on: false, state: 'off' }, { placed: false }), { text: '没放 · @lin · v1.2.0' })
  // Just put somewhere, not running yet: starting; running with nothing up yet, the same.
  assert.equal(at({ state: 'off' }, { placed: true }).text, '启动中… · @lin · v1.2.0')
  assert.equal(at({}, { placed: true, mine: [] }).text, '启动中… · @lin · v1.2.0')
  // A setting to fill in first, by its own label.
  assert.deepEqual(at({ state: 'needs', missing: 'City' }, { placed: true }), { text: '先填「城市」', bad: true })
  // What went wrong while it shows nothing, as hers say it; with widgets up, as hers, it runs.
  assert.deepEqual(at({ said: 'TypeError: x is undefined' }, { placed: true, mine: [] }), { text: '还没显示：TypeError: x is undefined', bad: true })
  assert.equal(at({ said: 'TypeError' }, { placed: true, mine: [widget] }).text, '运行中 · @lin · v1.2.0')
  // Not responding: turned off, with 重新启用; since when and how many
  // times, as the watchdog says (spec §3); too busy (§8); loading again
  // and again. Placed or not.
  assert.deepEqual(at({ state: 'stopped' }, { placed: true }), { text: '无响应，已停用 · @lin · v1.2.0', bad: true, revive: true })
  const since = new Date(2026, 9, 21, 10, 12).getTime()
  assert.equal(at({ state: 'stopped', stopped: { why: 'hung', since, at: since + 9000, times: 3 } }, { placed: true }).text, '无响应，已停用 · 10:12 起 3 次没回答 · @lin · v1.2.0')
  assert.equal(at({ state: 'stopped', stopped: { why: 'hung', since: new Date(2026, 9, 21, 9, 5).getTime(), times: 2 } }, { placed: false }).text, '无响应，已停用 · 09:05 起 2 次没回答 · @lin · v1.2.0')
  assert.deepEqual(at({ state: 'stopped', stopped: { why: 'busy', since, times: 0 } }, { placed: true }), { text: '太忙，已停用 · @lin · v1.2.0', bad: true, revive: true })
  assert.equal(at({ state: 'stopped', stopped: { why: 'crashed', since, times: 0 } }, { placed: true }).text, '反复重新载入，已停用 · @lin · v1.2.0')
  assert.equal(A.statusOf({ ...rain, state: 'stopped', stopped: { why: 'hung', since, times: 3 } }, { T: en }).text, 'Stopped responding, turned off · no answer 3 times since 10:12 · @lin · v1.2.0')
  assert.equal(A.statusOf({ ...rain, state: 'stopped', stopped: { why: 'busy', since, times: 0 } }, { T: en }).text, 'Too busy, turned off · @lin · v1.2.0')
  // Without the watchdog's say (an older one's): what it said.
  assert.equal(at({ state: 'stopped', said: '10:12 起 3 次没回答' }, { placed: true }).text, '无响应，已停用 · 10:12 起 3 次没回答 · @lin · v1.2.0')
  // A newer API than hers: it waits for a newer Wakuwaku, placed or not.
  assert.deepEqual(at({ state: 'unsupported' }, { placed: false }), { text: '需要新版 Wakuwaku · @lin · v1.2.0', bad: true })
  // No author: the version alone.
  assert.equal(A.statusOf({ ...rain, author: '' }, { placed: true, mine: [widget], T: en }).text, 'Running · v1.2.0')
})

test('the picker’s kinds for a manifest’s category: 系统, 效率, 信息', () => {
  assert.deepEqual(['system', 'productivity', 'info', undefined, 'games'].map(category => A.catOf({ category })), ['system', 'work', 'info', 'info', 'info'])
  assert.equal(zh(`picker.cat.${A.catOf({ category: 'productivity' })}`), '效率')
})

test('its page says who made it, its kind, and 自绘界面 for one with a view of its own', () => {
  assert.equal(A.aboutOf(rain, zh), '@lin · v1.2.0 · 信息')
  assert.equal(A.aboutOf({ ...rain, view: true, category: 'productivity' }, zh), '@lin · v1.2.0 · 效率 · 自绘界面')
  assert.equal(A.aboutOf({ ...rain, view: true }, en), '@lin · v1.2.0 · Info · Own view')
})

test('what it may do, a line each; the web is no question at install, the rest are', () => {
  assert.deepEqual(
    A.permLines(rain.perms, zh, andZh).map(p => [p.icon, p.text]),
    [
      ['net', '联网，只访问 api.open-meteo.com 和 geocoding-api.open-meteo.com'],
      ['notify', '提醒你，走岛上的提醒规则'],
    ],
  )
  assert.deepEqual(A.permLines({ agents: true, hardware: true }, en).map(p => p.icon), ['agents', 'hardware'])
  assert.deepEqual(A.permLines(null, zh), [])
  assert.deepEqual(A.permLines({ net: [] }, zh), [])
  assert.equal(A.isSensitive(rain.perms), true)
  assert.equal(A.isSensitive({ net: ['api.example.com'] }), false)
  assert.equal(A.isSensitive(undefined), false)
})

test('in the picker one just installed and put nowhere yet comes first, marked 新, until it is put somewhere', () => {
  const cards = ['widget:monitor', 'plugin:weather', 'plugin:rain-alert', 'plugin:tally', 'widget:deploy'].map(key => ({ key }))
  const news = A.makeNews()
  news.add('tally')
  const fresh = key => key.startsWith('plugin:') && news.has(key.slice(7))
  assert.deepEqual(A.freshFirst(cards, fresh).map(c => c.key), ['plugin:tally', 'widget:monitor', 'plugin:weather', 'plugin:rain-alert', 'widget:deploy'])
  assert.deepEqual(A.badgeOf(0, true, zh), { text: '新', fresh: true })
  assert.deepEqual(A.badgeOf(2, true, zh), { text: '已放 2', fresh: false }, 'placed: its count, not 新')
  assert.deepEqual(A.badgeOf(0, false, zh), { text: '', fresh: false })
  // Put somewhere once: new no more, back in its place.
  news.placed('tally')
  assert.deepEqual(A.freshFirst(cards, fresh).map(c => c.key), cards.map(c => c.key))
  assert.deepEqual(news.ids(), [])
  // Placing it turns it on as hers are (plugins[id].on), its settings kept.
  assert.deepEqual(P.switchesPatch({ plugins: { 'rain-alert': { City: 'Dresden' } } }, ['plugin:rain-alert'], true), { plugins: { 'rain-alert': { City: 'Dresden', on: true } } })
})

test('installing from a file: picked, looked at, asked about, installed; or why it cannot go in', () => {
  const step = (s, ...events) => events.reduce(A.next, s)
  const path = 'C:\\Users\\you\\Downloads\\agent-board.zip'
  const ok = { ok: true, file: 'agent-board.zip', id: 'agent-board', name: 'Agent 排行', version: '0.9.1', author: 'mika', kind: 'plugin', perms: { net: [], agents: true, hardware: false, notify: true }, sensitive: true, replaces: null, api: 1, supported: true, refused: null }
  // Dialog closed: nothing.
  assert.deepEqual(step(A.IDLE, { type: 'pick' }, { type: 'picked', path: null }), { step: 'idle' })
  // Looked at: asked about; 仍然安装: installing; in: idle, with what went in.
  const ask = step(A.IDLE, { type: 'pick' }, { type: 'picked', path }, { type: 'looked', path, summary: ok })
  assert.deepEqual(ask, { step: 'ask', path, summary: ok })
  const busy = A.next(ask, { type: 'go' })
  assert.equal(busy.step, 'installing')
  assert.deepEqual(A.next(busy, { type: 'installed', result: { ...ok } }), { step: 'idle', done: { id: 'agent-board', name: 'Agent 排行', kind: 'plugin' } })
  // 取消 there: nothing.
  assert.deepEqual(A.next(ask, { type: 'cancel' }), { step: 'idle' })
  // An answer about another file (a slow one before) changes nothing; nor do events out of turn.
  const looking = step(A.IDLE, { type: 'pick' }, { type: 'picked', path })
  assert.equal(A.next(looking, { type: 'looked', path: 'other.zip', summary: ok }), looking)
  assert.equal(A.next(A.IDLE, { type: 'go' }), A.IDLE)
  assert.equal(A.next(busy, { type: 'cancel' }), busy, 'not while it installs')
  // A program inside: 装不了, with the file it is.
  const exe = { ok: false, file: 'cool-tool.zip', refused: { reason: 'program', file: 'helper.exe', detail: 'a program' } }
  const no = step(A.IDLE, { type: 'pick' }, { type: 'picked', path: 'cool-tool.zip' }, { type: 'looked', path: 'cool-tool.zip', summary: exe })
  assert.deepEqual(no, { step: 'refused', file: 'cool-tool.zip', refused: exe.refused })
  assert.deepEqual(A.next(no, { type: 'cancel' }), { step: 'idle' })
  // A pack is asked about too; a kind she does not know is not; no answer at all.
  assert.equal(A.verdict('cat.zip', { ...ok, kind: 'pet' }).step, 'ask')
  assert.equal(A.verdict('beach.zip', { ...ok, kind: 'wallpaper' }).step, 'ask')
  assert.equal(A.verdict('x.zip', { ...ok, kind: 'theme' }).refused.reason, 'kind')
  assert.deepEqual(A.verdict('C:\\x\\broken.zip', null), { step: 'refused', file: 'broken.zip', refused: { reason: 'read', file: null, detail: '' } })
  // Refused as it was written in (it changed meanwhile).
  const late = A.next(busy, { type: 'installed', result: { ok: false, file: 'agent-board.zip', refused: { reason: 'write', file: null, detail: 'not put in place: denied' } } })
  assert.deepEqual([late.step, late.refused.reason], ['refused', 'write'])
})

test('the layers say what the spec says, in its words', () => {
  const ok = { ok: true, id: 'agent-board', name: 'Agent 排行', kind: 'plugin', perms: { net: [], agents: true, hardware: false, notify: true }, replaces: null, supported: true }
  const ask = A.layerOf({ step: 'ask', path: 'x.zip', summary: ok }, zh, andZh)
  assert.equal(ask.title, '安装「Agent 排行」？')
  assert.equal(`${ask.warn.head}${ask.warn.text}`, '未经审核：没人看过它的代码。它和市场里的插件一样跑在沙盒里，读不了你的文件，也运行不了程序。')
  assert.equal(ask.willHead, '它会：')
  assert.deepEqual(ask.will, ['读你的 agent 会话：在干活、等你批准、做完了，项目名', '提醒你，走岛上的提醒规则'])
  assert.deepEqual(ask.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['go', '仍然安装']])
  assert.equal(ask.busy, false)
  // The same id installed: an update.
  const update = A.layerOf({ step: 'ask', path: 'x.zip', summary: { ...ok, replaces: '1.2.0' } }, zh, andZh)
  assert.deepEqual(update.notes, ['替换已装的 1.2.0，设置和数据保留'])
  // Nothing asked for: it says it shows its own alone.
  assert.deepEqual(A.layerOf({ step: 'ask', summary: { ...ok, perms: {} } }, zh).will, ['只显示它自己的小工具，不联网'])
  // Installing: its buttons off.
  const busy = A.layerOf({ step: 'installing', path: 'x.zip', summary: ok }, zh)
  assert.deepEqual([busy.busy, busy.buttons[1][1]], [true, '正在安装…'])
  // A program inside.
  const no = A.layerOf({ step: 'refused', file: 'cool-tool.zip', refused: { reason: 'program', file: 'helper.exe', detail: 'a program' } }, zh)
  assert.equal(no.title, '装不了 cool-tool.zip')
  assert.equal(`${no.warn.head}${no.warn.text}`, '包里有可执行程序：helper.exe。Wakuwaku 的插件不能带程序。')
  assert.match(no.line.text, /外部程序/)
  assert.deepEqual(no.buttons.map(b => b.slice(0, 2)), [['cancel', '知道了']])
  // 卸载, asked first.
  const out = A.uninstallLayerOf(rain, zh)
  assert.equal(`${out.title}${out.notes[0]}`, '卸载「下雨提醒」？它存的数据也一起删掉。')
  assert.deepEqual(out.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['uninstall', '卸载']])
  assert.equal(A.layerOf(A.IDLE, zh), null)
  assert.equal(A.layerOf({ step: 'looking', path: 'x.zip' }, zh), null)
})

test('why a package will not go in, in words; its sizes read, a manifest’s fault as it was said', () => {
  const said = r => A.refusedWords(r, zh).text
  assert.equal(said({ reason: 'big', detail: 'over 5 MB' }), '太大了：超过 5 MB')
  assert.equal(said({ reason: 'big', detail: 'over 20 MB unpacked' }), '太大了：解开后超过 20 MB')
  assert.equal(said({ reason: 'big', detail: '?' }), '太大了')
  assert.equal(said({ reason: 'files', detail: '612 files, 500 at most' }), '文件太多：612 个，最多 500 个')
  assert.equal(said({ reason: 'type', file: 'helper.dll' }), 'helper.dll。插件里只能有网页文件、图片、字体和声音。')
  assert.equal(said({ reason: 'path', file: '../evil.js' }), '包里有个文件名指到了它的文件夹外面：../evil.js')
  assert.equal(said({ reason: 'manifest', detail: 'no manifest.json at its root' }), '最外层没有 manifest.json')
  assert.equal(said({ reason: 'manifest', file: 'manifest.json', detail: 'id: weather is one of Wakuwaku’s own' }), 'manifest.json 不对：id: weather is one of Wakuwaku’s own')
  assert.equal(said({ reason: 'zip', detail: 'not a zip' }), '不是 zip 文件，或者已经坏了')
  assert.equal(said({ reason: 'zip', detail: 'ZIP64' }), '是 ZIP64 格式的，她不收：请打成普通的 zip')
  assert.equal(said({ reason: 'zip', detail: 'encrypted' }), '里面有加了密的文件')
  assert.equal(said({ reason: 'read', detail: '' }), '读不了这个文件')
  assert.equal(A.refusedWords({ reason: 'program', file: 'helper.exe' }, en).head, "There's a program in it:")
  // Only a program, or a file of a kind it may not hold, points at programs of one's own.
  assert.match(A.layerOf({ step: 'refused', file: 'x.zip', refused: { reason: 'big', detail: 'over 5 MB' } }, zh).line.text, /开发文档/)
})

test('a folder that did not install: one quiet line, the reason in her words where she knows it', () => {
  assert.equal(A.brokenLine({ folder: 'foo', reason: 'no manifest.json' }, zh), 'plugins\\foo 没装上：没有 manifest.json')
  assert.equal(A.brokenLine({ folder: 'foo', reason: 'its folder is foo, its id bar' }, zh), 'plugins\\foo 没装上：文件夹叫 foo，它的 id 却是 bar')
  assert.equal(A.brokenLine({ folder: 'foo', reason: 'main.js: not in its folder' }, en), "plugins\\foo didn't install: main.js is missing")
  const long = A.brokenLine({ folder: 'foo', reason: `name: ${'x'.repeat(200)}` }, zh)
  assert.ok(long.endsWith('…') && [...long].length < 110, long)
  assert.equal(A.cut('一二三', 3), '一二三')
  assert.equal(A.cut('一二三四', 3), '一二…')
})

test('every word the Plugins page and the picker use for them is there, in both languages', () => {
  const keys = Object.keys(STRINGS.zh).filter(k => k.startsWith('addons.'))
  assert.ok(keys.length > 40, `${keys.length} keys`)
  for (const k of keys) assert.ok(STRINGS.en[k], k)
  // English as English, not the Chinese word for word: no CJK in it.
  for (const k of keys) assert.doesNotMatch(STRINGS.en[k], /[\u4e00-\u9fff]/, k)
})

test('a plugin’s own picture as its icon: drawn in one colour by a mask, only from her scheme', () => {
  const url = 'http://wakuplugin.localhost/hello/icons/sun #1.svg'
  assert.equal(Widgets.maskUrl(url), 'http://wakuplugin.localhost/hello/icons/sun%20%231.svg')
  for (const bad of ['https://example.com/x.svg', 'http://wakuplugin.localhost/hello/../x.svg', 'http://wakuplugin.localhost/Hello/x.svg', 'http://wakuplugin.localhost/hello/x.png', 'javascript:alert(1)', null]) assert.equal(Widgets.maskUrl(bad), '', String(bad))
  assert.ok(Widgets.maskUrl('data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg"/>').includes('%22'), 'quotes escaped')
  const w = { id: 'hello', plugin: 'hello', from: 'addon', icon: 'dot', iconUrl: 'http://wakuplugin.localhost/hello/icon.svg', label: '数数', value: '42', color: '#7fd8a6' }
  assert.match(Widgets.iconHTML(w), /class="wicon wmask" style="--mask:url\(&quot;http:\/\/wakuplugin\.localhost\/hello\/icon\.svg&quot;\)"/)
  assert.match(WCards.glyphHTML(w, 'twg'), /<span class="wsvg twg"><span class="wicon wmask"/)
  // Its own picture not to be had: its name's letter on its colour; one of hers: hers.
  assert.match(Widgets.iconHTML({ ...w, iconUrl: 'https://example.com/x.svg' }), /wmark[^>]*>数</)
  assert.match(WCards.glyphHTML({ ...w, iconUrl: undefined }, 'twg'), /wmark twg/)
  assert.match(WCards.glyphHTML({ ...w, iconUrl: undefined, icon: 'weather' }), /<svg class="wicon"/)
  // As generic cards, its sizes as its manifest gave them (addons.rs: detail.sizes).
  assert.equal(WCards.kindOf(w), 'generic')
  assert.deepEqual(Widgets.sizesOf({ ...w, detail: { sizes: ['s'] } }), ['s'])
})

test('a plugin’s name where it is put: hers by her words, someone else’s as its widgets say it, a private one’s never', () => {
  const all = [w('rain-alert', '下雨提醒'), { ...w('secret', 'Inbox of Ann'), private: true }]
  function w(plugin, label) {
    return { id: plugin, plugin, from: 'addon', label, value: '1' }
  }
  assert.equal(Widgets.pluginName('zh', 'weather', all), '天气')
  assert.equal(Widgets.pluginName('zh', 'rain-alert', all), '下雨提醒')
  assert.equal(Widgets.pluginName('zh', 'secret', all), '')
  assert.equal(Widgets.nameOf('zh', { type: 'plugin', of: 'rain-alert' }, all), '下雨提醒')
  assert.equal(Widgets.nameOf('zh', { type: 'plugin', of: 'gone', name: '数数' }, []), '数数')
  assert.equal(Widgets.nameOf('en', { type: 'plugin', of: 'gone' }, []), 'Widget')
})

// --- The plugin market (market.rs; addonui.js the words and steps; settings.js draws them) ---
const item = id => F.MARKET.find(i => i.id === id)

test('the market opens by a link, on a category: the picker’s 获取更多… on its widgets', () => {
  assert.deepEqual(A.marketLink('widgets:market'), { cat: 'all' })
  assert.deepEqual(A.marketLink('widgets:market:widget'), { cat: 'widget' })
  assert.deepEqual(A.marketLink('widgets:market:pet'), { cat: 'pet' })
  assert.deepEqual(A.marketLink('widgets:market:wallpaper'), { cat: 'wallpaper' })
  for (const other of ['widgets', 'widgets:rain-alert', 'widgets:market:games', 'market', '', null, undefined]) assert.equal(A.marketLink(other), null, String(other))
})

test('its list: a category, and words in either language, the author or the id', () => {
  const ids = o => A.marketList(F.MARKET, o).map(i => i.id)
  assert.equal(ids({}).length, 7)
  assert.deepEqual(ids({ cat: 'widget' }), ['agent-board', 'fx', 'gh-notify', 'rain-alert'])
  assert.deepEqual(ids({ cat: 'pet' }), ['pet-pixelcat', 'pet-shiba'])
  assert.deepEqual(ids({ cat: 'wallpaper' }), ['wall-aurora'])
  assert.deepEqual(ids({ query: '下雨' }), ['rain-alert'])
  assert.deepEqual(ids({ query: 'RAIN' }), ['rain-alert'])
  assert.deepEqual(ids({ query: 'mika' }), ['agent-board'])
  assert.deepEqual(ids({ query: '猫', cat: 'widget' }), [])
  assert.deepEqual(A.marketList(null, {}), [])
})

test('each row says its kind, 自绘界面 and what it is; its page who made it, its size and kind', () => {
  assert.equal(A.marketAbout(item('agent-board'), zh, 'zh'), '小工具 · 自绘界面 · 今天哪个项目让 agent 干得最多')
  assert.equal(A.marketAbout(item('pet-shiba'), en, 'en'), 'Pet · A shiba that wags, nine moves')
  assert.equal(A.marketByline(item('fx'), zh), '@wen · v1.4.0 · 36 KB · 小工具')
  assert.equal(A.marketByline(item('wall-aurora'), zh), '@sora · v1.0.0 · 18.0 MB · 壁纸')
  assert.equal(A.wordsIn({ en: 'Rain', zh: '雨' }, 'zh'), '雨')
  assert.equal(A.wordsIn({ en: 'Rain' }, 'zh'), 'Rain')
  assert.equal(A.wordsIn('Rain', 'zh'), 'Rain')
  assert.equal(A.wordsIn(null, 'zh'), '')
})

test('the button: 安装, 已安装 (its page: 添加小工具… or the Look page), 更新 for one older here, a download’s share', () => {
  const b = (id, o = {}) => A.marketButton(item(id), { T: zh, ...o })
  assert.deepEqual([b('fx').what, b('fx').label, b('fx').primary], ['install', '安装', true])
  const here = b('rain-alert', { installed: '1.2.0' })
  assert.deepEqual([here.what, here.label, here.disabled], ['installed', '已安装', true])
  const page = b('rain-alert', { installed: '1.2.0', detail: true })
  assert.deepEqual([page.what, page.label], ['add', '添加小工具…'])
  const pack = b('pet-shiba', { installed: '1.0.0', detail: true })
  assert.deepEqual([pack.what, pack.label], ['look', '去「外观」里选'])
  // Older here: 更新, by number (1.10.0 after 1.9.0), in the list and on its page.
  assert.deepEqual([b('rain-alert', { installed: '1.1.9' }).what, b('rain-alert', { installed: '1.1.9', detail: true }).label], ['update', '更新'])
  assert.equal(A.isNewer('1.10.0', '1.9.0'), true)
  assert.equal(A.isNewer('1.2.0', '1.2.0'), false)
  assert.equal(A.isNewer('1.2.0', '1.10.0'), false)
  // Under way: its share as it downloads, then a word; done or failed, the button again.
  const busy = job => b('fx', { job })
  assert.deepEqual(busy({ stage: 'download', got: 16588, total: 36864 }), { what: 'busy', label: '44%', pct: 44, primary: false, disabled: true })
  assert.equal(busy({ stage: 'verify', got: 1, total: 1 }).label, '校验中…')
  assert.equal(busy({ stage: 'install', got: 1, total: 1 }).label, '安装中…')
  assert.equal(busy({ stage: 'failed' }).what, 'install')
  assert.equal(b('fx', { job: { stage: 'done' }, installed: '1.4.0' }).what, 'installed')
})

test('installing one that asks for more than the web is asked first, in the spec’s words; the web alone goes straight in', () => {
  const ask = A.marketAskOf(item('gh-notify'), zh, andZh, 'zh')
  assert.equal(ask.title, '安装「GitHub 通知」？')
  assert.equal(ask.willHead, '它会：')
  assert.deepEqual(ask.will, ['联网，只访问 api.github.com', '提醒你，走岛上的提醒规则'])
  assert.deepEqual(ask.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['mkgo', '安装']])
  assert.equal(A.marketAskOf(item('fx'), zh, andZh, 'zh'), null)
  assert.equal(A.marketAskOf(item('agent-board'), en, undefined, 'en').title, 'Install “Agent board”?')
  // 更新 for one here: asked only when the new one asks for more, that alone.
  const had = { net: ['api.open-meteo.com', 'geocoding-api.open-meteo.com'], agents: false, hardware: false, notify: true }
  assert.equal(A.marketAskOf(item('rain-alert'), zh, andZh, 'zh', had), null)
  const more = A.marketAskOf({ ...item('rain-alert'), perms: { ...had, net: [...had.net, 'api.rainviewer.com'], agents: true } }, zh, andZh, 'zh', had)
  assert.equal(more.title, '更新「下雨提醒」？')
  assert.equal(more.willHead, '新版还要：')
  assert.deepEqual(more.will, ['联网，只访问 api.rainviewer.com', '读你的 agent 会话：在干活、等你批准、做完了，项目名'])
  assert.deepEqual(more.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['mkgo', '更新']])
  assert.deepEqual(A.permsBeyond({ net: ['*.github.io'] }, { net: ['lin.github.io', 'github.io', '*.a.github.io'], notify: true }), { net: ['github.io'], agents: false, hardware: false, notify: true })
  // Installed: where to put it; a pack, the Look page.
  assert.equal(A.marketDoneOf(item('fx'), zh, 'zh'), '装好了「汇率」· 用「添加小工具…」放到哪里')
  assert.equal(A.marketDoneOf(item('pet-shiba'), zh, 'zh'), '装好了「柴犬小八」· 在「外观」里选')
})

test('updates waiting: 「N 个插件有更新 · 全部更新」, asked first with what each newly asks for', () => {
  assert.deepEqual(A.updatesBanner(F.MARKET_UPDATES, zh), { text: '2 个插件有更新', button: '全部更新' })
  assert.equal(A.updatesBanner([], zh), null)
  assert.equal(A.updatesBanner(undefined, zh), null)
  assert.equal(A.updatesBanner(F.MARKET_UPDATES, en).text, 'Plugin updates: 2')
  const ask = A.updatesAskOf(F.MARKET_UPDATES, zh, andZh)
  assert.equal(ask.title, '全部更新？')
  assert.equal(ask.willHead, '这些新版要更多权限：')
  assert.deepEqual(ask.will, ['「下雨提醒」1.2.0 → 1.3.0：联网，只访问 api.rainviewer.com'])
  assert.deepEqual(ask.notes, ['另外 1 个只是更新，权限不变。'])
  assert.deepEqual(ask.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['mkall', '全部更新']])
  // Several new at once, in a line; none asking for more: not asked.
  const more = A.updatesAskOf([{ id: 'x', name: 'X', from: '1.0.0', to: '2.0.0', newPerms: ['agents', 'net:a.example.com', 'notify'] }], zh, andZh)
  assert.equal(more.will[0], '「X」1.0.0 → 2.0.0：联网，只访问 a.example.com；读你的 agent 会话：在干活、等你批准、做完了，项目名；提醒你，走岛上的提醒规则')
  assert.deepEqual(more.notes, [])
  assert.equal(A.updatesAskOf([{ id: 'y', name: 'Y', from: '1.0.0', to: '1.0.1', newPerms: [] }], zh), null)
  assert.deepEqual(A.permsOfNew(['net:a.example.com', 'hardware']), { net: ['a.example.com'], agents: false, hardware: true, notify: false })
})

test('why the market does not show: not open yet is no error; not reached says the way and the proxy; and why one did not go in', () => {
  assert.deepEqual(A.marketTroubleOf({ code: 'closed', detail: 'GitHub' }, zh), { closed: true, text: '市场还没开张', note: '放插件的仓库还在建，过些日子再来看看。', proxy: false })
  const far = A.marketTroubleOf({ code: 'failed', detail: 'GitHub: straight, github.com: timeout: connect' }, zh)
  assert.equal(far.text, '市场连不上：GitHub: straight, github.com: timeout: connect')
  assert.equal(far.proxy, true)
  assert.match(far.note, /网络代理/)
  assert.match(A.marketTroubleOf({ code: 'bad', detail: 'no items' }, en).text, /index doesn't read: no items/)
  assert.equal(A.marketTroubleOf(null, zh), null)
  const fail = e => A.marketFailOf(e, zh)
  assert.equal(fail({ code: 'refused', refused: { reason: 'packType', file: 'walk.js' } }), '装不了：包里有资源包不能带的文件：walk.js。资源包只有图片和说明文字，没有代码。')
  assert.equal(fail({ code: 'digest', detail: 'x' }), '下载的文件和目录给的校验值不一致，已放弃，没装')
  assert.equal(fail({ code: 'size', detail: 'x' }), '下载的文件和目录里写的大小不一样，已放弃，没装')
  assert.equal(fail({ code: 'download', detail: 'GitHub: straight, github.com: timeout' }), '下载失败：GitHub: straight, github.com: timeout')
  assert.equal(fail({ code: 'refused', refused: { reason: 'program', file: 'helper.exe' } }), '装不了：包里有可执行程序：helper.exe。Wakuwaku 的插件不能带程序。')
  assert.equal(fail({ code: 'mismatch', detail: 'x 1.0.0 in place of y 1.0.0' }), '下载的包和目录里写的不是同一个（x 1.0.0 in place of y 1.0.0），没装')
  assert.equal(fail({ code: 'cancelled' }), '')
  assert.equal(fail({ code: 'what', detail: 'oops' }), '没装上：oops')
})

test('its cards drawn from what its widget would say, as her pages take a plugin’s widget', () => {
  const w = A.sampleWidget(item('fx'), 'zh')
  assert.deepEqual(w, { id: 'fx', plugin: 'fx', from: 'addon', on: true, label: 'EUR/CNY', value: '7.81', icon: 'coin', color: '#ffd36b', private: false, detail: { extra: '+0.4% · 一周', series: { points: [7.72, 7.74, 7.73, 7.77, 7.79, 7.78, 7.81] }, sizes: ['s', 'm'] } })
  // Its own .svg as its picture; no label: its name; its sizes the manifest's.
  const gh = A.sampleWidget({ ...item('gh-notify'), sample: { value: '12' } }, 'en')
  assert.deepEqual([gh.label, gh.icon, gh.iconUrl.startsWith('data:image/svg+xml')], ['GitHub notifications', 'dot', true])
  assert.deepEqual(A.sampleSizes(item('agent-board')), ['s', 'm', 'l'])
  assert.deepEqual(A.sampleSizes({ sizes: ['l', 's'] }), ['s', 'l'])
  assert.deepEqual(A.sampleSizes({}), ['s', 'm'])
  assert.equal(A.sampleWidget(item('pet-shiba'), 'zh'), null)
  // As generic cards, drawn by the desktop's templates.
  const WDesk = wdeskModule
  for (const size of ['s', 'm']) assert.match(WDesk.faceHTML(w, size, { lang: 'zh', detailed: true, all: [w] }), /7\.81/)
})
