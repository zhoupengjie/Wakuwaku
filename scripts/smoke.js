#!/usr/bin/env node
// End-to-end check: starts a real pet window on its own port, profile and
// Claude Code settings folder, drives it as Claude Code's hooks would, checks
// each step and saves snapshots to out/smoke/ for a look.
//
//   npm run smoke
const { spawn } = require('child_process')
const fs = require('fs')
const os = require('os')
const path = require('path')

const { render, t } = require('../src/shared/i18n')
const { ENSURE_FLAG, PLUGIN_ID } = require('../src/shared/hooks-config')

const ROOT = path.join(__dirname, '..')
const OUT = path.join(ROOT, 'out', 'smoke')
const PORT = 47299
const URL = `http://127.0.0.1:${PORT}`
const electron = require(path.join(ROOT, 'node_modules', 'electron'))

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))

async function post(route, body) {
  const res = await fetch(`${URL}${route}`, { method: 'POST', body: JSON.stringify(body) })
  return res.json()
}

// An event as Claude Code's HTTP hook sends it; resolves to the hook output.
const hook = event => post('/hook?from=claude-pets', event)
const state = async () => (await (await fetch(`${URL}/health`)).json()).state
const click = async selector => (await post('/debug/click', { selector })).result
const evaluate = async (page, code) => (await post('/debug/eval', { page, code })).result
const settingsPatch = async patch => (await post('/debug/settings', patch)).result
const walkBy = (dx, ms) => post('/debug/walk', { dx, ms })
const bubble = () => evaluate('pet', "document.getElementById('bubble').textContent")
// The main window: switch tab, click, read.
const tab = name => evaluate('settings', `(document.querySelector('[data-tab="${name}"]').click(), 'ok')`)
const press = selector => evaluate('settings', `(() => { const b = document.querySelector(${JSON.stringify(selector)}); if (!b) return 'missing'; b.click(); return 'ok' })()`)
const read = (selector, what = 'textContent') => evaluate('settings', `document.querySelector(${JSON.stringify(selector)})?.${what}`)

async function snap(name, page = 'pet') {
  // A covered window stops painting: bring the main window up first.
  if (page === 'settings') {
    await post('/debug/settings', 'open')
    await sleep(400)
  }
  const png = Buffer.from(await (await fetch(`${URL}/snapshot?page=${page}`)).arrayBuffer())
  fs.writeFileSync(path.join(OUT, `${name}.png`), png)
}

let failures = 0
function report(step, ok, info) {
  if (!ok) failures += 1
  console.log(`${ok ? '✔' : '✘'} ${step}${info ? `: ${info}` : ''}`)
}

async function expect(step, want) {
  const { mood, detail } = await state()
  const shown = render('zh', detail)
  const ok = mood === want.mood && (want.detail === undefined || shown === want.detail)
  report(step, ok, `${mood}${shown ? ` · ${shown}` : ''}${ok ? '' : `（应为 ${want.mood}${want.detail ? ` · ${want.detail}` : ''}）`}`)
}

// The window is its own size (a pixel or two of rounding aside) and wholly
// on its display's work area.
function checkWindow(step, now, start) {
  const { bounds: b, size, workArea: a } = now
  const isSize = Math.abs(b.width - start.bounds.width) <= 2 && Math.abs(b.height - start.bounds.height) <= 2
  const isInside = b.x >= a.x && b.y >= a.y && b.x + size.width <= a.x + a.width && b.y + size.height <= a.y + a.height
  report(step, isSize && isInside, `${b.width}x${b.height} @ ${b.x},${b.y}（开始时 ${start.bounds.width}x${start.bounds.height}）`)
}

async function waitUp(ms = 15000) {
  const end = Date.now() + ms
  while (Date.now() < end) {
    try {
      if ((await fetch(`${URL}/health`)).ok) return true
    } catch {}
    await sleep(250)
  }
  return false
}

async function isDown() {
  try {
    await fetch(`${URL}/health`)
    return false
  } catch {
    return true
  }
}

async function waitFor(check, ms = 15000) {
  const end = Date.now() + ms
  while (Date.now() < end) {
    if (await check()) return true
    await sleep(300)
  }
  return false
}

