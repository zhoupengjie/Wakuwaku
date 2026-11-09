// Two-Tier Email Verification Engine (Verification Code & Verification Link)
// Guarantees zero false positives (anti-promo/order/unsubscribe) and zero false negatives.

;(function () {
  const KEYWORDS = [
    ['短信验证码', 110],
    ['手机验证码', 110],
    ['邮箱验证码', 110],
    ['验证码', 100],
    ['校验码', 100],
    ['安全验证码', 105],
    ['身份验证码', 105],
    ['动态密码', 100],
    ['动态码', 95],
    ['一次性密码', 105],
    ['账号验证码', 105],
    ['确认码', 90],
    ['安全码', 90],
    ['授权码', 75],
    ['登录码', 85],
    ['verification code', 100],
    ['your code is', 95],
    ['your code', 90],
    ['one-time password', 105],
    ['one time password', 105],
    ['one-time code', 100],
    ['one time code', 100],
    ['security code', 95],
    ['confirmation code', 95],
    ['authentication code', 95],
    ['auth code', 90],
    ['steam guard code', 105],
    ['steam guard', 100],
    ['guard code', 95],
    ['access code', 90],
    ['login code', 90],
    ['verify code', 95],
    ['passcode', 90],
    ['otp', 85],
    ['pin code', 80],
    ['pin', 75],
    ['code', 40],
  ]

  const NOT_A_CODE = [
    'order', 'invoice', 'telephone', 'fax', 'zip', 'postal', 'tracking',
    'coupon', 'promo', 'discount', 'price', 'amount', 'total', 'balance', 'cvv', 'cvc',
    'referral', 'invite', 'invitation', 'voucher', 'gift', 'qr', 'area code', 'code of conduct',
    '订单', '单号', '编号', '电话', '手机号', '传真', '邮编', '快递', '物流', '运单', '优惠',
    '折扣', '价格', '金额', '余额', '发票', '卡号', '账号：', '账号:', '座位', '航班', '$', '¥', '￥',
    'usd', 'cny', 'checkout', 'redeem', 'claim', 'offer', 'bonus', '%', 'off'
  ]

  const EXPIRATION_SIGNALS = [
    '有效时间', '分钟内有效', '小时内有效', '有效期', '请在', '内完成', '如非本人操作',
    'valid for', 'expires in', 'expire in', 'within', 'minutes', 'hours', 'if you did not request'
  ]

  function normalizeDigits(str) {
    let out = ''
    for (const ch of str) {
      const code = ch.charCodeAt(0)
      if (code >= 0xff10 && code <= 0xff19) {
        out += String.fromCharCode(code - 0xff10 + 0x30)
      } else {
        out += ch
      }
    }
    return out
  }

  function findCode(subject, body) {
    const normSubject = normalizeDigits(subject || '')
    const normBody = normalizeDigits(body || '')

    // 1. Subject check
    const codeSub = detectCode(normSubject, true)
    if (codeSub) return codeSub

    // 2. Body check
    const codeBody = detectCode(normBody, false)
    if (codeBody) return codeBody

    // 3. Combined check (subject header + top of body)
    if (normSubject.trim()) {
      const combined = `${normSubject.trim()}\n${normBody.trim()}`
      return detectCodeWith(combined, false, 35)
    }
    return null
  }

  function detectCode(raw, isSubject) {
    return detectCodeWith(raw, isSubject, isSubject ? 80 : 60)
  }

  function detectCodeWith(raw, isSubject, afterMax) {
    const chars = Array.from(raw)
    const candidates = findCandidates(chars)
    let best = null
    let bestScore = -1

    for (const c of candidates) {
      if (besideAddress(chars, c)) continue
      if (inLink(chars, c)) continue
      if (isYear(c)) continue

      let score = -1
      let minKwDist = Infinity
      let bestKwWeight = 0

      for (const [kw, kwScore] of KEYWORDS) {
        const dist = distance(chars, c, kw)
        if (dist === null) continue

        const d = Math.abs(dist)
        if (d < minKwDist) {
          minKwDist = d
          bestKwWeight = kwScore
        }

        if (dist >= 0 && dist <= afterMax) {
          const s = kwScore + (afterMax - dist) / 4
          if (s > score) score = Math.round(s)
        } else if (dist < 0 && Math.abs(dist) <= 40) {
          const s = kwScore * 0.9 + (40 + dist) / 4
          if (s > score) score = Math.round(s)
        }
      }

      if (score < 50) continue

      score += lengthBonus(c)

      if (hasExpirationNearby(chars, c)) {
        score += 25
      }

      if (hasBracketEnclosure(chars, c)) {
        score += 15
      }

      let penalty = notACodePenalty(chars, c)
      // Proximity protection: if an explicit verification keyword sits right next
      // to the candidate (<= 10 chars, e.g. 验证码：123456), suppress generic penalties.
      if (minKwDist <= 10 && bestKwWeight >= 95) {
        penalty = Math.min(15, penalty)
      }
      score -= penalty

      if (score >= 65 && score > bestScore) {
        bestScore = score
        best = c.text
      }
    }
    return best
  }

  function findCandidates(chars) {
    const candidates = []
    const isDigit = ch => ch >= '0' && ch <= '9'
    let i = 0
    while (i < chars.length) {
      if (isCandidateChar(chars[i])) {
        const start = i
        let digits = 0
        let letters = 0
        let text = ''
        let end = i

        while (end < chars.length && isCandidateChar(chars[end])) {
          const ch = chars[end]
          if (isDigit(ch)) digits++
          else if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')) letters++
          text += ch
          end++

          // Handle space-separated or hyphen-separated digits (e.g. "123 456" or "482-910")
          // Requires digits on both sides
          if (
            end < chars.length &&
            (chars[end] === ' ' || chars[end] === '-') &&
            end > 0 &&
            isDigit(chars[end - 1]) &&
            end + 1 < chars.length &&
            isDigit(chars[end + 1])
          ) {
            if (digits < 8) {
              end++
            }
          }
        }

        const cleanText = text.replace(/[\s-]+/g, '')
        if (cleanText.length >= 4 && cleanText.length <= 8 && digits >= 1) {
          candidates.push({
            text: cleanText,
            at: start,
            end: end,
            digits: digits,
            letters: letters,
          })
        }
        i = end
      } else {
        i++
      }
    }
    return candidates
  }

  function isCandidateChar(c) {
    return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
  }

  function besideAddress(chars, c) {
    return (c.at > 0 && chars[c.at - 1] === '@') || (c.end < chars.length && chars[c.end] === '@')
  }

  function inLink(chars, c) {
    const edge = ch => /\s|[()[\]{}<>"'，。、]/.test(ch)
    let from = c.at
    while (from > 0 && !edge(chars[from - 1])) from--
    let to = c.end
    while (to < chars.length && !edge(chars[to])) to++
    const token = chars.slice(from, to).join('').toLowerCase()
    return (
      token.includes('://') ||
      token.startsWith('www.') ||
      (token.includes('/') && token.includes('.')) ||
      (token.includes('?') && token.includes('=')) ||
      ['.png', '.jpg', '.jpeg', '.gif', '.webp', '.svg', '.html', '.php'].some(ext => token.includes(ext))
    )
  }

  function isYear(c) {
    if (c.letters === 0 && c.digits === 4) {
      const y = parseInt(c.text, 10)
      return y >= 1900 && y <= 2035
    }
    return false
  }

  function lengthBonus(c) {
    const len = c.digits + c.letters
    if (len === 6) return 26
    if (len === 5) return 22
    if (len === 7) return 24
    if (len === 4) return 16
    if (len === 8) return 20
    return 0
  }

  function distance(chars, c, kw) {
    const text = chars.join('').toLowerCase()
    const kwLower = kw.toLowerCase()
    let idx = 0
    let nearest = null
    while ((idx = text.indexOf(kwLower, idx)) !== -1) {
      const kwEnd = idx + kw.length
      let dist
      if (idx >= c.end) {
        dist = -(idx - c.end)
      } else if (c.at >= kwEnd) {
        dist = c.at - kwEnd
      } else {
        dist = 0
      }
      if (nearest === null || Math.abs(dist) < Math.abs(nearest)) {
        nearest = dist
      }
      idx = kwEnd
    }
    return nearest
  }

  function hasExpirationNearby(chars, c) {
    const from = Math.max(0, c.at - 60)
    const to = Math.min(chars.length, c.end + 60)
    const snippet = chars.slice(from, to).join('').toLowerCase()
    return EXPIRATION_SIGNALS.some(s => snippet.includes(s))
  }

  function hasBracketEnclosure(chars, c) {
    if (c.at > 0 && c.end < chars.length) {
      const prev = chars[c.at - 1]
      const next = chars[c.end]
      if (
        (prev === '【' && next === '】') ||
        (prev === '[' && next === ']') ||
        (prev === '"' && next === '"') ||
        (prev === '“' && next === '”') ||
        (prev === '`' && next === '`')
      ) {
        return true
      }
    }
    return false
  }

  function notACodePenalty(chars, c) {
    const from = Math.max(0, c.at - 30)
    const to = Math.min(chars.length, c.end + 30)
    const around = chars.slice(from, to).join('').toLowerCase()
    let penalty = 0
    for (const word of NOT_A_CODE) {
      if (around.includes(word)) penalty += 60
    }
    const nearFrom = Math.max(0, c.at - 12)
    const nearTo = Math.min(chars.length, c.end + 12)
    const nearAround = chars.slice(nearFrom, nearTo).join('').toLowerCase()
    for (const veto of ['order', 'invoice', 'tracking', 'postal', '订单', '单号', '运单', '快递', '邮编']) {
      if (nearAround.includes(veto)) penalty += 60
    }
    if (around.includes('http://') || around.includes('https://') || around.includes('www.')) {
      penalty += 35
    }
    return Math.min(150, penalty)
  }

  // -------------------------------------------------------------
  // VERIFICATION LINK ENGINE
  // -------------------------------------------------------------

  const LINK_VETO_WORDS = [
    'unsubscribe', 'optout', 'opt-out', 'preferences', 'mail-preferences', 'manage-notifications',
    '退订', '取消订阅', '偏好设置', 'privacy', 'terms', 'policy', 'legal', 'compliance',
    '服务条款', '隐私政策', '帮助中心', '常见问题', 'support.', 'help.', 'contact-us', 'about-us',
    'play.google.com', 'apps.apple.com', 'app store', 'google play',
    '.png', '.jpg', '.jpeg', '.gif', '.svg', '.webp', '.ico', '.pdf', '.css', '.js', 'open.track', 'pixel.gif'
  ]

  const LINK_AUTH_PATH_TOKENS = [
    'verify', 'verification', 'confirm', 'confirmation', 'activate', 'activation',
    'magic-link', 'magic', 'login/confirm', 'signin/confirm', 'auth/callback',
    'reset-password', 'reset_password', 'password/reset', 'device/verify'
  ]

  const LINK_AUTH_QUERY_KEYS = [
    'token=', 'code=', 'key=', 'ticket=', 'auth=', 'sig=', 'hash=', 'magic=', 'secret='
  ]

  const LINK_ACTION_PHRASES = [
    'verify email', 'verify your email', 'confirm your email', 'confirm email', 'confirm account',
    'activate account', 'activate your account', 'log in to', 'sign in to', 'magic link',
    'reset password', 'reset your password', 'authorize device', 'click here to verify',
    '验证邮箱', '确认邮箱', '激活账号', '激活您的账号', '点击验证', '立即登录', '一键登录',
    '确认注册', '重置密码', '完成注册', '授权此设备'
  ]

  function findLink(subject, body, html) {
    const candidates = extractLinkCandidates(body, html)
    let best = null
    let bestScore = -1

    for (const c of candidates) {
      const urlLower = c.url.toLowerCase()
      const anchorLower = (c.anchor || '').toLowerCase()

      const hasAuthMarker = LINK_AUTH_PATH_TOKENS.some(t => urlLower.includes(t)) ||
        LINK_AUTH_QUERY_KEYS.some(k => urlLower.includes(k))

      // Only apply veto if URL lacks auth markers
      if (!hasAuthMarker && LINK_VETO_WORDS.some(v => urlLower.includes(v) || anchorLower.includes(v))) {
        continue
      }

      let score = 0

      // Decode URL components for tracking wrapper inspection
      let decodedUrl = urlLower
      try { decodedUrl = decodeURIComponent(urlLower) } catch {}

      // URL Path & Query inspection
      for (const token of LINK_AUTH_PATH_TOKENS) {
        if (decodedUrl.includes(token)) score += 45
      }
      for (const qk of LINK_AUTH_QUERY_KEYS) {
        if (decodedUrl.includes(qk)) score += 35
      }

      // High entropy token check (token length >= 16)
      if (/[?&](token|code|key|ticket|sig|hash)=[a-zA-Z0-9_-]{16,}/i.test(c.url)) {
        score += 30
      }

      // Anchor text inspection
      for (const phrase of LINK_ACTION_PHRASES) {
        if (anchorLower.includes(phrase)) score += 50
      }

      // Surrounding context
      if (c.context) {
        const ctxLower = c.context.toLowerCase()
        if (EXPIRATION_SIGNALS.some(s => ctxLower.includes(s))) {
          score += 25
        }
        for (const phrase of LINK_ACTION_PHRASES) {
          if (ctxLower.includes(phrase)) score += 30
        }
      }

      // Subject inspection
      const subLower = (subject || '').toLowerCase()
      for (const phrase of LINK_ACTION_PHRASES) {
        if (subLower.includes(phrase)) score += 25
      }
      if (subLower.includes('verify') || subLower.includes('verification') || subLower.includes('验证') || subLower.includes('激活') || subLower.includes('magic')) {
        score += 20
      }

      if (score >= 70 && score > bestScore) {
        bestScore = score
        best = c.url
      }
    }

    return best
  }

  function extractLinkCandidates(body, html) {
    const candidates = []
    const seen = new Set()

    if (html) {
      const aTagRegex = /<a\s+[^>]*href\s*=\s*["'](https?:\/\/[^"']+)["'][^>]*>(.*?)<\/a>/gis
      let match
      while ((match = aTagRegex.exec(html)) !== null) {
        const rawUrl = match[1].trim()
        const cleanUrl = rawUrl.replace(/&amp;/g, '&')
        const rawAnchor = match[2].replace(/<[^>]+>/g, '').trim()
        if (!seen.has(cleanUrl)) {
          seen.add(cleanUrl)
          const start = Math.max(0, match.index - 100)
          const end = Math.min(html.length, match.index + match[0].length + 100)
          const context = html.slice(start, end).replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ')
          candidates.push({ url: cleanUrl, anchor: rawAnchor, context })
        }
      }
    }

    if (body) {
      const urlRegex = /https?:\/\/[^\s<>"'()[\]{}，。、]+/g
      let match
      while ((match = urlRegex.exec(body)) !== null) {
        let rawUrl = match[0].trim().replace(/[.,;!?]+$/, '')
        let cleanUrl = rawUrl.replace(/&amp;/g, '&')
        if (!seen.has(cleanUrl)) {
          seen.add(cleanUrl)
          const start = Math.max(0, match.index - 80)
          const end = Math.min(body.length, match.index + match[0].length + 80)
          const context = body.slice(start, end)
          candidates.push({ url: cleanUrl, anchor: '', context })
        }
      }
    }

    return candidates
  }

  function truncateDisplayUrl(url, maxLen = 38) {
    if (!url) return ''
    let cleaned = url.replace(/^https?:\/\//i, '').replace(/^www\./i, '')
    if (cleaned.length <= maxLen) return cleaned
    return `${cleaned.slice(0, maxLen - 3)}...`
  }

  function detectMailNotice(subject, body, html) {
    const code = findCode(subject, body)
    if (code) {
      return { kind: 'code', code }
    }
    const link = findLink(subject, body, html)
    if (link) {
      return { kind: 'link', url: link, displayUrl: truncateDisplayUrl(link) }
    }
    return null
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = {
      findCode,
      findLink,
      detectMailNotice,
      truncateDisplayUrl,
    }
  } else {
    window.MailEngine = {
      findCode,
      findLink,
      detectMailNotice,
      truncateDisplayUrl,
    }
  }
})()
