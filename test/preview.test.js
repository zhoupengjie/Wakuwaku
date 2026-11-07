// The preview tools (tools/preview): the scenes the playground and the
// gallery share, which code a build says it is, what a changed file means to
// the live playground, and how compare.js reads and compares pictures.
const assert = require('node:assert/strict')
const { test } = require('node:test')
const path = require('node:path')
const zlib = require('node:zlib')
const { execFileSync } = require('node:child_process')

const F = require('../tools/preview/fixtures')
const Scenes = require('../tools/preview/scenes')
const gitinfo = require('../tools/preview/gitinfo')
const publish = require('../tools/preview/publish')
const compare = require('../tools/preview/compare')

const REPO = path.join(__dirname, '..')

// The playground's one-click scenes, as agreed (2026-10-17; the hold and the
// island at a corner, zhoupengjie/Wakuwaku#59 and #65, 2026-10-18), in order.
const WANTED = ['新用户第一次打开', '平常', '三个会话在忙', '等你批准', '做完了（流光）', '出错了', '提醒停留', '展开停留', '放满小工具的任务栏', '桌面', '日历和正在播放', '空桌面', '灵动岛模式', '岛在右上', '浅色 + 透明任务栏', '编辑任务栏布局中', '添加小工具', '编辑胶囊', '组合', 'Ctrl 多选']

test('the playground has each agreed scene, in order, with a note on what to try', () => {
  assert.deepEqual(
    Scenes.SCENES.map(s => s.title),
    WANTED,
  )
  const ids = Scenes.SCENES.map(s => s.id)
  assert.equal(new Set(ids).size, ids.length, 'ids unique')
  for (const s of Scenes.SCENES) {
    assert.match(s.id, /^[a-z0-9-]+$/, `${s.id}: a plain #token`)
    assert.ok(s.note && s.note.length > 10, `${s.id}: a note`)
  }
})

test('every scene sets the whole world, and it holds together', () => {
  const MOODS = ['idle', 'working', 'waiting', 'done', 'review', 'error']
  for (const scene of Scenes.SCENES) {
    const w = Scenes.sceneWorld(scene, F)
    const at = `${scene.id}`
    for (const key of [...Scenes.WORLD_KEYS, 'home']) assert.ok(key in w && w[key] !== undefined, `${at}: ${key}`)
    for (const key of ['sessions', 'asks', 'widgets', 'plugins', 'windows', 'tray', 'deskLayout']) assert.ok(Array.isArray(w[key]), `${at}: ${key} a list`)
    assert.ok(['taskbar', 'island'].includes(w.home), `${at}: home`)
    assert.ok(['bottom', 'top', 'left', 'right'].includes(w.edge), `${at}: edge`)
    assert.ok(['mica', 'clear', 'black'].includes(w.material), `${at}: material`)
    if ('mode' in w) assert.ok(['dark', 'light'].includes(w.mode), `${at}: mode`)
    // Sessions: each once, in a mood; each prompt for one of them.
    const sids = w.sessions.map(s => s.id)
    assert.equal(new Set(sids).size, sids.length, `${at}: session ids unique`)
    for (const s of w.sessions) assert.ok(MOODS.includes(s.mood), `${at}: ${s.id} mood ${s.mood}`)
    for (const a of w.asks) assert.ok(sids.includes(a.session), `${at}: prompt ${a.id} for a session there`)
    // A session waiting has its prompt, and a prompt its session waiting.
    for (const s of w.sessions.filter(x => x.mood === 'waiting')) assert.ok(w.asks.some(a => a.session === s.id), `${at}: ${s.id} waits on a prompt`)
    // What the layouts put somewhere is among the widgets (no grey stand-ins).
    const has = item => (item.type === 'widget' ? w.widgets.some(x => x.id === item.of) : item.type === 'plugin' ? w.widgets.some(x => x.plugin === item.of) : true)
    const items = list => list.flatMap(e => (typeof e === 'string' ? [] : e.items ? e.items : [e]))
    const onTaskbar = items(['left', 'center', 'right'].flatMap(k => w.taskbarLayout[k] || []))
    for (const item of [...onTaskbar, ...items(w.deskLayout)]) assert.ok(has(item), `${at}: ${item.type} ${item.of} is among the widgets`)
  }
})

