# New mail in Wakuwaku's island, over IMAP: the unread count, and for each
# new letter the island opens with who it is from and what about. Sender and
# subject are private: while the settings' 显示具体内容 is off, only the
# count shows.
#
#   pwsh examples/widgets/mail-imap.ps1 -Address you@qq.com -Setup    # once
#   pwsh examples/widgets/mail-imap.ps1 -Address you@qq.com
#
# The password: QQ, 163, 126, Gmail, iCloud and Yahoo take an app password
# (授权码, app-specific password) for IMAP, not the one you log in with;
# -Setup says where each gives one. It is kept encrypted for your Windows
# account (DPAPI) under %LOCALAPPDATA%\wakuwaku\secrets, never on the command
# line. Outlook.com, Hotmail and Microsoft 365 take no passwords any more:
# use mail-microsoft.ps1 (or Thunderbird and its extension).
#
# Finding the server, the way mail apps do: the providers known here (and
# what they need), then Thunderbird's database (ISPDB), then the ISPDB entry
# for the domain's mail host (its MX record), then imap.<domain>:993.
param(
  [Parameter(Mandatory)][string]$Address,
  [string]$Server = '',
  [int]$ImapPort = 0,
  [string]$User = '',
  [switch]$Setup,
  [double]$Minutes = 1,
  [int]$Port = 47213,
  [switch]$Once,
  # Testing only: a local server without TLS.
  [switch]$NoTls
)

$ErrorActionPreference = 'Stop'
try { [Text.Encoding]::RegisterProvider([Text.CodePagesEncodingProvider]::Instance) } catch {}

# --- Where the mail is ---------------------------------------------------------------

$qq = @{ Host = 'imap.qq.com'; Name = 'QQ 邮箱'; Help = 'QQ 邮箱网页版：设置 → 账号 → POP3/IMAP/SMTP 服务，开启 IMAP，生成授权码' }
$known = @{
  'qq.com'         = $qq
  'vip.qq.com'     = $qq
  'foxmail.com'    = $qq
  '163.com'        = @{ Host = 'imap.163.com'; Name = '网易 163 邮箱'; Help = '网页版：设置 → POP3/SMTP/IMAP，开启 IMAP 服务，新增授权码' }
  '126.com'        = @{ Host = 'imap.126.com'; Name = '网易 126 邮箱'; Help = '网页版：设置 → POP3/SMTP/IMAP，开启 IMAP 服务，新增授权码' }
  'yeah.net'       = @{ Host = 'imap.yeah.net'; Name = '网易 yeah 邮箱'; Help = '网页版：设置 → POP3/SMTP/IMAP，开启 IMAP 服务，新增授权码' }
  'sina.com'       = @{ Host = 'imap.sina.com'; Name = '新浪邮箱'; Help = '网页版：设置 → 客户端 pop/imap/smtp，开启 IMAP（有的账号要授权码）' }
  'aliyun.com'     = @{ Host = 'imap.aliyun.com'; Name = '阿里邮箱'; Help = '网页版：设置 → 账户 → POP3/SMTP 及 IMAP/SMTP，开启 IMAP' }
  '139.com'        = @{ Host = 'imap.139.com'; Name = '139 邮箱'; Help = '网页版：设置 → 客户端设置，开启 IMAP，生成授权码' }
  'gmail.com'      = @{ Host = 'imap.gmail.com'; Name = 'Gmail'; Help = '先开两步验证，再到 https://myaccount.google.com/apppasswords 生成应用专用密码' }
  'googlemail.com' = @{ Host = 'imap.gmail.com'; Name = 'Gmail'; Help = '先开两步验证，再到 https://myaccount.google.com/apppasswords 生成应用专用密码' }
  'icloud.com'     = @{ Host = 'imap.mail.me.com'; Name = 'iCloud 邮件'; Help = 'https://account.apple.com → 登录和安全 → App 专用密码' }
  'me.com'         = @{ Host = 'imap.mail.me.com'; Name = 'iCloud 邮件'; Help = 'https://account.apple.com → 登录和安全 → App 专用密码' }
  'yahoo.com'      = @{ Host = 'imap.mail.yahoo.com'; Name = 'Yahoo 邮箱'; Help = 'Yahoo 账号安全 → 生成应用密码' }
}
$microsoft = 'outlook.com', 'hotmail.com', 'live.com', 'msn.com'

