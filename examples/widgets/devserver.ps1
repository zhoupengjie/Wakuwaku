# Your dev servers in Wakuwaku's island: which are up ("3000 ✓ 5173 ✗"); the
# island opens when one that was up goes down. Any answer counts as up, an
# error page too; no answer is down.
#
#   pwsh examples/widgets/devserver.ps1
#   pwsh examples/widgets/devserver.ps1 -Url http://localhost:3000,http://localhost:5173/api/health
[CmdletBinding(PositionalBinding = $false)]
param(
  [string[]]$Url = @('http://localhost:3000'),
  [int]$Seconds = 15,
  [int]$Port = 47213,
  [switch]$Once
)

# From cmd or bash, "-Url a,b" comes in as one word: the parts.
$Url = @($Url | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

function IsUp($u) {
  try {
    Invoke-WebRequest $u -TimeoutSec 3 -UseBasicParsing -MaximumRedirection 0 -ErrorAction Stop | Out-Null
    return $true
  } catch {
    # An answer, if not a happy one, is a server that is up.
    return [bool]$_.Exception.Response
  }
}

# The few characters a server goes by: its port, or its host.
function Short($u) {
  $uri = [uri]$u
  if ($uri.IsDefaultPort) { $uri.Host } else { "$($uri.Port)" }
}

$was = @{}
while ($true) {
  try {
    $marks = @()
    $down = @()
    foreach ($u in $Url) {
      $up = IsUp $u
      $marks += "$(Short $u) $(if ($up) { '✓' } else { '✗' })"
      if (-not $up -and $was[$u]) { $down += Short $u }
      $was[$u] = $up
    }
    $allUp = -not ($marks -match '✗')
    $widget = @{ id = 'devserver'; icon = 'server'; color = $(if ($allUp) { '#34d27b' } else { '#ff5c6c' }); label = '开发服务器'; value = ($marks -join ' '); ttl = $Seconds + 60 }
    if ($down) { $widget.nudge = "开发服务器 $($down -join '、') 停了" }
    Send $widget
  } catch {
    Write-Warning $_
  }
  if ($Once) { break }
  Start-Sleep -Seconds $Seconds
}