test('the scenes say what they are about', () => {
  const w = id => Scenes.sceneWorld(Scenes.sceneById(id), F)
  assert.equal(w('first-run').connected, false)
  assert.equal(w('first-run').config.onboarded, false)
  assert.equal(w('first-run').sessions.length, 0)
  assert.deepEqual(w('everyday').sessions.map(s => s.mood), ['working', 'done'])
  assert.equal(w('three-busy').sessions.filter(s => s.mood === 'working').length, 3)
  assert.equal(w('ask').asks.length, 1)
  assert.equal(w('done-flow').sessions[0].mood, 'done')
  assert.equal(w('done-flow').config.endCue, 'ring')
  assert.equal(w('error').sessions[0].mood, 'error')
  const right = w('taskbar-full').taskbarLayout.right
  assert.ok(right.some(e => e.type === 'carousel') && right.some(e => e.type === 'stack'))
  assert.ok([...w('taskbar-full').taskbarLayout.left, ...right].filter(e => e.size === 'M').length >= 2, 'mediums')
  const desk = w('desktop').deskLayout
  assert.ok(desk.filter(e => e.type === 'board').length >= 2 && desk.some(e => e.type === 'stack') && desk.filter(e => e.type === 'widget' || e.type === 'plugin').length >= 2)
  assert.deepEqual(w('desktop-empty').deskLayout, [])
  assert.equal(w('island-home').home, 'island')
  assert.equal(w('light-clear').mode, 'light')
  assert.equal(w('light-clear').material, 'clear')
  assert.ok(Scenes.sceneById('edit-layout').play, 'edit-layout starts editing')
  // The hold (停留多久 30 s) with a ring, and open; a turn ending as each opens.
  assert.deepEqual([w('hold-ring').config.endCue, w('hold-ring').config.hold, w('hold-ring').sessions[0].mood], ['ring', 30, 'done'])
  assert.deepEqual([w('hold-open').config.endCue, w('hold-open').config.hold, w('hold-open').sessions[0].mood], ['open', 30, 'done'])
  assert.ok(Scenes.sceneById('hold-ring').play && Scenes.sceneById('hold-open').play)
  // The island at the top right, opened.
  assert.deepEqual([w('island-right').home, w('island-right').config.islandPos], ['island', 'right'])
  assert.ok(Scenes.sceneById('island-right').play, 'island-right opens it')
})