# Thunderbird's database of providers: the IMAP server it knows for a domain.
function Find-InIspdb($domain) {
  try { $xml = Invoke-RestMethod "https://autoconfig.thunderbird.net/v1.1/$domain" -TimeoutSec 10 } catch { return $null }
  $servers = @($xml.clientConfig.emailProvider.incomingServer) | Where-Object { $_.type -eq 'imap' }
  # This speaks TLS from the start (port 993): such a server first.
  $s = @($servers | Where-Object { $_.socketType -eq 'SSL' }) + @($servers) | Select-Object -First 1
  if (-not $s) { return $null }
  $p = if ($s.socketType -eq 'SSL') { [int]$s.port } else { 993 }
  [pscustomobject]@{ Host = $s.hostname; Port = $p; Name = "$($xml.clientConfig.emailProvider.displayName)"; Help = ''; OAuth = [bool](@($s.authentication) -contains 'OAuth2' -and -not (@($s.authentication) -contains 'password-cleartext')) }
}

# The domain a mail host belongs to: mx1.mail.example.com → example.com
# (example.com.cn for those under com.cn and the like).
function Get-MxDomain($domain) {
  try { $mx = Resolve-DnsName -Type MX $domain -ErrorAction Stop | Where-Object { $_.Type -eq 'MX' } | Sort-Object Preference | Select-Object -First 1 } catch { return $null }
  if (-not $mx) { return $null }
  $labels = $mx.NameExchange.TrimEnd('.').Split('.')
  $keep = if ($labels.Count -ge 3 -and $labels[-1].Length -eq 2 -and $labels[-2] -in 'com', 'net', 'org', 'edu', 'gov', 'co') { 3 } else { 2 }
  ($labels | Select-Object -Last $keep) -join '.'
}

function Find-Server($address) {
  $domain = $address.Split('@')[-1].ToLower()
  if ($Server) { return [pscustomobject]@{ Host = $Server; Port = $(if ($ImapPort) { $ImapPort } else { 993 }); Name = $domain; Help = '' } }
  if ($domain -in $microsoft) { throw "$domain 不再接受密码登录 IMAP：请用 mail-microsoft.ps1" }
  if ($known[$domain]) { $k = $known[$domain]; return [pscustomobject]@{ Host = $k.Host; Port = 993; Name = $k.Name; Help = $k.Help } }
  $found = Find-InIspdb $domain
  if (-not $found) { $mxDomain = Get-MxDomain $domain; if ($mxDomain -and $mxDomain -ne $domain) { $found = Find-InIspdb $mxDomain } }
  if ($found) {
    if ($found.OAuth) { throw "$domain 只接受 OAuth 登录：请用 mail-microsoft.ps1 或 Thunderbird" }
    return $found
  }
  [pscustomobject]@{ Host = "imap.$domain"; Port = 993; Name = $domain; Help = '' }
}

# --- The password, kept for this Windows account (DPAPI) ---------------------------

$secrets = Join-Path $env:LOCALAPPDATA 'wakuwaku\secrets'
$secretFile = Join-Path $secrets ("imap-" + ($Address.ToLower() -replace '[^a-z0-9._@-]', '_') + '.txt')

function Get-Password {
  if ($env:WAKUWAKU_MAIL_PASSWORD) { return $env:WAKUWAKU_MAIL_PASSWORD }
  if (-not (Test-Path $secretFile)) { throw "还没有 $Address 的授权码：先运行一次，加上 -Setup" }
  [Net.NetworkCredential]::new('', (Get-Content $secretFile | ConvertTo-SecureString)).Password
}

# --- A little IMAP: enough to look ------------------------------------------------------

class Imap {
  [Net.Sockets.TcpClient]$Tcp
  [IO.Stream]$Stream
  [byte[]]$Buf = [byte[]]::new(65536)
  [int]$Len = 0
  [int]$Pos = 0
  [int]$N = 0

  Imap([string]$hostName, [int]$port, [bool]$tls) {
    $this.Tcp = [Net.Sockets.TcpClient]::new()
    $this.Tcp.ReceiveTimeout = 20000
    $this.Tcp.SendTimeout = 20000
    $this.Tcp.Connect($hostName, $port)
    $this.Stream = $this.Tcp.GetStream()
    if ($tls) {
      $ssl = [Net.Security.SslStream]::new($this.Stream, $false)
      $ssl.AuthenticateAsClient($hostName)
      $this.Stream = $ssl
    }
    $greeting = $this.ReadLine()
    if ($greeting -notmatch '^\* (OK|PREAUTH)') { throw "IMAP 服务器没有问好：$greeting" }
  }

