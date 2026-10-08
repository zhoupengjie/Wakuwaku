#!/usr/bin/env node
// End-to-end check: starts a real pet window on its own port and profile,
// drives it through hooks/claude-hook.js as Claude Code would, checks each
// state and saves a snapshot of each step to out/smoke/ for a look.
//
//   npm run smoke
const { spawn } = require('child_process')
const fs = require('fs')
const os = require('os')
const path = require('path')

const ROOT = path.join(__dirname, '..')
const HOOK = path.join(ROOT, 'hooks', 'claude-hook.js')
const OUT = path.join(ROOT, 'out', 'smoke')
const PORT = 47299
const URL = `http://127.0.0.1:${PORT}`

const { EVENTS } = require('../src/shared/hook-events')

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))

// Deliver an event the way the installed hooks do: most as an HTTP hook,
// SessionStart through the command.
async function hook(event) {
  if (EVENTS[event.hook_event_name]?.via !== 'http') return runCommand(event)
  const res = await fetch(`${URL}/hook?from=claude-pets`, { method: 'POST', body: JSON.stringify(event) })
  const body = await res.text()
  if (res.status !== 200 || body !== '{}') throw new Error(`/hook answered ${res.status} ${body}`)
}

function runCommand(event) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [HOOK], {
      env: { ...process.env, CLAUDE_PETS_PORT: String(PORT), CLAUDE_PETS_AUTOSTART: '0' },
      stdio: ['pipe', 'pipe', 'inherit'],
    })
    let out = ''
    child.stdout.on('data', chunk => (out += chunk))
    child.on('close', code => (code === 0 && out === '' ? resolve() : reject(new Error(`hook: exit ${code}, printed ${JSON.stringify(out)}`))))
    child.stdin.end(JSON.stringify(event))
  })
}

async function mood() {
  return (await (await fetch(`${URL}/health`)).json()).state
}

async function snap(name) {
  const png = Buffer.from(await (await fetch(`${URL}/snapshot`)).arrayBuffer())
  fs.writeFileSync(path.join(OUT, `${name}.png`), png)
}

async function look(dx, dy) {
  await fetch(`${URL}/debug/look`, { method: 'POST', body: JSON.stringify({ dx, dy }) })
}

// A prompt as Claude Code's HTTP hook sends it; resolves to the hook output.
async function askHook(event) {
  const res = await fetch(`${URL}/hook?from=claude-pets`, { method: 'POST', body: JSON.stringify(event) })
  return res.json()
}

async function click(selector) {
  const res = await fetch(`${URL}/debug/click`, { method: 'POST', body: JSON.stringify({ selector }) })
  return (await res.json()).result
}

async function walkBy(dx, ms) {
  await fetch(`${URL}/debug/walk`, { method: 'POST', body: JSON.stringify({ dx, ms }) })
}

let failures = 0
function report(step, ok, info) {
  if (!ok) failures += 1
  console.log(`${ok ? '✔' : '✘'} ${step}${info ? `: ${info}` : ''}`)
}

// The window is its own size (a pixel or two of rounding aside) and wholly
// on its display's work area.
function checkWindow(step, now, start) {
  const { bounds: b, size, workArea: a } = now
  const isSize = Math.abs(b.width - start.bounds.width) <= 2 && Math.abs(b.height - start.bounds.height) <= 2
  const isInside = b.x >= a.x && b.y >= a.y && b.x + size.width <= a.x + a.width && b.y + size.height <= a.y + a.height
  report(step, isSize && isInside, `${b.width}x${b.height} @ ${b.x},${b.y}（开始时 ${start.bounds.width}x${start.bounds.height}）`)
}

async function expect(step, want) {
  const { mood: got, detail } = await mood()
  const ok = got === want.mood && (want.detail === undefined || detail === want.detail)
  if (!ok) failures += 1
  console.log(`${ok ? '✔' : '✘'} ${step}: ${got}${detail ? ` · ${detail}` : ''}${ok ? '' : `（应为 ${want.mood}${want.detail ? ` · ${want.detail}` : ''}）`}`)
}