test('the sample mail: two accounts on the example domains, each as its widget says it', () => {
  const { accounts, letters, talks } = F.MAIL
  assert.deepEqual(accounts.map(a => a.id), ['a1', 'a2'])
  const people = []
  for (const a of accounts) {
    const kept = letters[a.id]
    const all = [...kept.inbox, ...kept.sent]
    assert.ok(kept.inbox.length >= 12, `${a.id}: a dozen letters or so`)
    // Its line in the settings, and its widget (mail.rs show): the unread,
    // the newest six of them (by UID, as they came), the newest its label.
    const unread = kept.inbox.filter(l => !l.seen)
    assert.equal(a.status.unread, unread.length, `${a.id}: its unread`)
    const w = F.widget(`inbox-${a.id}`)
    assert.equal(w.value.vars.n, String(unread.length), `${a.id}: its widget's unread`)
    const newest = [...unread].sort((x, y) => y.uid - x.uid).slice(0, 6)
    assert.deepEqual(w.detail.rows.map(r => [r.from, r.subject, r.at]), newest.map(l => [l.from.name, l.subject, l.date]), `${a.id}: its widget's letters`)
    assert.equal(w.label, `${newest[0].from.name}：${newest[0].subject}`)
    // Each folder's UIDs once, in the order the letters came.
    for (const folder of ['inbox', 'sent']) {
      const list = [...kept[folder]].sort((x, y) => x.uid - y.uid)
      assert.equal(new Set(list.map(l => l.uid)).size, list.length, `${a.id} ${folder}: each UID once`)
      for (let i = 1; i < list.length; i++) assert.ok(list[i].date > list[i - 1].date, `${a.id} ${folder}: ${list[i].uid} came after ${list[i - 1].uid}`)
    }
    // What a letter answers is there; some starred, some with files, some conversations.
    const ids = new Set(all.map(l => l.id))
    assert.equal(ids.size, all.length, `${a.id}: Message-IDs once`)
    for (const l of all) for (const id of [l.parent, ...l.refs].filter(Boolean)) assert.ok(ids.has(id), `${a.id} ${l.uid}: answers ${id}, which is there`)
    assert.ok(kept.inbox.some(l => l.flagged) && kept.inbox.some(l => l.files.length) && kept.inbox.some(l => l.parent) && kept.sent.length, `${a.id}: starred, files, conversations`)
    people.push(a.address, a.username.includes('@') ? a.username : `${a.username}@${a.address.split('@')[1]}`, ...all.flatMap(l => [l.from, ...l.to, ...l.cc].map(p => p.address)))
  }
  // Made-up people at the example domains only (the repo is public).
  for (const address of people) assert.match(address, /^[^@\s]+@example\.(com|org)$/)
  for (const [key, talk] of Object.entries(talks)) assert.ok(letters[talk.account].inbox.some(l => l.id === key && l.uid === talk.uid), `talk ${key}: about a letter there`)
})

test("the island's window where it sits, as island.rs will put it (anchor.js windowLeft)", () => {
  const A = require('../src/pet/anchor')
  const area = [0, 0, ...Scenes.SCREEN]
  for (const pos of ['left', 'center', 'right']) {
    const [x, y, w, h] = Scenes.islandWindow(pos)
    assert.equal(x, A.windowLeft(pos, area, w), pos)
    assert.deepEqual([y, w, h], [0, 760, 720], pos)
  }
  assert.deepEqual(Scenes.islandWindow('center'), Scenes.WINDOWS.island)
  assert.deepEqual(Scenes.islandWindow(undefined), Scenes.WINDOWS.island)
  // A shot's: its own place, else its scene's; its world says it too.
  assert.deepEqual(Scenes.windowOf(Scenes.byId('island-right-open')), Scenes.islandWindow('right'))
  assert.deepEqual(Scenes.windowOf(Scenes.byId('island-left')), Scenes.islandWindow('left'))
  assert.deepEqual(Scenes.windowOf(Scenes.byId('scene-island-right')), Scenes.islandWindow('right'))
  assert.deepEqual(Scenes.windowOf(Scenes.byId('scene-island-home')), Scenes.WINDOWS.island)
  assert.deepEqual(Scenes.homeWindowOf({ scene: 'everyday' }), Scenes.WINDOWS.taskbar)
  assert.equal(Scenes.worldFor(Scenes.byId('island-left-open'), F).config.islandPos, 'left')
  assert.equal(Scenes.worldFor(Scenes.byId('flow-held-right'), F).config.islandPos, 'right')
  assert.equal(Scenes.worldFor(Scenes.byId('flow-held-right'), F).config.hold, 30)
  // Each shot's view on its page's window, at least in part (what it shows is there).
  for (const s of Scenes.LIST.filter(x => x.page === 'island' && Scenes.homeOf(x) === 'island')) {
    const [vx, , vw] = s.view
    const [wx, , ww] = Scenes.windowOf(s)
    assert.ok(vx < wx + ww && vx + vw > wx, `${s.id}: its view on its window`)
  }
})

