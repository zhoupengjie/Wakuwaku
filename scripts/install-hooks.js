#!/usr/bin/env node
// Adds the claude-pets hooks to Claude Code's user settings, or removes them.
//
//   npm run install-hooks              add (again: replaces the old entries)
//   npm run uninstall-hooks            remove
//   ... -- --http-only                 SessionStart too: no process at all, but
//                                      nothing starts the window for you (use
//                                      its 开机自动启动, or npm start)
//   ... -- --settings <path>           another settings file (default ~/.claude/settings.json)
//
// Most events are HTTP hooks: Claude Code POSTs them to the window itself, so
// a tool call starts no process. SessionStart runs hooks/claude-hook.js in the
// background (async: the session does not wait on it), to start the window
// when it is not up, which no HTTP hook can do.
//
// Only our entries are touched (an HTTP hook to ...?from=claude-pets, or a
// command running claude-hook.js, as older versions installed); the file is
// backed up next to itself before the first change.
const fs = require('fs')
const os = require('os')
const path = require('path')

const { EVENTS } = require('../src/shared/hook-events')

const PORT = Number(process.env.CLAUDE_PETS_PORT || 47213)
const HOOK = path.join(__dirname, '..', 'hooks', 'claude-hook.js').replaceAll('\\', '/')
const URL = `http://127.0.0.1:${PORT}/hook?from=claude-pets`

const ENTRY = {
  http: { type: 'http', url: URL, timeout: 2 },
  command: { type: 'command', command: `node "${HOOK}"`, async: true, timeout: 10 },
}

function isOurs(hook) {
  return String(hook.command || '').includes('claude-hook.js') || String(hook.url || '').includes('from=claude-pets')
}

const args = process.argv.slice(2)
const isUninstall = args.includes('--uninstall')
const isHttpOnly = args.includes('--http-only')
const at = args.indexOf('--settings')
const file = at >= 0 ? path.resolve(args[at + 1]) : path.join(os.homedir(), '.claude', 'settings.json')

function read() {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'))
  } catch (err) {
    if (err.code === 'ENOENT') return {}
    console.error(`读不了 ${file}：${err.message}`)
    process.exit(1)
  }
}

// Drop our entries from every event, and events left empty.
function strip(hooks) {
  for (const [event, groups] of Object.entries(hooks)) {
    if (!Array.isArray(groups)) continue
    const kept = groups
      .map(group => ({ ...group, hooks: (group.hooks || []).filter(h => !isOurs(h)) }))
      .filter(group => group.hooks.length > 0)
    if (kept.length) hooks[event] = kept
    else delete hooks[event]
  }
}

const settings = read()
const hooks = settings.hooks && typeof settings.hooks === 'object' ? settings.hooks : {}
strip(hooks)

if (!isUninstall) {
  for (const [event, { matcher, via }] of Object.entries(EVENTS)) {
    const group = { hooks: [ENTRY[isHttpOnly ? 'http' : via]] }
    hooks[event] = [...(hooks[event] || []), matcher ? { matcher: '*', ...group } : group]
  }
}

if (Object.keys(hooks).length) settings.hooks = hooks
else delete settings.hooks

const backup = `${file}.claude-pets.bak`
if (fs.existsSync(file) && !fs.existsSync(backup)) {
  fs.copyFileSync(file, backup)
  console.log(`已备份原设置：${backup}`)
}
fs.mkdirSync(path.dirname(file), { recursive: true })
fs.writeFileSync(file, `${JSON.stringify(settings, null, 2)}\n`)

if (isUninstall) {
  console.log(`已从 ${file} 移除 claude-pets 的 hooks。`)
} else {
  const by = via =>
    Object.entries(EVENTS)
      .filter(([, e]) => (isHttpOnly ? 'http' : e.via) === via)
      .map(([name]) => name)
      .join('、')
  console.log(`已写入 ${file}：`)
  console.log(`  HTTP（不开进程）→ ${URL}：${by('http')}`)
  if (isHttpOnly) {
    console.log('  全部走 HTTP：窗口不会再被自动启动，请在宠物右键菜单里打开「开机自动启动」，或者手动 npm start。')
  } else {
    console.log(`  后台命令（async，不阻塞）→ node "${HOOK}"：${by('command')}`)
  }
  console.log('新开的 Claude Code 会话生效。')
}
