// Packs on the Plugins page and the Look page (packs.rs; addonui.js the
// words; docs/design/specs/plugins.md §1, §3): a pack's row and its page,
// a pack pet in her list, a wallpaper pack's row, installing one from a
// file, why one will not go in, the market's state for one, the links to
// the market on its kind; and the examples as the market's check takes them.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import * as A from '../src/pet/addonui.ts'
import F from '../tools/preview/fixtures.js'
import pkg from '../tools/market/package.js'
import { t, list, STRINGS } from '../src/shared/i18n.ts'

const zh = (key, vars?) => t('zh', key, vars)
const en = (key, vars?) => t('en', key, vars)
const andZh = names => list('zh', names, 'and')
const item = id => F.MARKET.find(i => i.id === id)

const cat = { id: 'pet-pixelcat', kind: 'pet', name: '像素猫', author: 'neko', version: '1.1.0', source: 'market', reviewed: true, size: 2.1 * 1048576, pictures: 0, preview: null, inUse: false }
const aurora = { id: 'wall-aurora', kind: 'wallpaper', name: '极光', author: 'sora', version: '1.0.0', source: 'market', reviewed: true, size: 18 * 1048576, pictures: 6, preview: null, inUse: true }
const beach = { id: 'beach', kind: 'wallpaper', name: '海边', author: 'you', version: '0.1.0', source: 'file', reviewed: false, size: 512 * 1024, pictures: 1, preview: null, inUse: false }

test('a pack’s row on the Plugins page: 「宠物 · @作者 · 大小」, 在「外观」里选 at its kind’s part', () => {
  assert.equal(A.packLine(cat, zh), '宠物 · @neko · 2.1 MB')
  assert.equal(A.packLine(aurora, zh), '壁纸 · @sora · 18.0 MB')
  assert.equal(A.packLine({ ...beach, author: '' }, en), 'Wallpaper · 512 KB')
  assert.equal(A.packByline(cat, zh), '@neko · v1.1.0 · 2.1 MB · 宠物')
  assert.equal(A.packKey(cat), 'pet:pet-pixelcat')
  assert.equal(zh('packs.toLook'), '在「外观」里选')
  // Its 在「外观」里选: her pets, or the wallpaper.
  assert.deepEqual([A.lookPartOf('pet'), A.lookPartOf('wallpaper')], ['pets', 'paper'])
  assert.deepEqual([A.isPack(cat), A.isPack(beach), A.isPack({ kind: 'plugin' }), A.isPack(null)], [true, true, false, false])
  // Its page's 它能做的, as the market's page says it for a pack.
  assert.equal(zh('mk.packOnly'), '只有图片，没有代码')
  // 卸载, asked first: what goes, and what falls back.
  const out = A.packUninstallLayerOf(cat, zh)
  assert.equal(`${out.title}${out.notes[0]}`, '卸载「像素猫」？它的图一起删掉；正用着它的话，换回默认的宠物。')
  assert.equal(A.packUninstallLayerOf(aurora, zh).notes[0], '它的图片一起删掉；桌面正在轮换它的话，停在现在这一张。')
  assert.deepEqual(out.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['uninstall', '卸载']])
  assert.equal(A.packUninstallLayerOf(cat, zh, true).busy, true)
})

