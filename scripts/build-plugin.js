#!/usr/bin/env node
// Writes the Claude Code plugin in integrations/claude-code/ from
// src/agents/claude-code/hooks.js, so
// its hooks always match what the app expects. `npm test` checks they agree.
//
//   npm run build-plugin
//
// The plugin holds HTTP hooks only: Claude Code posts each event to the pet,
// no process, nothing written to anyone's settings. (A plugin cannot know
// where the app is installed, so it cannot start her; she starts at login.)
const fs = require('fs')
const path = require('path')

const { pluginHooks } = require('../src/agents/claude-code/hooks')
const pkg = require('../package.json')

// Third-party plugin names may not start with "claude-" (kept for Anthropic's own).
const PLUGIN_NAME = 'wakuwaku'
const MARKETPLACE_NAME = 'wakuwaku'

const ROOT = path.join(__dirname, '..')
const PLUGIN = path.join(ROOT, 'integrations', 'claude-code')

function write(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true })
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`)
}

function plugin() {
  return {
    name: PLUGIN_NAME,
    version: pkg.version,
    description: 'Tells the Wakuwaku desktop pet what Claude Code is doing, and lets you answer its prompts on her.',
    author: { name: pkg.author },
    homepage: pkg.homepage,
    repository: pkg.repository.url,
    license: pkg.license,
  }
}

function marketplace() {
  return {
    name: MARKETPLACE_NAME,
    owner: { name: pkg.author },
    description: 'A desktop pet for Claude Code: her hooks.',
    plugins: [{ name: PLUGIN_NAME, source: './integrations/claude-code', description: plugin().description }],
  }
}

function build() {
  write(path.join(PLUGIN, '.claude-plugin', 'plugin.json'), plugin())
  write(path.join(PLUGIN, 'hooks', 'hooks.json'), { hooks: pluginHooks({ port: 47213 }) })
  write(path.join(ROOT, '.claude-plugin', 'marketplace.json'), marketplace())
}

if (require.main === module) {
  build()
  console.log(`integrations/claude-code/ and .claude-plugin/marketplace.json written (${pkg.version}).`)
}

module.exports = { plugin, marketplace, PLUGIN_NAME, MARKETPLACE_NAME }
