const assert = require('node:assert/strict')
const { test } = require('node:test')

const { parsePetRef } = require('../scripts/fetch-pet')

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

test('links elsewhere, or with no pet in them, are refused in words', () => {
  const cases = [
    ['https://example.com/#/pets/deepseek-chan', /只支持 codex-pets\.net/],
    ['https://codex-pets.net.evil.com/#/pets/x', /只支持 codex-pets\.net/],
    ['https://codex-pets.net/', /没找到宠物 id/],
    ['https://codex-pets.net/#/creators/dullsaw', /没找到宠物 id/],
    ['https://codex-pets.net/#/pets/../../etc', /没找到宠物 id/],
    ['../evil', /看不懂|只支持|没找到/],
    ['', /看不懂|只支持|没找到/],
  ]
  for (const [ref, message] of cases) assert.throws(() => parsePetRef(ref), message, ref)
})