test('the Look page: a pack pet says where it came from; a wallpaper pack 「壁纸包 · N 张」; both lead to the market on their kind', () => {
  // Her pets: who made it, then Codex's, 来自市场, or 未经审核 from a file; an old sheet's note.
  assert.equal(A.petNote({ author: 'zhoupengjie' }, zh), 'zhoupengjie')
  assert.equal(A.petNote({ author: 'neko', pack: { source: 'market', reviewed: true } }, zh), 'neko · 来自市场')
  assert.equal(A.petNote({ author: 'you', pack: { source: 'file', reviewed: false } }, zh), 'you · 未经审核')
  assert.equal(A.petNote({ author: 'x', codex: true, version: 1 }, zh), `x · 来自 Codex · ${zh('home.v1')}`)
  assert.equal(A.petNote({ author: 'neko', pack: { reviewed: true } }, en), 'neko · From the market')
  // A wallpaper pack: its pictures, who made it, 未经审核 from a file.
  assert.equal(A.wallLine(aurora, zh), '壁纸包 · 6 张 · @sora')
  assert.equal(A.wallLine(beach, zh), '壁纸包 · 1 张 · @you · 未经审核')
  assert.equal(A.wallLine(beach, en), 'Wallpaper pack · 1 picture · @you · Not reviewed')
  assert.equal(A.wallLine(aurora, en), 'Wallpaper pack · 6 pictures · @sora')
  // Its buttons' words; the rows to the market at the end of each.
  assert.deepEqual(['packs.use', 'packs.inUse', 'packs.morePets', 'packs.moreWalls', 'packs.moreWallsNote'].map(k => zh(k)), ['用这一包', '在用', '获取更多宠物…', '获取更多壁纸…', '壁纸包装好后出现在这里'])
  // They open the market on their kind (spec §1), as the picker's does on its widgets.
  assert.deepEqual([A.marketLink('widgets:market:pet'), A.marketLink('widgets:market:wallpaper')], [{ cat: 'pet' }, { cat: 'wallpaper' }])
  assert.deepEqual(A.marketList(F.MARKET, A.marketLink('widgets:market:wallpaper')).map(i => i.id), ['wall-aurora'])
})

test('installing a pack from a file: asked, pictures only, not reviewed; the word after points to Look', () => {
  const pet = { ok: true, file: 'pixel-cat.zip', id: 'pixel-cat', name: '像素猫', version: '1.1.0', author: 'neko', kind: 'pet', perms: { net: [], agents: false, hardware: false, notify: false }, sensitive: false, replaces: null, api: 1, supported: true, refused: null }
  const ask = A.layerOf({ step: 'ask', path: 'pixel-cat.zip', summary: pet }, zh, andZh)
  assert.equal(ask.title, '安装「像素猫」？')
  assert.equal(`${ask.warn.head}${ask.warn.text}`, '未经审核：没人看过它。它只有图片和说明文字，没有代码，什么也运行不了。')
  assert.deepEqual(ask.will, ['只有图片，没有代码'])
  assert.deepEqual(ask.buttons.map(b => b.slice(0, 2)), [['cancel', '取消'], ['go', '仍然安装']])
  // Its update: in use, it stays so.
  assert.deepEqual(A.layerOf({ step: 'ask', summary: { ...pet, replaces: '1.0.0' } }, zh).notes, ['替换已装的 1.0.0；正用着的话，接着用'])
  // Installed: its kind kept, and the word says where to pick it.
  const done = A.next({ step: 'installing', path: 'pixel-cat.zip', summary: pet }, { type: 'installed', result: pet }).done
  assert.deepEqual(done, { id: 'pixel-cat', name: '像素猫', kind: 'pet' })
  assert.equal(A.installedWords(done, zh), '装好了「像素猫」· 在「外观」里选')
  assert.equal(A.installedWords({ id: 'x', name: '数数', kind: 'plugin' }, zh), '装好了「数数」· 用「添加小工具…」放到哪里')
  // Why one will not go in: a .js in it, a pet without its files, a
  // wallpaper pack with no pictures, the person's own pet or folder of that name.
  const why = r => {
    const w = A.refusedWords(r, zh)
    return `${w.head}${w.text}`
  }
  assert.equal(why({ reason: 'packType', file: 'walk.js' }), '包里有资源包不能带的文件：walk.js。资源包只有图片和说明文字，没有代码。')
  assert.equal(why({ reason: 'pet', file: null }), '宠物包的最外层要有 pet.json 和 spritesheet.webp')
  assert.equal(why({ reason: 'pet', file: 'pet.json', detail: 'not a JSON object' }), 'pet.json 不对：not a JSON object')
  assert.equal(why({ reason: 'pictures' }), '壁纸包的最外层没有图片（.jpg .jpeg .png .webp）')
  assert.equal(why({ reason: 'clash', file: 'mochi', detail: "pet mochi: a pet of that name is there already, not a pack's" }), '已经有一只叫 mochi 的宠物了，不是装的资源包：她不动它，这个就装不了')
  assert.equal(why({ reason: 'clash', file: 'sea', detail: 'wallpaper sea: a folder of that name is there already' }), '壁纸文件夹里已经有一个 sea，不是装的资源包：她不动它，这个就装不了')
  const no = A.layerOf({ step: 'refused', file: 'pixel-cat.zip', refused: { reason: 'packType', file: 'walk.js' } }, zh)
  assert.doesNotMatch(no.line.text, /外部程序/, 'a pack is pointed to the docs, not to external programs')
})

