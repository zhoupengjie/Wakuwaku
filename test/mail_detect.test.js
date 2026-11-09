const assert = require('node:assert')
const { test, describe } = require('node:test')
const { findCode, findLink, detectMailNotice } = require('../src/shared/mail-engine.js')

describe('Verification Code Detection', () => {
  test('standard and formatted codes (Chinese/English, full-width, spaced, hyphenated, alphanumeric)', () => {
    assert.strictEqual(findCode('', '【GitHub】您的验证码为 482910，请在 10 分钟内完成验证。'), '482910')
    assert.strictEqual(findCode('', '【腾讯云】验证码：６５４３２１，用于登录验证。'), '654321')
    assert.strictEqual(findCode('', '动态密码 123 456，请勿泄露。'), '123456')
    assert.strictEqual(findCode('', 'Your security code is 482-910'), '482910')
    assert.strictEqual(findCode('', '482910 是您的验证码，5分钟内有效。'), '482910')
    assert.strictEqual(findCode('', 'Your verification code is A1B2C3.'), 'A1B2C3')
    assert.strictEqual(findCode('Your verification code', 'Hi,\n\n849201\n\nThanks.'), '849201')
  })

  test('cloud account verification with 账号/手机 context is not falsely rejected', () => {
    assert.strictEqual(
      findCode('', '【阿里云】验证码：582910。您正在为账号 123@aliyun.com 绑定手机，如非本人操作请忽略。'),
      '582910'
    )
  })

  test('rejects non-verification numbers (orders, promos, phones, zip, years, amounts, URLs)', () => {
    assert.strictEqual(findCode('', '您的订单号 20260918 已经发货，请注意查收。'), null)
    assert.strictEqual(findCode('Your verification code', 'Your order 12345678 has shipped'), null)
    assert.strictEqual(findCode('', 'Enter promo code SAVE20 at checkout for 20% off!'), null)
    assert.strictEqual(findCode('', 'Call us at 13800138000 or zip code 100080.'), null)
    assert.strictEqual(findCode('', 'The verification code era in 2026 is here.'), null)
    assert.strictEqual(findCode('', 'Your invoice total is $1,234.56.'), null)
    assert.strictEqual(findCode('', 'Your code is at https://example.com/item/482910'), null)
  })
})

describe('Verification Link Detection', () => {
  test('detects magic links, email confirmation, and HTML anchors', () => {
    const notion = 'Click the link below to sign in:\nhttps://www.notion.so/login/confirm?token=9f8e7d6c5b4a3210fe87c2b1a0'
    assert.strictEqual(findLink('Log in to Notion', notion), 'https://www.notion.so/login/confirm?token=9f8e7d6c5b4a3210fe87c2b1a0')

    const xUrl = 'https://x.com/account/verify_email?token=8f9a0b1c2d3e4f5a6b7c'
    assert.strictEqual(findLink('Confirm your X account', xUrl), xUrl)

    const html = '<p>Please confirm:</p><A HREF = "https://example.com/confirm?token=48f7e6d5c4b3a210fedc&amp;u=1">Verify Email Address</A>'
    assert.strictEqual(findLink('Confirm your email', '', html), 'https://example.com/confirm?token=48f7e6d5c4b3a210fedc&u=1')
  })

  test('rejects unsubscribe, privacy policy, tracking pixels, and empty hrefs', () => {
    assert.strictEqual(findLink('Weekly Digest', 'To unsubscribe: https://news.com/unsubscribe?token=9f8e7d6c5b4a3210fe'), null)
    assert.strictEqual(findLink('Policy Update', 'Read at https://example.com/privacy-policy'), null)
    assert.strictEqual(findLink('Verify', '', '<a href="">click</a><img src="https://track.com/open.gif?id=123">'), null)
  })
})

describe('Disambiguation & Priority', () => {
  test('prioritizes verification code over link when both exist', () => {
    const sub = 'Verify your account'
    const body = 'Your verification code is 482910. Or click:\nhttps://example.com/verify?token=9f8e7d6c5b4a3210fe'
    assert.deepStrictEqual(detectMailNotice(sub, body), { kind: 'code', code: '482910' })
  })

  test('returns formatted displayUrl for link-only notice and null for normal mail', () => {
    const res = detectMailNotice('Log in to Notion', 'Sign in:\nhttps://www.notion.so/login/confirm?token=9f8e7d6c5b4a3210fe87c2b1a0')
    assert.strictEqual(res.kind, 'link')
    assert.ok(res.displayUrl.length <= 40)
    assert.strictEqual(detectMailNotice('Dinner tonight?', 'See you at 7pm at https://restaurant.com'), null)
  })
})