async function main() {
  if (!fs.existsSync(path.join(ROOT, 'pets', 'deepseek-chan', 'spritesheet.webp'))) {
    throw new Error('先运行 npm run fetch-pet')
  }
  fs.mkdirSync(OUT, { recursive: true })
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-pets-smoke-'))

  const electron = require(path.join(ROOT, 'node_modules', 'electron'))
  const appEnv = { ...process.env, CLAUDE_PETS_PORT: String(PORT), CLAUDE_PETS_USER_DATA: profile, CLAUDE_PETS_DEBUG: '1' }
  const app = spawn(electron, [ROOT], {
    env: appEnv,
    stdio: 'ignore',
  })

  try {
    for (let i = 0; ; i += 1) {
      if (i > 60) throw new Error('窗口 15 秒内没有起来')
      try {
        if ((await fetch(`${URL}/health`)).ok) break
      } catch {}
      await sleep(250)
    }
    await sleep(400)
    await snap('01-greeting-waving')
    await expect('打开窗口', { mood: 'idle' })

    await hook({ hook_event_name: 'UserPromptSubmit', prompt: 'hi' })
    await sleep(1600)
    await snap('02-running')
    await expect('UserPromptSubmit', { mood: 'working' })

    await hook({ hook_event_name: 'PreToolUse', tool_name: 'Bash' })
    await expect('PreToolUse', { mood: 'working', detail: 'Bash' })

    await hook({ hook_event_name: 'PostToolUseFailure', tool_name: 'Bash', error: 'exit 1' })
    await sleep(300)
    await snap('03-tool-failed')
    await expect('PostToolUseFailure', { mood: 'working', detail: 'Bash' })

    // A prompt now waits for an answer; leave it to the "terminal".
    const waiting = askHook({ hook_event_name: 'PermissionRequest', session_id: 'smoke', tool_name: 'Bash', tool_input: { command: 'ls' } })
    await sleep(1400)
    await snap('04-waiting')
    await expect('PermissionRequest', { mood: 'waiting', detail: 'Bash 需要你批准' })
    await click('[data-action="dismiss"]')
    report('「去终端处理」：不替你做决定', JSON.stringify(await waiting) === '{}')

    await hook({ hook_event_name: 'TaskCompleted', task_subject: '写测试' })
    await sleep(250)
    await snap('05-task-jumping')
    await expect('TaskCompleted', { mood: 'waiting' })

    await hook({ hook_event_name: 'PostToolUse', tool_name: 'Edit' })
    await hook({ hook_event_name: 'Stop' })
    await sleep(500)
    await snap('06-review')
    await expect('Stop（改过文件）', { mood: 'review' })

    await hook({ hook_event_name: 'UserPromptSubmit', prompt: 'thanks' })
    await hook({ hook_event_name: 'Stop' })
    await sleep(500)
    await snap('07-done-waving')
    await expect('Stop（没改文件）', { mood: 'done' })

    await hook({ hook_event_name: 'StopFailure', error: 'rate_limit' })
    await sleep(500)
    await snap('08-error-failed')
    await expect('StopFailure', { mood: 'error' })

    await hook({ hook_event_name: 'SessionStart', source: 'compact' })
    await expect('SessionStart（compact）不打断', { mood: 'error' })

    await hook({ hook_event_name: 'SessionEnd', reason: 'exit' })
    await sleep(300)
    await expect('SessionEnd', { mood: 'idle' })

    // Walking: the window keeps its size and stays on screen. (On Windows at
    // 125% it used to grow a pixel per step and carry the pet off the screen.)
    const start = (await mood()).window
    for (let i = 0; i < 8; i++) await walkBy(i % 2 ? -300 : 300, 300)
    await walkBy(-5000, 300)
    checkWindow('来回走 8 趟、再往左走到底', (await mood()).window, start)
    await walkBy(5000, 300)
    checkWindow('往右走到底', (await mood()).window, start)

    // Started again: she comes home to the bottom right.
    await walkBy(-5000, 200)
    const again = spawn(electron, [ROOT], { env: appEnv, stdio: 'ignore' })
    await new Promise(resolve => again.on('exit', resolve))
    await sleep(500)
    const home = (await mood()).window
    const isHome =
      home.bounds.x + home.size.width >= home.workArea.x + home.workArea.width - 30 &&
      home.bounds.y + home.size.height >= home.workArea.y + home.workArea.height - 30
    report('再启动一次，回到右下角', isHome, JSON.stringify(home.bounds))

    // Prompts answered on the pet.
    const base = (await mood()).window.bounds
    const bash = {
      hook_event_name: 'PermissionRequest',
      session_id: 'smoke',
      cwd: ROOT,
      tool_name: 'Bash',
      tool_input: { command: 'npm test -- --watch=false' },
      permission_suggestions: [
        { type: 'addRules', rules: [{ toolName: 'Bash', ruleContent: 'npm test:*' }], behavior: 'allow', destination: 'localSettings' },
      ],
    }
    let pending = askHook(bash)
    await sleep(250)
    report('授权面板：刚弹出时按钮还点不了', (await click('[data-action="always"]')) === 'disabled')
    await sleep(500)
    await snap('13-permission-panel')
    const grown = (await mood()).window.bounds
    const { shift } = (await mood()).window
    const petMoved = grown.x + grown.width / 2 + shift - (base.x + base.width / 2)
    report('授权面板：窗口变大、宠物原地不动', grown.height > base.height && Math.abs(petMoved) <= 2 && Math.abs(grown.y + grown.height - (base.y + base.height)) <= 2, `${base.width}x${base.height} → ${grown.width}x${grown.height}，宠物偏移 ${petMoved}px（窗口内挪了 ${shift}px）`)
    report('授权面板：点「以后都允许」', (await click('[data-action="always"]')) === 'ok')
    let out = await pending
    const always = out?.hookSpecificOutput?.decision
    report('授权面板：回给 Claude 的是 allow + 规则', always?.behavior === 'allow' && always.updatedPermissions?.[0]?.rules?.[0]?.ruleContent === 'npm test:*', JSON.stringify(always))
    await sleep(300)
    const shrunk = (await mood()).window.bounds
    report('授权面板：答完窗口恢复原样', shrunk.width === base.width && shrunk.height === base.height && shrunk.x === base.x && shrunk.y === base.y, `${shrunk.width}x${shrunk.height} @ ${shrunk.x},${shrunk.y}`)

    pending = askHook({
      hook_event_name: 'PermissionRequest',
      session_id: 'smoke',
      cwd: ROOT,
      tool_name: 'AskUserQuestion',
      tool_input: {
        questions: [
          { header: '颜色', question: '气泡用什么颜色？', multiSelect: false, options: [{ label: '蓝', description: '冷静一点' }, { label: '粉', description: '可爱一点' }] },
          { header: '功能', question: '还要哪些功能？', multiSelect: true, options: [{ label: '走动' }, { label: '注视' }, { label: '气泡' }] },
        ],
      },
    })
    await sleep(800)
    await snap('14-question-panel')
    report('选择题：单选点「蓝」', (await click('[data-option="蓝"]')) === 'ok')
    await sleep(250)
    await click('[data-option="走动"]')
    await click('[data-option="气泡"]')
    await sleep(100)
    await snap('15-question-multiselect')
    report('选择题：多选后点「确定」', (await click('[data-action="confirm"]')) === 'ok')
    out = await pending
    const answers = out?.hookSpecificOutput?.decision?.updatedInput?.answers
    report('选择题：答案按题目文字回给 Claude', answers?.['气泡用什么颜色？'] === '蓝' && answers?.['还要哪些功能？'] === '走动, 气泡', JSON.stringify(answers))

    pending = askHook({ hook_event_name: 'PermissionRequest', session_id: 'smoke', cwd: ROOT, tool_name: 'ExitPlanMode', tool_input: { plan: '1. 写代码\n2. 跑测试\n3. 推送' } })
    await sleep(800)
    await snap('16-plan-panel')
    await click('[data-action="deny"]')
    out = await pending
    report('计划：点「拒绝」', out?.hookSpecificOutput?.decision?.behavior === 'deny')

    pending = askHook(bash)
    await sleep(300)
    await hook({ hook_event_name: 'PostToolUse', session_id: 'smoke', tool_name: 'Bash', tool_input: bash.tool_input })
    out = await pending
    report('在终端答了：面板收起、不替你做决定', JSON.stringify(out) === '{}' && (await mood()).asks.length === 0)

    // Eyes on the cursor: up, right, down, left of her face.
    const looks = { up: [0, -200], right: [200, 0], down: [0, 200], left: [-200, 0] }
    let n = 9
    for (const [name, [dx, dy]] of Object.entries(looks)) {
      await look(dx, dy)
      await sleep(150)
      await snap(`${String(n++).padStart(2, '0')}-look-${name}`)
    }

    console.log(failures ? `\n${failures} 步不对` : `\n全部通过，截图在 ${OUT}`)
  } finally {
    app.kill()
    await sleep(500)
    fs.rmSync(profile, { recursive: true, force: true })
  }
}

main()
  .then(() => process.exit(failures ? 1 : 0))
  .catch(err => {
    console.error(err.message)
    process.exit(1)
  })
