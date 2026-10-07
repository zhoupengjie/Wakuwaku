#!/usr/bin/env node
// Adds the claude-pets hook to Claude Code's user settings, or removes it.
//
//   npm run install-hooks              add (again: replaces the old entries)
//   npm run uninstall-hooks            remove
//   ... -- --settings <path>           another settings file (default ~/.claude/settings.json)
//
// Only entries whose command runs hooks/claude-hook.js are touched; the file
// is backed up next to itself before the first change.
const fs = require('fs')
const os = require('os')
const path = require('path')

const HOOK = path.join(__dirname, '..', 'hooks', 'claude-hook.js').replaceAll('\\', '/')
const COMMAND = `node "${HOOK}"`
const MARK = 'claude-hook.js'

// The events the hook takes; the tool events take a matcher.
const { EVENTS } = require('../hooks/claude-hook')

const args = process.argv.slice(2)
const isUninstall = args.includes('--uninstall')
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
      .map(group => ({ ...group, hooks: (group.hooks || []).filter(h => !String(h.command || '').includes(MARK)) }))
      .filter(group => group.hooks.length > 0)
    if (kept.length) hooks[event] = kept
    else delete hooks[event]
  }
}

const settings = read()
const hooks = settings.hooks && typeof settings.hooks === 'object' ? settings.hooks : {}
strip(hooks)

if (!isUninstall) {
  for (const [event, hasMatcher] of Object.entries(EVENTS)) {
    const entry = { type: 'command', command: COMMAND, timeout: 5 }
    hooks[event] = [...(hooks[event] || []), hasMatcher ? { matcher: '*', hooks: [entry] } : { hooks: [entry] }]
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

console.log(
  isUninstall
    ? `已从 ${file} 移除 claude-pets 的 hooks。`
    : `已写入 ${file}：${Object.keys(EVENTS).join('、')} → ${COMMAND}\n新开的 Claude Code 会话生效。`,
)