test('the gallery shoots every scene, and each shot is a scene or a world of its own', () => {
  const shotIds = Scenes.LIST.map(s => s.id)
  assert.equal(new Set(shotIds).size, shotIds.length, 'shot ids unique')
  const sections = new Set(Scenes.SECTIONS.map(([k]) => k))
  for (const s of Scenes.LIST) {
    assert.ok(sections.has(s.section), `${s.id}: section`)
    assert.ok(s.scene || s.world, `${s.id}: a scene or a world`)
    if (s.scene) assert.ok(Scenes.sceneById(s.scene), `${s.id}: scene ${s.scene}`)
    assert.ok(['island', 'desk', 'bar', 'screen'].includes(s.page), `${s.id}: page`)
    const [x, y, w, h] = s.view
    assert.ok(x >= 0 && y >= 0 && x + w <= Scenes.SCREEN[0] && y + h <= Scenes.SCREEN[1], `${s.id}: view on the screen`)
    // Its world builds (the fixtures' data and the scene's).
    const world = Scenes.worldFor(s, F)
    assert.equal(typeof world, 'object')
  }
  for (const scene of Scenes.SCENES) assert.ok(Scenes.LIST.some(s => s.scene === scene.id), `${scene.id} is shot`)
})

test("the market's shots: its list, a category, an item's page, installing, not reached, not open yet, updates waiting", () => {
  const ids = ['settings-market', 'settings-market-pets', 'settings-market-detail', 'settings-market-detail-view', 'settings-market-detail-pack', 'settings-market-confirm', 'settings-market-installing', 'settings-market-error', 'settings-market-closed', 'settings-plugins-updates', 'settings-plugins-updates-confirm']
  for (const id of ids) assert.ok(Scenes.byId(id), id)
  const w = id => Scenes.worldFor(Scenes.byId(id), F)
  assert.equal(w('settings-market').marketMode, undefined, 'the fake reads its index as it is')
  assert.equal(w('settings-market-error').marketMode, 'error')
  assert.equal(w('settings-market-closed').marketMode, 'closed')
  assert.equal(w('settings-market-installing').marketHold, 45)
  assert.deepEqual(w('settings-plugins-updates').marketUpdates.map(u => u.id), ['rain-alert', 'weather-radar'])
  assert.equal(w('settings-plugins-updates-confirm').config.pluginAutoUpdate, false)
  // The fixtures' market: the mockup's seven, each by the index's rules
  // (market.rs item_json): a sample for the widgets, a picture for the
  // packs; one asking for the web alone, one with a view, one with an .svg.
  assert.deepEqual(F.MARKET.map(i => i.name.zh).sort(), ['Agent 排行', 'GitHub 通知', '下雨提醒', '像素猫', '柴犬小八', '极光', '汇率'].sort())
  for (const i of F.MARKET) {
    assert.match(i.id, /^[a-z][a-z0-9-]{1,39}$/, i.id)
    if (i.kind === 'plugin') assert.ok(i.sample && typeof i.sample.value === 'string', `${i.id}: a sample`)
    else assert.ok('preview' in i && !i.sample, `${i.id}: a pack's picture, no sample`)
  }
  const A = require('../src/pet/addonui')
  assert.ok(F.MARKET.some(i => i.kind === 'plugin' && !A.isSensitive(i.perms) && i.perms.net.length), 'one asking for the web alone')
  assert.ok(F.MARKET.some(i => i.view) && F.MARKET.some(i => i.icon), 'a view of its own, an .svg of its own')
  // The ones the shots press are there.
  for (const id of ['fx', 'agent-board', 'gh-notify']) assert.ok(F.MARKET.some(i => i.id === id), id)
})

