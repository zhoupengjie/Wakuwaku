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

    await hook({ hook_event_name: 'PermissionRequest', tool_name: 'Bash' })
    await sleep(1400)
    await snap('04-waiting')
    await expect('PermissionRequest', { mood: 'waiting', detail: 'Bash 需要你批准' })

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
