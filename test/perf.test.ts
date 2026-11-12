// The performance run's table (tools/perf/report.js): medians, and B against
// A by docs/PERFORMANCE.md section 5's rules.
import assert from 'node:assert/strict'
import { test } from 'node:test'
import reportModule from '../tools/perf/report.js'

const { median, judge, report } = reportModule

test('medians: the middle run, or the two middle ones halved; runs without the number left out', () => {
  assert.equal(median([3, 1, 2]), 2)
  assert.equal(median([4, 1, 3, 2]), 2.5)
  assert.equal(median([null, 5, undefined]), 5)
  assert.equal(median([]), null)
})

test('the rules: 110% of CPU or half a ms/s more, 120% of wakeups, 30 MB more private, GPU only while quiet', () => {
  // 3 GHz: half a ms/s is 1.5 Mcyc/s.
  assert.equal(judge('mcycPerSec', 10, 11.4, true, 3).pass, true)
  assert.equal(judge('mcycPerSec', 10, 11.6, true, 3).pass, false)
  assert.equal(judge('mcycPerSec', 100, 109, true, 3).pass, true)
  assert.equal(judge('mcycPerSec', 100, 111, true, 3).pass, false)
  assert.equal(judge('msPerSec', 2, 2.4, true, 3).pass, true)
  assert.equal(judge('msPerSec', 2, 2.6, true, 3).pass, false)
  assert.equal(judge('wakeupsPerSec', 100, 119, true, 3).pass, true)
  assert.equal(judge('wakeupsPerSec', 100, 121, true, 3).pass, false)
  assert.equal(judge('privateMB', 300, 329, true, 3).pass, true)
  assert.equal(judge('privateMB', 300, 331, true, 3).pass, false)
  assert.equal(judge('gpuPct', 1, 1.3, true, 3).pass, false)
  assert.equal(judge('gpuPct', 1, 1.3, false, 3).pass, null)
  assert.equal(judge('gpuPct', 0, 0.05, true, 3).pass, true)
  assert.equal(judge('mcycPerSec', null, 1, true, 3).pass, null)
})

const run = (mcyc, extra = {}) => ({ totals: { mcycPerSec: mcyc, msPerSec: mcyc / 3, wakeupsPerSec: 50, privateMB: 200, workingSetMB: 400, gpuPct: 0 }, animations: { count: 0, disallowed: 0 }, ...extra })
const side = (label, scenes) => ({ label, machine: { name: 'test', mhz: 3000 }, scenes })

test('a table: each scene by medians, endless animations failing only a quiet scene, start-up by its own limits', () => {
  const A = side('A', {
    S1: { title: 'at rest', quiet: true, runs: [run(10), run(12), run(11)] },
    S3: { title: 'someone there', quiet: false, runs: [run(50)] },
    S7: { title: 'start-up', starts: [{ kind: 'warm', procToReady: 600 }, { kind: 'warm', procToReady: 640 }, { kind: 'cold', procToReady: 700 }] },
  })
  const B = side('B', {
    S1: { title: 'at rest', quiet: true, runs: [run(11), run(30), run(12, { animations: { count: 1, disallowed: 1 } })] },
    S3: { title: 'someone there', quiet: false, runs: [run(50, { animations: { count: 2, disallowed: 2 } })] },
    S7: { title: 'start-up', starts: [{ kind: 'warm', procToReady: 760 }, { kind: 'cold', procToReady: 990 }] },
  })
  const { rows, fails } = report(A, B)
  const row = (scene, metric) => rows.find(r => r.scene === scene && r.metric === metric)
  assert.equal(row('S1', 'Mcyc/s').a, '11.0')
  assert.equal(row('S1', 'Mcyc/s').b, '12.0')
  assert.equal(row('S1', 'Mcyc/s').pass, true)
  assert.equal(row('S1', 'endless animations').pass, false)
  assert.equal(row('S3', 'endless animations').pass, null)
  assert.equal(row('S7', 'warm: start → page ready ms').pass, false)
  assert.equal(row('S7', 'cold: start → page ready ms').pass, true)
  assert.deepEqual(fails, ['S1 endless animations', 'S7 warm start → page ready'])
})
