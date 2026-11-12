// The strict check (npm run typecheck): tsc over tsconfig.strict.json, its
// listed files with strict on. What they import comes into the check too
// (tsc has no way to check one file alone), and the errors in those files
// are left out: only the listed files have to pass. Any error that is not
// in a page's file (the config, an option) fails it all the same.
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'

const ROOT = path.join(import.meta.dirname, '..')
const CONFIG = 'tsconfig.strict.json'
// tsconfig allows comments: whole lines of them here.
const config = JSON.parse(
  fs
    .readFileSync(path.join(ROOT, CONFIG), 'utf8')
    .split(/\r?\n/)
    .filter(line => !/^\s*\/\//.test(line))
    .join('\n'),
)
const listed = new Set(config.include.map(f => path.posix.normalize(f)))
const tsc = path.join(ROOT, 'node_modules', 'typescript', 'bin', 'tsc')
const run = spawnSync(process.execPath, [tsc, '-p', CONFIG, '--pretty', 'false'], { cwd: ROOT, encoding: 'utf8' })
if (run.error) throw run.error

// Each error: its first line (file(line,col): error TS…) and the lines under it.
const errors = []
for (const line of `${run.stdout}${run.stderr}`.split(/\r?\n/)) {
  if (!line.trim()) continue
  if (/^\s/.test(line) && errors.length) errors[errors.length - 1].text += `\n${line}`
  else errors.push({ file: (/^(.+?)\(\d+,\d+\): /.exec(line)?.[1] || '').replace(/\\/g, '/'), text: line })
}
const ours = errors.filter(e => listed.has(e.file) || !e.file.startsWith('src/'))
const left = errors.length - ours.length
for (const e of ours) console.log(e.text)
if (ours.length) {
  console.log(`\nstrict: ${ours.length} error(s) in the files ${CONFIG} lists`)
  process.exit(1)
}
if (run.status !== 0 && !errors.length) {
  console.log(`strict: tsc failed with nothing to say (${run.status})`)
  process.exit(1)
}
console.log(`strict: the ${listed.size} files ${CONFIG} lists pass (${left} error(s) left out, in files they import)`)