test("the packs' shots: the Plugins page's 资源包, her pets and the wallpaper with theirs, a pack's page and 卸载, one from a file", () => {
  const ids = ['settings-plugins-packs', 'settings-look-pets-packs', 'settings-look-wallpaper-packs', 'settings-pack-detail', 'settings-pack-uninstall-confirm', 'settings-pack-install-confirm']
  for (const id of ids) assert.ok(Scenes.byId(id), id)
  const w = Scenes.worldFor(Scenes.byId('settings-look-wallpaper-packs'), F)
  // A pet and two wallpaper packs, one from a file; the slideshow on one of them.
  assert.deepEqual(w.packs.map(p => [p.kind, p.id, p.reviewed]), [['pet', 'pet-pixelcat', true], ['wallpaper', 'wall-aurora', true], ['wallpaper', 'beach', false]])
  assert.equal(w.paper.slideshow, true)
  assert.equal(w.paper.folder, w.packs.find(p => p.id === 'wall-aurora').folder)
  // Each as packs.rs view has it; her pets list the pack's pet.
  for (const p of F.PACKS) for (const k of ['id', 'kind', 'name', 'author', 'version', 'source', 'reviewed', 'size', 'pictures', 'preview', 'folder']) assert.ok(k in p, `${p.id}: ${k}`)
  assert.deepEqual(F.PACK_PETS.map(p => [p.id, p.pack.source]), [['pet-pixelcat', 'market']])
  // The pet pack picked from a file is one; the one with a script is refused.
  assert.equal(Scenes.worldFor(Scenes.byId('settings-pack-install-confirm'), F).addonPick, 'mochi.zip')
  assert.equal(F.ADDON_FILES['mochi.zip'].kind, 'pet')
  assert.equal(F.ADDON_FILES['walls-plus.zip'].refused.reason, 'packType')
})

test("a shot's world is its scene's, its own home over it; the play its scene's unless left out", () => {
  const shot = { id: 'x', scene: 'everyday', home: 'island', page: 'island' }
  const w = Scenes.worldFor(shot, F)
  assert.equal(w.home, 'island')
  assert.deepEqual(w.sessions.map(s => s.mood), ['working', 'done'])
  assert.equal(Scenes.worldFor({ id: 'y', scene: 'light-clear' }, F).mode, 'light')
  assert.equal(Scenes.playOf({ scene: 'done-flow' }), Scenes.sceneById('done-flow').play)
  assert.equal(Scenes.playOf({ scene: 'done-flow', play: false }), null)
  assert.equal(Scenes.playOf({ world: () => ({}) }), null)
  assert.equal(Scenes.stepsOn({ page: 'screen' }), 'island')
  assert.equal(Scenes.homeOf({ scene: 'island-home' }), 'island')
  assert.deepEqual(Scenes.windowOf({ page: 'screen' }), Scenes.WINDOWS.desk)
  assert.throws(() => Scenes.worldFor({ id: 'z', scene: 'nowhere' }, F), /no scene/)
})

test('the quiet shots are of nobody there, in each home and on the desktop: boot.js fails one with an endless animation', () => {
  const quiet = Scenes.LIST.filter(s => s.section === 'quiet')
  assert.deepEqual(quiet.map(s => [Scenes.homeOf(s), s.page]), [['taskbar', 'island'], ['island', 'island'], ['taskbar', 'screen']])
  for (const s of quiet) {
    const w = Scenes.worldFor(s, F)
    assert.equal(w.quiet, true, `${s.id}: quiet`)
    assert.deepEqual(w.sessions, [], `${s.id}: no session at work`)
    assert.equal(Scenes.playOf(s), null, `${s.id}: nothing played`)
  }
})

test("a build's label says the branch and commit, main as 主线, and unsaved work", () => {
  assert.equal(gitinfo.labelOf({ branch: 'main', sha: '3ec87d9' }), '主线 · 3ec87d9')
  assert.equal(gitinfo.labelOf({ branch: 'master', sha: '3ec87d9' }), '主线 · 3ec87d9')
  assert.equal(gitinfo.labelOf({ branch: 'simulator', sha: '3ec87d9' }), '分支 simulator · 3ec87d9')
  assert.equal(gitinfo.labelOf({ branch: 'simulator', sha: '3ec87d9', dirty: true }), '分支 simulator · 3ec87d9 + 未提交的改动')
  assert.equal(gitinfo.labelOf({ branch: null, sha: '3ec87d9' }), '3ec87d9')
  assert.equal(gitinfo.labelOf({ branch: 'HEAD', sha: '3ec87d9' }), '3ec87d9')
})