  [bool] Fill() {
    $this.Len = $this.Stream.Read($this.Buf, 0, $this.Buf.Length)
    $this.Pos = 0
    return $this.Len -gt 0
  }

  [byte[]] ReadBytes([int]$count) {
    $out = [IO.MemoryStream]::new()
    while ($out.Length -lt $count) {
      if ($this.Pos -ge $this.Len -and -not $this.Fill()) { throw 'IMAP 连接断了' }
      $take = [math]::Min($count - $out.Length, $this.Len - $this.Pos)
      $out.Write($this.Buf, $this.Pos, $take)
      $this.Pos += $take
    }
    return $out.ToArray()
  }

  [string] ReadLine() {
    $out = [IO.MemoryStream]::new()
    while ($true) {
      if ($this.Pos -ge $this.Len -and -not $this.Fill()) { throw 'IMAP 连接断了' }
      $b = $this.Buf[$this.Pos]
      $this.Pos++
      if ($b -eq 10) { break }
      if ($b -ne 13) { $out.WriteByte($b) }
    }
    return [Text.Encoding]::UTF8.GetString($out.ToArray())
  }

  # A command, and what came back: { Ok, Status, Items: [{ Text, Literal }] }.
  [object] Run([string]$command) {
    $this.N++
    $tag = "w$($this.N)"
    $bytes = [Text.Encoding]::UTF8.GetBytes("$tag $command`r`n")
    $this.Stream.Write($bytes, 0, $bytes.Length)
    $this.Stream.Flush()
    $items = [Collections.Generic.List[object]]::new()
    while ($true) {
      $line = $this.ReadLine()
      if ($line.StartsWith("$tag ")) {
        return [pscustomobject]@{ Ok = $line.StartsWith("$tag OK"); Status = $line.Substring($tag.Length + 1); Items = $items }
      }
      $text = $line
      $literal = $null
      # A literal: {n}, then n bytes, then the rest of the line.
      while ($text -match '\{(\d+)\}$') {
        $literal = $this.ReadBytes([int]$Matches[1])
        $text += ' ' + $this.ReadLine()
      }
      $items.Add([pscustomobject]@{ Text = $text; Literal = $literal })
    }
    return $null
  }

  [void] Close() {
    try { $this.Run('LOGOUT') | Out-Null } catch {}
    $this.Tcp.Dispose()
  }
}

function Quote($s) { '"' + ($s -replace '\\', '\\' -replace '"', '\"') + '"' }

# "=?UTF-8?B?5byg5LiJ?=" and friends, as text.
function Decode-Words([string]$s) {
  $s = [regex]::Replace($s, '(\?=)\s+(=\?)', '$1$2')
  [regex]::Replace($s, '=\?([^?]+)\?([BbQq])\?([^?]*)\?=', {
      param($m)
      try { $enc = [Text.Encoding]::GetEncoding($m.Groups[1].Value) } catch { $enc = [Text.Encoding]::UTF8 }
      $text = $m.Groups[3].Value
      if ($m.Groups[2].Value -in 'B', 'b') {
        $bytes = [Convert]::FromBase64String($text)
      } else {
        $q = [IO.MemoryStream]::new()
        for ($i = 0; $i -lt $text.Length; $i++) {
          $c = $text[$i]
          if ($c -eq '_') { $q.WriteByte(32) }
          elseif ($c -eq '=' -and $i + 2 -lt $text.Length) { $q.WriteByte([Convert]::ToByte($text.Substring($i + 1, 2), 16)); $i += 2 }
          else { $q.WriteByte([byte][char]$c) }
        }
        $bytes = $q.ToArray()
      }
      $enc.GetString($bytes)
    })
}

# From: and Subject: out of a header block.
function Read-Header([byte[]]$raw) {
  $text = [Text.Encoding]::UTF8.GetString($raw) -replace "`r?`n[ `t]+", ' '
  $from = if ($text -match '(?im)^From:\s*(.*)$') { Decode-Words $Matches[1].Trim() } else { '' }
  $subject = if ($text -match '(?im)^Subject:\s*(.*)$') { Decode-Words $Matches[1].Trim() } else { '' }
  # Who: the name if there is one, else the address.
  $who = if ($from -match '^\s*"?([^"<]+?)"?\s*<') { $Matches[1].Trim() } elseif ($from -match '<([^>]+)>') { $Matches[1] } else { $from }
  [pscustomobject]@{ From = $who; Subject = $(if ($subject) { $subject } else { '（无主题）' }) }
}

