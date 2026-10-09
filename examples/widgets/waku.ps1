# Run a command, and have the island tell you when it is done.
#
#   waku cargo build --release
#   waku npm test
#
# While it runs the island shows it with a clock ("cargo build · ⏳ 2:13");
# when it ends the island opens: done, or failed with its exit code. The
# command runs right here, in this console, as it would without waku.
#
# To have `waku` everywhere: in your PowerShell profile (notepad $PROFILE),
#   Set-Alias waku <this folder>\waku.ps1
# and for cmd or Git Bash, waku.cmd / waku next to this file.
param([Parameter(ValueFromRemainingArguments)][string[]]$Command)

$port = if ($env:WAKUWAKU_PORT) { $env:WAKUWAKU_PORT } else { 47213 }
if (-not $Command) {
  Write-Host 'usage: waku <command> [arguments...]'
  exit 2
}

$id = "run-$PID"
$label = ($Command -join ' ')
if ($label.Length -gt 40) { $label = $label.Substring(0, 39) + '…' }

# To her; her being away never gets in the way of the command.
$send = {
  param($port, $widget)
  try {
    $body = $widget | ConvertTo-Json -Compress
    Invoke-RestMethod -Method Post "http://127.0.0.1:$port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) -TimeoutSec 2 | Out-Null
  } catch {}
}
function Clock([TimeSpan]$t) {
  if ($t.TotalHours -ge 1) { '{0}:{1:mm}:{1:ss}' -f [int][math]::Floor($t.TotalHours), $t } else { '{0}:{1:ss}' -f [int][math]::Floor($t.TotalMinutes), $t }
}

$start = Get-Date
# The clock, in the background, while the command has this console.
$tick = {
  param($port, $id, $label, $start, $send)
  $send = [scriptblock]::Create($send)
  while ($true) {
    $t = (Get-Date) - $start
    $clock = if ($t.TotalHours -ge 1) { '{0}:{1:mm}:{1:ss}' -f [int][math]::Floor($t.TotalHours), $t } else { '{0}:{1:ss}' -f [int][math]::Floor($t.TotalMinutes), $t }
    & $send $port @{ id = $id; icon = 'terminal'; color = '#5e9bff'; label = $label; value = "⏳ $clock"; ttl = 30 }
    Start-Sleep -Seconds 5
  }
}
$jobArgs = @($port, $id, $label, $start, $send.ToString())
$clockJob = if (Get-Command Start-ThreadJob -ErrorAction SilentlyContinue) { Start-ThreadJob $tick -ArgumentList $jobArgs } else { Start-Job $tick -ArgumentList $jobArgs }

$code = 1
$isOver = $false
try {
  $global:LASTEXITCODE = 0
  & $Command[0] @($Command | Select-Object -Skip 1)
  $code = if ($global:LASTEXITCODE) { $global:LASTEXITCODE } elseif ($?) { 0 } else { 1 }
  $isOver = $true
} finally {
  Stop-Job $clockJob -ErrorAction SilentlyContinue
  Remove-Job $clockJob -Force -ErrorAction SilentlyContinue
  $took = Clock ((Get-Date) - $start)
  $widget = if (-not $isOver) {
    @{ id = $id; icon = 'terminal'; color = '#8e8e93'; label = $label; value = "已停止 · $took"; ttl = 120 }
  } elseif ($code -eq 0) {
    @{ id = $id; icon = 'check'; color = '#34d27b'; label = $label; value = "✓ $took"; ttl = 300; nudge = "$label 跑完了 · $took" }
  } else {
    @{ id = $id; icon = 'terminal'; color = '#ff5c6c'; label = $label; value = "✗ 退出码 $code"; ttl = 300; nudge = "$label 失败了（退出码 $code）" }
  }
  & $send $port $widget
}
exit $code
