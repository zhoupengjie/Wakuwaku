# A reminder for Wakuwaku's island: every so often it asks her to open the
# island and say it (a nudge), unless a session needs you just then. Between
# reminders it shows when the next one comes.
#
#   pwsh examples/widgets/stretch.ps1
#   pwsh examples/widgets/stretch.ps1 -Minutes 45 -Words '喝口水'
param(
  [int]$Minutes = 50,
  [string]$Words = '坐久了，起来走走',
  [int]$Port = 47213
)

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

$next = (Get-Date).AddMinutes($Minutes)
while ($true) {
  try {
    $left = [math]::Max(0, [math]::Ceiling(($next - (Get-Date)).TotalMinutes))
    if ($left -le 0) {
      Send @{ id = 'stretch'; icon = 'bell'; color = '#34d27b'; label = $Words; value = '现在'; ttl = 600; nudge = $Words }
      $next = (Get-Date).AddMinutes($Minutes)
    } else {
      Send @{ id = 'stretch'; icon = 'bell'; color = '#34d27b'; label = $Words; value = "$left 分钟后"; ttl = 180 }
    }
  } catch {
    Write-Warning $_
  }
  Start-Sleep -Seconds 60
}