test("a build's label comes from git: this checkout's commit, and the page carries it", () => {
  let sha
  try {
    sha = execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: REPO, encoding: 'utf8' }).trim()
  } catch {
    return // no git here
  }
  const said = gitinfo.label(REPO)
  assert.ok(said.includes(sha), `${said} has ${sha}`)
  assert.equal(gitinfo.ref(REPO, 'HEAD').sha, sha)
  const html = publish.withLabel('<h1>Wakuwaku<!--wk-build--></h1>', said)
  assert.ok(html.includes(`<span class="wk-build">${said.replace(/&/g, '&amp;')}</span>`))
  assert.equal(publish.withLabel('<h1>Wakuwaku<!--wk-build--></h1>', ''), '<h1>Wakuwaku</h1>')
  assert.ok(publish.withLabel('<!--wk-build-->', '<b>&', true).includes('wk-build wk-live">&lt;b&gt;&amp;</span>'))
})

test('the live playground: a stylesheet swapped, a script its frames reloaded, its own files the page', () => {
  const pages = publish.framePages()
  assert.ok(pages.island.has('pet/island.js') && pages.island.has('shared/i18n.js') && pages.desk.has('pet/desk.js'))
  const at = rel => publish.liveChange(rel, pages)
  assert.deepEqual(at('src/pet/style.css'), { css: { island: ['pet/style.css'] } })
  assert.deepEqual(at('src/pet/desk.css'), { css: { desk: ['pet/desk.css'] } })
  assert.deepEqual(at('src/pet/island.js'), { frames: ['island'] })
  assert.deepEqual(at('src/pet/widgets.js'), { frames: ['island', 'desk'] })
  assert.deepEqual(at('src/shared/i18n.js'), { frames: ['island', 'desk'] })
  assert.deepEqual(at('src/pet/index.html'), { frames: ['island'] })
  assert.deepEqual(at('tools/preview/boot.js'), { frames: ['island', 'desk'] })
  assert.deepEqual(at('pets/claude-chan/spritesheet.webp'), { frames: ['island', 'desk'] })
  // A plugin's view and her SDK, which the fake writes into a card's frame.
  assert.deepEqual(at('examples/plugins/agent-board/view.js'), { frames: ['island', 'desk'] })
  assert.deepEqual(at('src/addon/sdk.js'), { frames: ['island', 'desk'] })
  for (const f of ['playground.html', 'playground.js', 'fake-tauri.js', 'fixtures.js', 'scenes.js']) assert.deepEqual(at(`tools/preview/${f}`), { page: true }, f)
  assert.deepEqual(at('tools/preview/assets/wall.jpg'), { page: true })
  // Not the playground's: nothing to do.
  for (const f of ['src/pet/bar.js', 'tools/preview/render.ps1', 'tools/preview/compare.js', 'src-tauri/src/main.rs', 'docs/DEVELOPMENT.md']) assert.equal(at(f), null, f)
  // A burst of saves, one message.
  const msg = publish.mergeChanges([{ rel: 'src/pet/style.css', change: at('src/pet/style.css') }, { rel: 'src/pet/island.js', change: at('src/pet/island.js') }, { rel: 'x', change: null }])
  assert.deepEqual(msg, { page: false, frames: ['island'], css: { island: ['pet/style.css'] }, files: ['src/pet/style.css', 'src/pet/island.js'] })
  assert.equal(publish.mergeChanges([{ rel: 'x', change: null }]), null)
})