# The inbox now: unread count, the newest unread letter, the next UID.
function Read-Inbox($box, $password) {
  $imap = [Imap]::new($box.Host, $box.Port, -not $NoTls)
  try {
    # Some servers (163, 126, yeah.net) want to know the client before they
    # open a folder; before and after logging in, as servers differ in when.
    $id = 'ID ("name" "Wakuwaku" "version" "1.0" "vendor" "Wakuwaku")'
    $imap.Run($id) | Out-Null
    $login = $imap.Run("LOGIN $(Quote $(if ($User) { $User } else { $Address })) $(Quote $password)")
    if (-not $login.Ok) { throw "登录失败：$($login.Status)" }
    $imap.Run($id) | Out-Null
    $open = $imap.Run('EXAMINE INBOX')
    if (-not $open.Ok) { throw "打不开收件箱：$($open.Status)" }
    $next = 0
    foreach ($i in $open.Items) { if ($i.Text -match '\[UIDNEXT (\d+)\]') { $next = [long]$Matches[1] } }
    $search = $imap.Run('UID SEARCH UNSEEN')
    $uids = @(foreach ($i in $search.Items) { if ($i.Text -match '^\* SEARCH(.*)$') { $Matches[1].Trim().Split(' ', [StringSplitOptions]::RemoveEmptyEntries) | ForEach-Object { [long]$_ } } })
    $newest = $null
    if ($uids.Count) {
      $top = ($uids | Measure-Object -Maximum).Maximum
      $fetch = $imap.Run("UID FETCH $top (UID BODY.PEEK[HEADER.FIELDS (FROM SUBJECT)])")
      $item = $fetch.Items | Where-Object { $_.Literal } | Select-Object -First 1
      if ($item) { $newest = Read-Header $item.Literal; $newest | Add-Member Uid $top }
    }
    [pscustomobject]@{ Unread = $uids.Count; Newest = $newest; Next = $next }
  } finally {
    $imap.Close()
  }
}

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

# --- Setting up, then looking every so often ---------------------------------------------

# Not $server: names ignore case in PowerShell, and -Server is a string.
$mailbox = Find-Server $Address
if ($Setup) {
  Write-Host "邮箱：$Address  服务器：$($mailbox.Host):$($mailbox.Port)"
  if ($mailbox.Help) { Write-Host "授权码：$($mailbox.Help)" }
  $secure = Read-Host '授权码（或应用专用密码）' -AsSecureString
  $plain = [Net.NetworkCredential]::new('', $secure).Password
  $inbox = Read-Inbox $mailbox $plain
  New-Item -ItemType Directory -Force $secrets | Out-Null
  $secure | ConvertFrom-SecureString | Set-Content $secretFile
  Write-Host "好了：收件箱里 $($inbox.Unread) 封未读。以后不带 -Setup 运行就行。"
  return
}

$id = 'mail-' + (($Address.ToLower() -replace '[^a-z0-9_-]', '-') -replace '-+', '-')
if ($id.Length -gt 40) { $id = $id.Substring(0, 40) }
$ttl = [int]($Minutes * 60) + 180
$lastNext = $null
while ($true) {
  try {
    $inbox = Read-Inbox $mailbox (Get-Password)
    $widget = @{ id = $id; icon = 'mail'; color = '#5e9bff'; private = $true; ttl = $ttl
      label = $(if ($inbox.Newest) { "$($inbox.Newest.From)：$($inbox.Newest.Subject)" } else { $(if ($mailbox.Name) { $mailbox.Name } else { $Address }) })
      value = $(if ($inbox.Unread) { "$($inbox.Unread) 封未读" } else { '没有未读' })
    }
    # A letter that came since the last look, and is still unread.
    if ($null -ne $lastNext -and $inbox.Next -gt $lastNext -and $inbox.Newest -and $inbox.Newest.Uid -ge $lastNext) {
      $widget.nudge = "新邮件 · $($inbox.Newest.From)：$($inbox.Newest.Subject)"
    }
    $lastNext = $inbox.Next
    Send $widget
  } catch {
    Write-Warning $_
    try { Send @{ id = $id; icon = 'mail'; color = '#ff5c6c'; private = $true; ttl = $ttl; label = $(if ($mailbox.Name) { $mailbox.Name } else { $Address }); value = '收不到信' } } catch {}
  }
  if ($Once) { break }
  Start-Sleep -Seconds ([math]::Max(10, [int]($Minutes * 60)))
}
