# Deadlines in Wakuwaku's island: the next one and how long until it; the
# island opens an hour before it, and when it comes.
#
#   pwsh examples/widgets/countdown.ps1 -At '2026-10-10 18:00' -Title 交周报
#   pwsh examples/widgets/countdown.ps1 -File deadlines.txt
#
# A file has one deadline a line, "2026-10-10 18:00 交周报"; # starts a note.
# It is read again each minute, so editing it is enough.
param(
  [string]$At = '',
  [string]$Title = '截止',
  [string]$File = '',
  [int]$Port = 47213,
  [switch]$Once
)

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

function Deadlines {
  $list = @()
  if ($At) { $list += [pscustomobject]@{ At = [datetime]::Parse($At); Title = $Title } }
  if ($File -and (Test-Path $File)) {
    foreach ($line in Get-Content -Encoding utf8 $File) {
      if ($line -match '^\s*(\d{4}-\d{1,2}-\d{1,2}(?:[ T]\d{1,2}:\d{2})?)\s+(.+?)\s*$' -and -not $line.TrimStart().StartsWith('#')) {
        $list += [pscustomobject]@{ At = [datetime]::Parse($Matches[1]); Title = $Matches[2] }
      }
    }
  }
  $list | Sort-Object At
}

# How long, in a few words.
function Left([TimeSpan]$t) {
  if ($t.TotalDays -ge 1) { return "还有 $([int][math]::Floor($t.TotalDays)) 天 $($t.Hours) 小时" }
  if ($t.TotalHours -ge 1) { return "还有 {0}:{1:mm}" -f [int][math]::Floor($t.TotalHours), $t }
  return "还有 $([int][math]::Ceiling($t.TotalMinutes)) 分钟"
}

$told = @{}
while ($true) {
  try {
    $now = Get-Date
    $next = Deadlines | Where-Object { $_.At -gt $now.AddMinutes(-10) } | Select-Object -First 1
    if (-not $next) {
      Send @{ id = 'countdown'; remove = $true }
    } else {
      $left = $next.At - $now
      $key = "$($next.At.ToString('o')) $($next.Title)"
      $widget = @{ id = 'countdown'; icon = 'flag'; color = $(if ($left.TotalHours -lt 1) { '#ff5c6c' } else { '#ffb340' }); label = $next.Title; value = $(if ($left.TotalSeconds -le 0) { '到点了' } else { Left $left }); ttl = 180 }
      if ($left.TotalSeconds -le 0 -and -not $told["$key now"]) {
        $widget.nudge = "到点了：$($next.Title)"
        $told["$key now"] = $true
        $told["$key hour"] = $true
      } elseif ($left.TotalHours -le 1 -and -not $told["$key hour"]) {
        $widget.nudge = "$(Left $left)：$($next.Title)"
        $told["$key hour"] = $true
      }
      Send $widget
    }
  } catch {
    Write-Warning $_
  }
  if ($Once) { break }
  Start-Sleep -Seconds 60
}