test('the live playground reads only from its folders', () => {
  assert.ok(publish.repoFile('pet/island.js').endsWith(path.join('src', 'pet', 'island.js')))
  assert.ok(publish.repoFile('preview/boot.js').endsWith(path.join('tools', 'preview', 'boot.js')))
  assert.equal(publish.repoFile('pet/../../package.json'), null)
  assert.equal(publish.repoFile('preview/../../../secret'), null)
  assert.equal(publish.repoFile('src/pet/island.js'), null)
  assert.equal(publish.publishedOf('src/pet/island.js'), 'pet/island.js')
  assert.equal(publish.publishedOf('tools/preview/assets/wall.jpg'), 'assets/wall.jpg')
})

test('a build has every file its two pages load', () => {
  const plan = publish.plan()
  for (const role of ['island', 'desk']) for (const pub of publish.framePages()[role]) assert.ok(plan.has(pub), pub)
  for (const f of ['playground.html', 'preview/scenes.js', 'preview/fixtures.js', 'preview/playground.js', 'assets/wall.jpg', 'pets/claude-chan/spritesheet.webp']) assert.ok(plan.has(f), f)
  // The example plugin with a view of its own, and her SDK, for the cards that draw it.
  for (const f of ['addon/sdk.js', 'plugins/agent-board/view.html', 'plugins/agent-board/view.js', 'plugins/agent-board/view.css', 'plugins/agent-board/manifest.json']) assert.ok(plan.has(f), f)
  assert.ok(![...plan.keys()].some(f => f.startsWith('plugins/tally/')), 'none without a view')
})

// --- compare.js: PNGs read, pictures compared -----------------------------------------------
// A PNG of RGBA pixels, each row with its filter (0 none ... 4 Paeth).
function png(width, height, rgba, filters, type = 6) {
  const ch = type === 6 ? 4 : 3
  const px = Buffer.alloc(width * height * ch)
  for (let i = 0; i < width * height; i++) for (let c = 0; c < ch; c++) px[i * ch + c] = rgba[i * 4 + c]
  const stride = width * ch
  const rows = []
  for (let y = 0; y < height; y++) {
    const f = filters[y % filters.length]
    const row = Buffer.alloc(stride + 1)
    row[0] = f
    for (let x = 0; x < stride; x++) {
      const v = px[y * stride + x]
      const a = x >= ch ? px[y * stride + x - ch] : 0
      const b = y ? px[(y - 1) * stride + x] : 0
      const c = y && x >= ch ? px[(y - 1) * stride + x - ch] : 0
      const p = a + b - c
      const paeth = Math.abs(p - a) <= Math.abs(p - b) && Math.abs(p - a) <= Math.abs(p - c) ? a : Math.abs(p - b) <= Math.abs(p - c) ? b : c
      const pred = [0, a, b, (a + b) >> 1, paeth][f]
      row[x + 1] = (v - pred) & 255
    }
    rows.push(row)
  }
  const chunk = (kind, data) => {
    const len = Buffer.alloc(4)
    len.writeUInt32BE(data.length)
    const body = Buffer.concat([Buffer.from(kind, 'ascii'), data])
    const crc = Buffer.alloc(4)
    crc.writeUInt32BE(zlib.crc32(body))
    return Buffer.concat([len, body, crc])
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(width, 0)
  ihdr.writeUInt32BE(height, 4)
  ihdr[8] = 8
  ihdr[9] = type
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(Buffer.concat(rows))), chunk('IEND', Buffer.alloc(0))])
}
const picture = (w, h, at = () => [40, 80, 120, 255]) => {
  const d = Buffer.alloc(w * h * 4)
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) d.set(at(x, y), (y * w + x) * 4)
  return d
}

