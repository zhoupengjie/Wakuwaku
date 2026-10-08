const assert = require('node:assert/strict')
const { test } = require('node:test')

const { parsePetRef, describe } = require('../src/shared/pet-fetch')

test('a pet is found from its id or any codex-pets.net link to it', () => {
  const cases = [
    'deepseek-chan',
    '  deepseek-chan  ',
    'https://codex-pets.net/#/pets/deepseek-chan',
    'https://codex-pets.net/#/pets/deepseek-chan/',
    'https://codex-pets.net/#/pets/deepseek-chan?from=share',
    'http://codex-pets.net/#/pets/deepseek-chan',
    'codex-pets.net/#/pets/deepseek-chan',
    'https://www.codex-pets.net/#/pets/deepseek-chan',
    'https://codex-pets.net/api/pets/deepseek-chan',
    'https://codex-pets.net/api/pets/deepseek-chan/download?v=1791020401654',
    'https://codex-pets.net/assets/pets/v/1791020401654/deepseek-chan/spritesheet.webp',
    'https://codex-pets.net/pets/deepseek-chan',
  ]
  for (const ref of cases) assert.equal(parsePetRef(ref), 'deepseek-chan', ref)
})

test('links elsewhere, or with no pet in them, are refused, in either language', () => {
  const cases = [
    ['https://example.com/#/pets/deepseek-chan', 'otherSite', /只支持 codex-pets.net/, /Only pets on codex-pets.net/],
    ['https://codex-pets.net.evil.com/#/pets/x', 'otherSite', /只支持/, /Only pets/],
    ['https://codex-pets.net/', 'noId', /没找到宠物 id/, /No pet id/],
    ['https://codex-pets.net/#/creators/dullsaw', 'noId', /没找到/, /No pet id/],
    ['https://codex-pets.net/#/pets/../../etc', 'noId', /没找到/, /No pet id/],
  ]
  for (const [ref, code, zh, en] of cases) {
    assert.throws(() => parsePetRef(ref), err => err.code === code && zh.test(describe('zh', err)) && en.test(describe('en', err)), ref)
  }
  for (const ref of ['../evil', '']) assert.throws(() => parsePetRef(ref), err => typeof err.code === 'string', ref)
})
