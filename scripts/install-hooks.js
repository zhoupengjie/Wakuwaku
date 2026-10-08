#!/usr/bin/env node
// Adds the claude-pets hooks to Claude Code's user settings, or removes them.
// For a copy run from source; the installed app does this from its settings.
//
//   npm run install-hooks              add (again: replaces the old entries)
//   npm run uninstall-hooks            remove
//   ... -- --http-only                 nothing starts the window for you
//                                      (use its start-at-login, or npm start)
//   ... -- --settings <path>           another settings file (default ~/.claude/settings.json)
//
// See src/shared/hooks-config.js for what goes in. Only our entries are
// touched; the file is backed up next to itself before the first change.
const fs = require('fs')
const os = require('os')
const path = require('path')

const { detectLang } = require('../src/shared/i18n')
const hooksConfig = require('../src/shared/hooks-config')

const ROOT = path.join(__dirname, '..')
const PORT = Number(process.env.CLAUDE_PETS_PORT || 47213)
const lang = detectLang(process.env.LANG || process.env.LC_ALL || Intl.DateTimeFormat().resolvedOptions().locale)

const SAY = {
  zh: {
    unreadable: '读不了 {file}：{message}',
    backup: '已备份原设置：{file}',
    removed: '已从 {file} 移除 claude-pets 的 hooks。',
    installed: '已写入 {file}：所有事件都通过 HTTP 发给 {url}（不开进程）。',
    starter: '会话开始时，在后台检查宠物有没有开，没开就启动：{command}',
    httpOnly: '不会自动启动宠物：请在宠物设置里打开「开机自动启动」，或者手动 npm start。',
    next: '新开的 Claude Code 会话生效。',
  },
  en: {
    unreadable: "Can't read {file}: {message}",
    backup: 'Backed up the old settings: {file}',
    removed: 'Removed the claude-pets hooks from {file}.',
    installed: 'Wrote {file}: every event goes over HTTP to {url} (no process).',
    starter: 'When a session starts, checks in the background that the pet is up, and starts it if not: {command}',
    httpOnly: 'Nothing will start the pet for you: turn on Start at login in its settings, or run npm start.',
    next: 'New Claude Code sessions pick this up.',
  },
}

function say(key, vars = {}) {
  return SAY[lang][key].replace(/\{(\w+)\}/g, (all, name) => (vars[name] !== undefined ? String(vars[name]) : all))
}

const args = process.argv.slice(2)
const isUninstall = args.includes('--uninstall')
const isHttpOnly = args.includes('--http-only')
const at = args.indexOf('--settings')
const file = at >= 0 ? path.resolve(args[at + 1]) : path.join(os.homedir(), '.claude', 'settings.json')

// This copy, run by its Electron, as the hook should start it.
const launch = { command: require('electron'), args: [ROOT] }

function read() {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'))
  } catch (err) {
    if (err.code === 'ENOENT') return {}
    console.error(say('unreadable', { file, message: err.message }))
    process.exit(1)
  }
}

const before = read()
const after = isUninstall ? hooksConfig.uninstall(before) : hooksConfig.install(before, { port: PORT, launch, httpOnly: isHttpOnly })

const backup = `${file}.claude-pets.bak`
if (fs.existsSync(file) && !fs.existsSync(backup)) {
  fs.copyFileSync(file, backup)
  console.log(say('backup', { file: backup }))
}
fs.mkdirSync(path.dirname(file), { recursive: true })
fs.writeFileSync(file, `${JSON.stringify(after, null, 2)}\n`)

if (isUninstall) {
  console.log(say('removed', { file }))
} else {
  console.log(say('installed', { file, url: hooksConfig.hookUrl(PORT) }))
  console.log(isHttpOnly ? say('httpOnly') : say('starter', { command: [launch.command, ...launch.args].join(' ') }))
  console.log(say('next'))
}