test('the market’s 已安装 and 更新 for a pack: the pack of its kind here, not a plugin of that name', () => {
  const here = { addons: [{ id: 'rain-alert', kind: 'plugin', version: '1.2.0' }, { id: 'pet-shiba', kind: 'plugin', version: '9.9.9' }], packs: [{ ...cat, version: '1.0.0' }, aurora] }
  assert.equal(A.installedOf(item('pet-pixelcat'), here), '1.0.0')
  assert.equal(A.installedOf(item('wall-aurora'), here), '1.0.0')
  assert.equal(A.installedOf(item('pet-shiba'), here), null, 'a plugin of that id is no pet')
  assert.equal(A.installedOf(item('rain-alert'), here), '1.2.0')
  assert.equal(A.installedOf(item('fx'), here), null)
  assert.equal(A.installedOf(null, here), null)
  // So the button: 更新 for the older pet here, 已安装 for the wallpapers, on its page 去「外观」里选.
  const b = (id, o = {}) => A.marketButton(item(id), { T: zh, installed: A.installedOf(item(id), here), ...o })
  assert.deepEqual([b('pet-pixelcat').what, b('wall-aurora').what, b('wall-aurora', { detail: true }).what, b('pet-shiba').what], ['update', 'installed', 'look', 'install'])
  assert.equal(A.marketDoneOf(item('wall-aurora'), zh, 'zh'), '装好了「极光」· 在「外观」里选')
})

test('every word of the packs is there, in both languages', () => {
  const keys = Object.keys(STRINGS.zh).filter(k => k.startsWith('packs.') || /^addons\.(no\.(packType|pet|pictures|clash)|unreviewedPack|replacesPack)/.test(k))
  assert.ok(keys.length >= 20, `${keys.length} keys`)
  for (const k of keys) {
    assert.ok(STRINGS.en[k], k)
    assert.doesNotMatch(STRINGS.en[k], /[一-鿿]/, k)
  }
})

// A wallpaper pack as one is made (the repository keeps no example of one
// since 2026-11-02): pictures at its top and a preview, as the market takes it.
test('a wallpaper pack passes the market’s check, its pictures at its top', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'wk-pack-'))
  const png = Buffer.from('\x89PNG\r\n\x1a\n....', 'latin1')
  const manifest = { id: 'three-skies', kind: 'wallpaper', name: { en: 'Three skies', zh: '三种天色' }, description: { en: 'Dusk, a lake and an aurora', zh: '黄昏、湖和极光' }, version: '1.0.0', author: 'wakuwaku' }
  fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify(manifest))
  for (const f of ['dusk.png', 'lake.png', 'aurora.png', 'preview.png']) fs.writeFileSync(path.join(dir, f), png)
  try {
    const r = pkg.read(dir, { market: false })
    assert.deepEqual(r.errors, [])
    assert.deepEqual(r.warnings.map(w => w.code), [])
    assert.equal(r.kind, 'wallpaper')
    const pictures = r.files.filter(f => /\.(png|jpe?g|webp)$/.test(f.path) && !f.path.startsWith('preview.'))
    assert.ok(pictures.length === 3 && pictures.every(f => !f.path.includes('/')), 'three, at its top')
    assert.ok(pkg.previewOf(r)?.startsWith('data:image/png;base64,'), 'its preview, inlined')
  } finally {
    fs.rmSync(dir, { recursive: true, force: true })
  }
})