function run(args, env, input) {
  return new Promise(resolve => {
    const started = Date.now()
    const child = spawn(electron, args, { env, stdio: [input === undefined ? 'ignore' : 'pipe', 'ignore', 'ignore'] })
    if (input !== undefined) child.stdin.end(input)
    child.on('exit', code => resolve({ code, ms: Date.now() - started }))
  })
}

async function main() {
  if (!fs.existsSync(path.join(ROOT, 'pets', 'deepseek-chan', 'spritesheet.webp'))) {
    throw new Error('先运行 npm run fetch-pet')
  }
  fs.rmSync(OUT, { recursive: true, force: true })
  fs.mkdirSync(OUT, { recursive: true })
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-smoke-'))
  const claudeDir = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-smoke-claude-'))
  const claudeSettings = path.join(claudeDir, 'settings.json')
  const env = {
    ...process.env,
    CLAUDE_PETS_PORT: String(PORT),
    CLAUDE_PETS_USER_DATA: profile,
    CLAUDE_CONFIG_DIR: claudeDir,
    CLAUDE_PETS_DEBUG: '1',
  }
  // The test talks Chinese unless told otherwise.
  fs.writeFileSync(path.join(profile, 'config.json'), JSON.stringify({ lang: 'zh' }))

  const app = spawn(electron, [ROOT], { env, stdio: 'ignore' })

  try {
    if (!(await waitUp())) throw new Error('窗口 15 秒内没有起来')
    await sleep(1500)

    // --- First run: the main window opens to set up.
    report('第一次打开：主窗口自己弹出', (await read('.welcome', 'className')) !== undefined && (await read('.welcome', 'className')) !== null)
    report('主窗口：没有连接时提示', (await read('[data-connection]', 'dataset.connection')) === 'none')
    await snap('01-home-now', 'settings')

    await tab('claude')
    await sleep(200)
    await snap('02-home-claude', 'settings')
    report('Claude Code 页：推荐插件，给出安装命令', (await read('[data-plugin]', 'dataset.plugin')) === 'off' && (await read('.steps code')) === '/plugin marketplace add zhoupengjie/claude-pets')
    report('Claude Code 页：点「复制」', (await press('[data-action="copy-1"]')) === 'ok')
    await sleep(150)
    report('复制后按钮显示「已复制」', (await read('[data-action="copy-1"]')) === '已复制')
    report('高级：点「安装」写入 settings.json', (await press('[data-action="install"]')) === 'ok')
    await sleep(500)
    const written = JSON.parse(fs.readFileSync(claudeSettings, 'utf8'))
    const starter = written.hooks.SessionStart[0].hooks
    report(
      'hooks 写进了设置：会话开始只有启动命令（Claude Code 不跑这一事件的 HTTP hook）',
      starter.length === 1 && starter[0].type === 'command' && starter[0].command === electron && starter[0].args.includes(ENSURE_FLAG),
      `${Object.keys(written.hooks).length} 个事件`,
    )
    report('状态显示「已安装」', (await read('[data-status]', 'dataset.status')) === 'ok')

    // The plugin turned on as well: every event would arrive twice.
    fs.writeFileSync(claudeSettings, JSON.stringify({ ...written, enabledPlugins: { [PLUGIN_ID]: true } }, null, 2))
    await settingsPatch({ bubble: true })
    await sleep(400)
    report('插件和旧 hooks 都在：提示重复', (await read('[data-connection]', 'dataset.connection')) === 'both' || (await state()) !== null)
    report('点「移除旧 hooks」', (await press('[data-action="remove-old"]')) === 'ok')
    await sleep(400)
    const after = JSON.parse(fs.readFileSync(claudeSettings, 'utf8'))
    report('只剩插件：settings.json 里我们的 hooks 删干净了', !after.hooks && after.enabledPlugins[PLUGIN_ID] === true)
    report('插件已启用', (await read('[data-plugin]', 'dataset.plugin')) === 'on')

    // --- The starter hands the session-start event on (the greeting).
    const S = { session_id: 'one', cwd: path.join(os.tmpdir(), 'alpha') }
    const started = await run([ROOT, ENSURE_FLAG], env, JSON.stringify({ ...S, hook_event_name: 'SessionStart', source: 'startup' }))
    await sleep(300)
    report('会话开始：启动命令把事件转给她，她打招呼', started.code === 0 && (await bubble()).includes('你好呀'), `${started.ms}ms · ${await bubble()}`)

    // --- Moods, one session.
    await hook({ ...S, hook_event_name: 'UserPromptSubmit', prompt: 'hi' })
    // The greeting (a wave, about 1.4 s) plays first.
    await sleep(1800)
    await snap('03-running')
    await expect('UserPromptSubmit', { mood: 'working' })
    report('干活时显示用时', /\d:\d\d/.test(await bubble()), await bubble())

    await hook({ ...S, hook_event_name: 'PreToolUse', tool_name: 'Bash' })
    await expect('PreToolUse', { mood: 'working', detail: 'Bash' })
    await hook({ ...S, hook_event_name: 'PostToolUseFailure', tool_name: 'Bash', error: 'exit 1' })
    await sleep(300)
    report('工具失败：闪一下「失败了」', (await bubble()).includes('Bash 失败了'), await bubble())

    // A prompt, answered from the main window this time.
    await tab('now')
    const fromHome = hook({ ...S, hook_event_name: 'PermissionRequest', tool_name: 'Bash', tool_input: { command: 'ls -la' } })
    await sleep(900)
    await expect('PermissionRequest', { mood: 'waiting', detail: 'Bash 需要你批准' })
    report('主窗口「现在」页：列出会话', (await read('[data-session="alpha"]', 'dataset.session')) === 'alpha')
    await snap('04-home-now-asking', 'settings')
    report('主窗口里点「允许」', (await press('[data-action="ask-allow"]')) === 'ok')
    report('回给 Claude 的是 allow', (await fromHome)?.hookSpecificOutput?.decision?.behavior === 'allow')

    await hook({ ...S, hook_event_name: 'TaskCompleted', task_subject: '写测试' })
    await sleep(250)
    report('任务完成：跳一下', (await bubble()).includes('完成：写测试'), await bubble())

    await hook({ ...S, hook_event_name: 'PostToolUse', tool_name: 'Edit' })
    await hook({ ...S, hook_event_name: 'Stop' })
    await sleep(400)
    await snap('05-review')
    await expect('Stop（改过文件）', { mood: 'review' })
    report('做完显示用时', (await bubble()).includes('用时'), await bubble())

    // --- Endings wait until you have seen them.
    await sleep(3000)
    await expect('3 秒后还在等你看', { mood: 'review' })
    await evaluate('pet', "document.getElementById('pet').dispatchEvent(new MouseEvent('mouseenter')), 'ok'")
    await sleep(200)
    await expect('鼠标经过她：算你看到了', { mood: 'idle' })
    await evaluate('pet', "document.getElementById('pet').dispatchEvent(new MouseEvent('mouseleave')), 'ok'")

    await hook({ ...S, hook_event_name: 'UserPromptSubmit', prompt: 'thanks' })
    await hook({ ...S, hook_event_name: 'Stop' })
    await sleep(400)
    await expect('Stop（没改文件）', { mood: 'done' })
    await hook({ ...S, hook_event_name: 'StopFailure', error: 'rate_limit' })
    await sleep(400)
    await snap('06-error-failed')
    await expect('StopFailure', { mood: 'error' })

    // --- Two sessions: each its own mood; the one that wants you is shown.
    const B = { session_id: 'two', cwd: path.join(os.tmpdir(), 'beta') }
    await hook({ ...S, hook_event_name: 'UserPromptSubmit', prompt: 'more' })
    await hook({ ...B, hook_event_name: 'UserPromptSubmit', prompt: 'go' })
    await hook({ ...B, hook_event_name: 'Stop' })
    await hook({ ...S, hook_event_name: 'PreToolUse', tool_name: 'Grep' })
    await sleep(400)
    await snap('07-two-sessions')
    const two = await bubble()
    report('两个会话：显示做完的 beta，并说另一个在忙', two.startsWith('beta：搞定啦') && two.includes('另有 1 个会话在忙'), JSON.stringify(two))
    report('主窗口列出两个会话', (await evaluate('settings', "document.querySelectorAll('[data-session]').length")) === 2)
    await hook({ ...B, hook_event_name: 'SessionEnd', reason: 'exit' })
    await hook({ ...S, hook_event_name: 'SessionEnd', reason: 'exit' })
    await sleep(300)
    await expect('SessionEnd', { mood: 'idle' })

    // --- Capsule mode.
    await settingsPatch({ display: 'capsule' })
    await sleep(500)
    const pill = (await state()).window
    report('胶囊模式：窗口变成小药丸', pill.size.width === 340 && pill.size.height === 64, `${pill.size.width}x${pill.size.height}`)
    await hook({ ...S, hook_event_name: 'UserPromptSubmit', prompt: 'pill' })
    await hook({ ...S, hook_event_name: 'PreToolUse', tool_name: 'Read' })
    await sleep(400)
    const pillText = await evaluate('pet', "document.getElementById('capsule-text').textContent")
    report('胶囊里一行字：状态和用时', /干活中… · Read · \d:\d\d/.test(pillText), pillText)
    await snap('08-capsule')
    const askInPill = hook({ ...S, hook_event_name: 'PermissionRequest', tool_name: 'Write', tool_input: { file_path: 'notes.md' } })
    await sleep(900)
    await snap('09-capsule-panel')
    report('胶囊上方弹出确认面板', (await evaluate('pet', "document.querySelector('#panel .title')?.textContent")) === 'Write 需要你批准')
    await click('[data-action="deny"]')
    report('胶囊模式下点「拒绝」', (await askInPill)?.hookSpecificOutput?.decision?.behavior === 'deny')
    await hook({ ...S, hook_event_name: 'Stop' })
    await settingsPatch({ display: 'pet' })
    await sleep(500)
    report('切回整只宠物', (await state()).window.size.height > 100)
    await evaluate('pet', "document.getElementById('pet').dispatchEvent(new MouseEvent('mouseenter')), 'ok'")
    await evaluate('pet', "document.getElementById('pet').dispatchEvent(new MouseEvent('mouseleave')), 'ok'")

    // --- Language: English from the Look tab.
    await tab('look')
    await evaluate('settings', "(() => { const s = document.querySelector('[data-key=\"lang\"]'); s.value = 'en'; s.dispatchEvent(new Event('change')); return 'ok' })()")
    await sleep(400)
    await hook({ ...S, hook_event_name: 'UserPromptSubmit', prompt: 'hi' })
    await sleep(300)
    report('切到英文：气泡', (await bubble()).startsWith('Working…'), await bubble())
    report('切到英文：主窗口', (await read('[data-tab="now"]')) === 'Now')
    await snap('10-home-look-english', 'settings')
    const askEn = hook({ ...S, hook_event_name: 'PermissionRequest', tool_name: 'Write', tool_input: { file_path: 'notes.md' } })
    await sleep(800)
    report('切到英文：面板', (await evaluate('pet', "document.querySelector('#panel .title').textContent")) === 'Write needs your approval')
    await click('[data-action="deny"]')
    report('切到英文：拒绝时留给 Claude 的话', (await askEn)?.hookSpecificOutput?.decision?.message === t('en', 'deny.message'))
    await settingsPatch({ lang: 'zh' })
    await hook({ ...S, hook_event_name: 'Stop' })
    await evaluate('pet', "document.getElementById('pet').dispatchEvent(new MouseEvent('mouseenter')), 'ok'")
    await evaluate('pet', "document.getElementById('pet').dispatchEvent(new MouseEvent('mouseleave')), 'ok'")

    // --- Quiet: do not disturb, and another app full screen.
    for (const [name, on, off] of [
      ['勿扰', () => settingsPatch({ dnd: true }), () => settingsPatch({ dnd: false })],
      ['全屏时', () => post('/debug/fullscreen', true), () => post('/debug/fullscreen', false)],
    ]) {
      await on()
      await sleep(300)
      const hidden = (await state()).window.isVisible === false
      const t0 = Date.now()
      const answer = await hook({ ...S, hook_event_name: 'PermissionRequest', tool_name: 'Bash', tool_input: { command: 'ls' } })
      const ms = Date.now() - t0
      report(`${name}：宠物藏起来，确认立刻交给终端`, hidden && JSON.stringify(answer) === '{}' && ms < 500, `${ms}ms`)
      await off()
      await sleep(300)
      report(`${name}结束：宠物回来`, (await state()).window.isVisible === true)
    }

    // --- Walking keeps the window's size and stays on screen.
    const start = (await state()).window
    for (let i = 0; i < 8; i++) await walkBy(i % 2 ? -300 : 300, 300)
    await walkBy(-5000, 300)
    checkWindow('来回走 8 趟、再往左走到底', (await state()).window, start)
    await walkBy(5000, 300)
    checkWindow('往右走到底', (await state()).window, start)

    // Started again: she comes home to the bottom right.
    await walkBy(-5000, 200)
    await run([ROOT], env)
    await sleep(500)
    const home = (await state()).window
    report(
      '再启动一次，回到右下角',
      home.bounds.x + home.size.width >= home.workArea.x + home.workArea.width - 30 && home.bounds.y + home.size.height >= home.workArea.y + home.workArea.height - 30,
      JSON.stringify(home.bounds),
    )

    // --- Prompts answered on the pet.
    const base = (await state()).window.bounds
    const bash = {
      ...S,
      cwd: ROOT,
      hook_event_name: 'PermissionRequest',
      tool_name: 'Bash',
      tool_input: { command: 'npm test -- --watch=false' },
      permission_suggestions: [{ type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: 'npm test:*' }], behavior: 'allow', destination: 'localSettings' }],
    }
    let pending = hook(bash)
    await sleep(250)
    report('授权面板：刚弹出时按钮还点不了', (await click('[data-action="always"]')) === 'disabled')
    await sleep(500)
    await snap('11-permission-panel')
    const grown = (await state()).window
    const petMoved = grown.bounds.x + grown.bounds.width / 2 + grown.shift - (base.x + base.width / 2)
    report('授权面板：窗口变大、宠物原地不动', grown.bounds.height > base.height && Math.abs(petMoved) <= 2, `${base.width}x${base.height} → ${grown.bounds.width}x${grown.bounds.height}`)
    report('授权面板：点「以后都允许」', (await click('[data-action="always"]')) === 'ok')
    const always = (await pending)?.hookSpecificOutput?.decision
    report('授权面板：回给 Claude 的是 allow + 规则', always?.behavior === 'allow' && always.updatedPermissions?.[0]?.rules?.[0]?.ruleContent === 'npm test:*')
    await sleep(300)
    const shrunk = (await state()).window.bounds
    report('授权面板：答完窗口恢复原样', shrunk.width === base.width && shrunk.height === base.height && shrunk.x === base.x && shrunk.y === base.y)

    pending = hook({
      ...S,
      hook_event_name: 'PermissionRequest',
      tool_name: 'AskUserQuestion',
      tool_input: {
        questions: [
          { header: '颜色', question: '气泡用什么颜色？', multiSelect: false, options: [{ label: '蓝', description: '冷静一点' }, { label: '粉', description: '可爱一点' }] },
          { header: '功能', question: '还要哪些功能？', multiSelect: true, options: [{ label: '走动' }, { label: '注视' }, { label: '气泡' }] },
          { header: '名字', question: '给她起个名字？', kind: 'text', placeholder: '比如：小鲸' },
          { header: '大小', question: '多大合适？', kind: 'number', min: 1, max: 10, defaultValue: 5, unit: '级' },
        ],
      },
    })
    await sleep(800)
    await snap('12-question-other')
    const type = text =>
      evaluate('pet', `(() => { const box = document.querySelector('[data-input="answer"]'); box.value = ${JSON.stringify(text)}; box.dispatchEvent(new Event('input', { bubbles: true })); return 'ok' })()`)
    await type('紫色')
    await sleep(100)
    report('选择题：「其他」里打字后点确定', (await click('[data-action="confirm"]')) === 'ok')
    await sleep(250)
    await click('[data-option="走动"]')
    await click('[data-option="气泡"]')
    await click('[data-action="confirm"]')
    await sleep(250)
    await type('小鲸')
    await click('[data-action="confirm"]')
    await sleep(250)
    await type('7')
    report('数字题：填 7 点确定', (await click('[data-action="confirm"]')) === 'ok')
    const answers = (await pending)?.hookSpecificOutput?.decision?.updatedInput?.answers
    report(
      '四道题的答案都回给了 Claude',
      answers?.['气泡用什么颜色？'] === '紫色' && answers?.['还要哪些功能？'] === '走动, 气泡' && answers?.['给她起个名字？'] === '小鲸' && answers?.['多大合适？'] === '7',
      JSON.stringify(answers),
    )

    pending = hook({ ...S, hook_event_name: 'PermissionRequest', tool_name: 'ExitPlanMode', tool_input: { plan: '1. 写代码\n2. 跑测试\n3. 推送' } })
    await sleep(800)
    await click('[data-action="deny"]')
    report('计划：点「拒绝」', (await pending)?.hookSpecificOutput?.decision?.behavior === 'deny')

    pending = hook(bash)
    await sleep(300)
    await hook({ ...S, hook_event_name: 'PostToolUse', tool_name: 'Bash', tool_input: bash.tool_input })
    report('在终端答了：面板收起、不替你做决定', JSON.stringify(await pending) === '{}' && (await state()).asks.length === 0)

    // --- Pets: the gallery and downloading from it.
    await tab('pets')
    report('宠物页：图库加载出来', await waitFor(async () => (await evaluate('settings', "document.querySelectorAll('[data-gallery]').length")) > 0), `${await evaluate('settings', "document.querySelectorAll('[data-gallery]').length")} 只`)
    await sleep(1500)
    await snap('13-home-pets', 'settings')
    const bad = await evaluate(
      'settings',
      "(async () => { const box = document.querySelector('[data-key=\"fetch\"]'); box.value = 'https://example.com/#/pets/x'; box.dispatchEvent(new Event('input')); document.querySelector('[data-action=\"fetch\"]').click(); await new Promise(r => setTimeout(r, 600)); return document.querySelector('[data-note]')?.textContent })()",
    )
    report('别的网站的地址被拒绝', /只支持 codex-pets\.net/.test(bad || ''), bad)
    const good = await evaluate(
      'settings',
      "(async () => { const box = document.querySelector('[data-key=\"fetch\"]'); box.value = 'https://codex-pets.net/#/pets/deepseek-chan'; box.dispatchEvent(new Event('input')); document.querySelector('[data-action=\"fetch\"]').click(); for (let i = 0; i < 60; i++) { await new Promise(r => setTimeout(r, 500)); const n = document.querySelector('[data-note]'); if (n && n.dataset.note !== 'busy') return n.textContent } return 'timeout' })()",
    )
    report('粘贴地址下载宠物', /已下载/.test(good || ''), good)
    report('下载的宠物存在自己的目录', fs.existsSync(path.join(profile, 'pets', 'deepseek-chan', 'spritesheet.webp')))

    // --- Eyes on the cursor: up, right, down, left of her face.
    let n = 14
    for (const [name, [dx, dy]] of Object.entries({ up: [0, -200], right: [200, 0], down: [0, 200], left: [-200, 0] })) {
      await post('/debug/look', { dx, dy })
      await sleep(150)
      await snap(`${n++}-look-${name}`)
    }

    // --- The SessionStart starter: quick and quiet while she is up...
    const whileUp = await run([ROOT, ENSURE_FLAG], env)
    report('会话开始时她已经在：启动器很快退出', whileUp.code === 0 && whileUp.ms < 5000, `${whileUp.ms}ms`)
  } finally {
    app.kill()
    await sleep(800)
  }

  // ...and starts her when she is not.
  try {
    report('窗口关掉了', await isDown())
    const whileDown = await run([ROOT, ENSURE_FLAG], env)
    const isUpAgain = await waitUp(15000)
    report('会话开始时她不在：启动器把她拉起来', whileDown.code === 0 && isUpAgain, `${whileDown.ms}ms`)
  } finally {
    await post('/debug/eval', { page: 'pet', code: "setTimeout(() => window.close(), 50), 'ok'" }).catch(() => {})
    await sleep(800)
    for (const dir of [profile, claudeDir]) fs.rmSync(dir, { recursive: true, force: true })
  }

  console.log(failures ? `\n${failures} 步不对` : `\n全部通过，截图在 ${OUT}`)
}

main()
  .then(() => process.exit(failures ? 1 : 0))
  .catch(err => {
    console.error(err.message)
    process.exit(1)
  })
