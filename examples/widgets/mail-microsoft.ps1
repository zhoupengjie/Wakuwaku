# New mail in Wakuwaku's island, for Outlook.com, Hotmail and Microsoft 365
# (work and school): the unread count, and for each new letter the island
# opens with who it is from and what about. Sender and subject are private:
# while the settings' 显示具体内容 is off, only the count shows.
#
#   pwsh examples/widgets/mail-microsoft.ps1 -ClientId <id> -Setup    # once: sign in
#   pwsh examples/widgets/mail-microsoft.ps1 -ClientId <id>
#
# Microsoft takes no passwords from mail apps any more, only OAuth: you sign
# in once in the browser (a device code, as command-line tools do), and this
# keeps the refresh token encrypted for your Windows account (DPAPI) under
# %LOCALAPPDATA%\wakuwaku\secrets. It reads mail through Microsoft Graph,
# with only Mail.Read.
#
# Like every mail app, it needs an app registered with Microsoft, whose id is
# -ClientId (free, five minutes, once):
#   1. https://entra.microsoft.com → 应用注册 → 新注册: a name ("Wakuwaku
#      Mail"); 受支持的帐户类型: 任何组织目录中的帐户和个人 Microsoft 帐户
#   2. 身份验证 → 允许公共客户端流: 是
#   3. API 权限 → 添加 → Microsoft Graph → 委托的权限: Mail.Read
#   4. 概述: 复制 "应用程序(客户端) ID"
param(
  [Parameter(Mandatory)][string]$ClientId,
  # common: personal and work accounts; consumers, organizations, or a tenant id.
  [string]$Tenant = 'common',
  [switch]$Setup,
  [double]$Minutes = 2,
  [int]$Port = 47213,
  [switch]$Once,
  # Testing only: other places to sign in and to read from.
  [string]$LoginBase = 'https://login.microsoftonline.com',
  [string]$GraphBase = 'https://graph.microsoft.com/v1.0'
)

$ErrorActionPreference = 'Stop'
$scope = 'offline_access https://graph.microsoft.com/Mail.Read'
$token = "$LoginBase/$Tenant/oauth2/v2.0/token"
$secrets = Join-Path $env:LOCALAPPDATA 'wakuwaku\secrets'
$secretFile = Join-Path $secrets ("graph-" + ($ClientId -replace '[^A-Za-z0-9-]', '_') + '.txt')

function Save-Refresh($refresh) {
  New-Item -ItemType Directory -Force $secrets | Out-Null
  ConvertTo-SecureString $refresh -AsPlainText -Force | ConvertFrom-SecureString | Set-Content $secretFile
}

function Get-Refresh {
  if (-not (Test-Path $secretFile)) { throw '还没登录：先运行一次，加上 -Setup' }
  [Net.NetworkCredential]::new('', (Get-Content $secretFile | ConvertTo-SecureString)).Password
}

# A new access token from the refresh token, which may come back renewed.
function Get-Access {
  $r = Invoke-RestMethod -Method Post $token -Body @{ client_id = $ClientId; grant_type = 'refresh_token'; refresh_token = (Get-Refresh); scope = $scope }
  if ($r.refresh_token) { Save-Refresh $r.refresh_token }
  $r.access_token
}

# Signing in: a code to type in the browser, then waiting for it.
function Sign-In {
  $code = Invoke-RestMethod -Method Post "$LoginBase/$Tenant/oauth2/v2.0/devicecode" -Body @{ client_id = $ClientId; scope = $scope }
  Write-Host $code.message
  $wait = [math]::Max(1, [int]$code.interval)
  $until = (Get-Date).AddSeconds([int]$code.expires_in)
  while ((Get-Date) -lt $until) {
    Start-Sleep -Seconds $wait
    try {
      $r = Invoke-RestMethod -Method Post $token -Body @{ client_id = $ClientId; grant_type = 'urn:ietf:params:oauth:grant-type:device_code'; device_code = $code.device_code }
      Save-Refresh $r.refresh_token
      return
    } catch {
      $why = try { ($_.ErrorDetails.Message | ConvertFrom-Json).error } catch { '' }
      if ($why -eq 'authorization_pending') { continue }
      if ($why -eq 'slow_down') { $wait += 5; continue }
      throw "登录没成功：$why $_"
    }
  }
  throw '登录超时了，再运行一次 -Setup'
}

# The inbox now: how many unread, and the newest unread letter.
function Read-Inbox {
  $h = @{ Authorization = "Bearer $(Get-Access)" }
  $folder = Invoke-RestMethod "$GraphBase/me/mailFolders/inbox?`$select=unreadItemCount" -Headers $h
  # Graph sorts only by what it filters on, in the same order.
  $query = "`$filter=receivedDateTime ge 1900-01-01T00:00:00Z and isRead eq false&`$orderby=receivedDateTime desc&`$top=1&`$select=id,subject,from,receivedDateTime"
  $list = Invoke-RestMethod "$GraphBase/me/mailFolders/inbox/messages?$query" -Headers $h
  $m = @($list.value) | Select-Object -First 1
  $newest = if ($m) {
    $who = if ($m.from.emailAddress.name) { $m.from.emailAddress.name } else { $m.from.emailAddress.address }
    [pscustomobject]@{ Id = $m.id; From = $who; Subject = $(if ($m.subject) { $m.subject } else { '（无主题）' }) }
  }
  [pscustomobject]@{ Unread = [int]$folder.unreadItemCount; Newest = $newest }
}

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

if ($Setup) {
  Sign-In
  $inbox = Read-Inbox
  Write-Host "好了：收件箱里 $($inbox.Unread) 封未读。以后不带 -Setup 运行就行。"
  return
}

$id = 'mail-microsoft'
$ttl = [int]($Minutes * 60) + 180
$lastId = $null
$isFirst = $true
while ($true) {
  try {
    $inbox = Read-Inbox
    $widget = @{ id = $id; icon = 'mail'; color = '#5e9bff'; private = $true; ttl = $ttl
      label = $(if ($inbox.Newest) { "$($inbox.Newest.From)：$($inbox.Newest.Subject)" } else { 'Outlook' })
      value = $(if ($inbox.Unread) { "$($inbox.Unread) 封未读" } else { '没有未读' })
    }
    # A newest unread letter not seen at the last look.
    if (-not $isFirst -and $inbox.Newest -and $inbox.Newest.Id -ne $lastId) {
      $widget.nudge = "新邮件 · $($inbox.Newest.From)：$($inbox.Newest.Subject)"
    }
    $lastId = if ($inbox.Newest) { $inbox.Newest.Id } else { $null }
    $isFirst = $false
    Send $widget
  } catch {
    Write-Warning $_
    try { Send @{ id = $id; icon = 'mail'; color = '#ff5c6c'; private = $true; ttl = $ttl; label = 'Outlook'; value = '收不到信' } } catch {}
  }
  if ($Once) { break }
  Start-Sleep -Seconds ([math]::Max(30, [int]($Minutes * 60)))
}