test('compare.js reads PNGs with every row filter, RGB and RGBA', () => {
  const w = 7
  const h = 6
  const rgba = picture(w, h, (x, y) => [(x * 37 + y * 11) & 255, (x * x + y * 53) & 255, (200 - x * y) & 255, 255 - x])
  const back = compare.readPng(png(w, h, rgba, [0, 1, 2, 3, 4]))
  assert.equal(back.width, w)
  assert.equal(back.height, h)
  assert.deepEqual([...back.data], [...rgba])
  const opaque = picture(w, h, (x, y) => [x * 30, y * 40, 7, 255])
  assert.deepEqual([...compare.readPng(png(w, h, opaque, [4, 3, 2, 1, 0], 2)).data], [...opaque])
  assert.throws(() => compare.readPng(Buffer.from('not a png at all')), /not a PNG/)
})

test('compare.js finds what changed, and the box round it; noise is no change', () => {
  const w = 20
  const h = 10
  const a = { width: w, height: h, data: picture(w, h) }
  assert.deepEqual(compare.diffImages(a, a), { size: false, changed: 0, box: null })
  // A small shift of a channel (antialiasing): within the tolerance.
  const near = { width: w, height: h, data: picture(w, h, () => [40 + compare.TOLERANCE, 80, 120, 255]) }
  assert.equal(compare.diffImages(a, near).changed, 0)
  // A block that changed.
  const b = { width: w, height: h, data: picture(w, h, (x, y) => (x >= 3 && x < 8 && y >= 2 && y < 5 ? [255, 0, 0, 255] : [40, 80, 120, 255])) }
  assert.deepEqual(compare.diffImages(a, b), { size: false, changed: 15, box: [3, 2, 5, 3] })
  assert.equal(compare.diffImages(a, { width: 5, height: 5, data: picture(5, 5) }).size, true)
})

test('compare.js judges a pair from its files', t => {
  const fs = require('node:fs')
  const os = require('node:os')
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wk-judge-'))
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }))
  const w = 30
  const h = 12
  const write = (name, at) => {
    const f = path.join(dir, name)
    fs.writeFileSync(f, png(w, h, picture(w, h, at), [1]))
    return f
  }
  const a = write('a.png', () => [10, 10, 10, 255])
  const same = write('same.png', () => [10, 10, 10, 255])
  const speck = write('speck.png', (x, y) => (x === 4 && y === 4 ? [250, 10, 10, 255] : [10, 10, 10, 255]))
  const block = write('block.png', (x, y) => (x >= 10 && x < 20 && y < 6 ? [250, 250, 10, 255] : [10, 10, 10, 255]))
  assert.equal(compare.judge(a, same).status, 'same')
  assert.equal(compare.judge(a, speck).status, 'same', 'a pixel or few: noise')
  assert.deepEqual(compare.judge(a, block), { status: 'changed', changed: 60, box: [10, 0, 10, 6] })
  assert.equal(compare.judge(null, a).status, 'new')
  assert.equal(compare.judge(a, null).status, 'gone')
})

test('compare.js takes its options', () => {
  assert.deepEqual(compare.parseArgs([]), { base: 'main', head: null, only: ['*'], modes: ['dark', 'light'], out: null, scale: 2, keep: false })
  const o = compare.parseArgs(['--base', 'v0.3.2', '--head', 'simulator', '--only', 'scene-*, capsule-open', '--modes', 'light', '--scale', '1', '--keep'])
  assert.deepEqual([o.base, o.head, o.only, o.modes, o.scale, o.keep], ['v0.3.2', 'simulator', ['scene-*', 'capsule-open'], ['light'], 1, true])
  assert.throws(() => compare.parseArgs(['--what']), /unknown/)
})

test('compare.js pairs only the shots --only picks, as render.ps1 matches them', () => {
  const p = compare.picks(['scene-*', 'capsule-open', 'fl?w-dot'])
  assert.deepEqual(['scene-ask', 'capsule-open', 'capsule-idle', 'flow-dot', 'Scene-Desktop', 'menu.app'].map(p), [true, true, false, true, true, false])
  assert.equal(compare.picks(['*'])('anything'), true)
  assert.equal(compare.picks(['menu.app'])('menuxapp'), false, 'a dot is a dot')
})
