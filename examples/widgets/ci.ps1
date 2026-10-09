# CI and pull requests in Wakuwaku's island, from GitHub: how the latest
# Actions run on a branch went, and (-Prs) the checks on the open pull
# requests. The island opens when a run you saw going finishes.
#
#   pwsh examples/widgets/ci.ps1 -Repo owner/repo
#   pwsh examples/widgets/ci.ps1 -Repo owner/repo -Branch dev -Prs -Author you
#
# Public repositories need no token (GitHub allows 60 requests an hour then:
# keep -Minutes at 3 or more, more with -Prs). For private ones, or more
# often: $env:GITHUB_TOKEN, or the GitHub CLI's (gh auth token) if it is there.
param(
  [Parameter(Mandatory)][string]$Repo,
  [string]$Branch = 'main',
  [switch]$Prs,
  [string]$Author = '',
  [double]$Minutes = 3,
  [string]$Token = $env:GITHUB_TOKEN,
  [int]$Port = 47213,
  [switch]$Once
)

if (-not $Token -and (Get-Command gh -ErrorAction SilentlyContinue)) {
  $Token = (gh auth token 2>$null)
}
$headers = @{ 'User-Agent' = 'wakuwaku'; Accept = 'application/vnd.github+json' }
if ($Token) { $headers.Authorization = "Bearer $Token" }
$base = "https://api.github.com/repos/$Repo"
$key = ($Repo.ToLower() -replace '[^a-z0-9_-]', '-')

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

# A run or a check's state, in a mark and a colour.
function Mark($status, $conclusion) {
  if ($status -ne 'completed') { return @('⏳', '#5e9bff') }
  switch ($conclusion) {
    'success' { @('✓', '#34d27b') }
    'skipped' { @('✓', '#8e8e93') }
    'neutral' { @('✓', '#8e8e93') }
    'cancelled' { @('–', '#8e8e93') }
    default { @('✗', '#ff5c6c') }
  }
}

$seen = @{}
$ttl = [int]($Minutes * 60) + 180
while ($true) {
  try {
    $runs = Invoke-RestMethod "$base/actions/runs?branch=$([uri]::EscapeDataString($Branch))&per_page=1" -Headers $headers -TimeoutSec 20
    if ($runs.total_count -eq 0) {
      Send @{ id = "ci-$key"; icon = 'check'; color = '#8e8e93'; label = "$Branch · CI"; value = '没有运行'; ttl = $ttl }
    } else {
      $run = $runs.workflow_runs[0]
      $mark, $color = Mark $run.status $run.conclusion
      $value = if ($run.status -ne 'completed') { "$mark 运行中" } elseif ($mark -eq '✗') { "$mark 失败" } else { "$mark 通过" }
      $widget = @{ id = "ci-$key"; icon = 'check'; color = $color; label = "$Branch · $($run.name)"; value = $value; ttl = $ttl }
      # Seen going, now done: the island says how it went.
      if ($seen[$run.id] -and $seen[$run.id] -ne 'completed' -and $run.status -eq 'completed') {
        $widget.nudge = if ($mark -eq '✗') { "CI 失败了：$($run.display_title)" } else { "CI 通过了：$($run.display_title)" }
      }
      $seen[$run.id] = $run.status
      Send $widget
    }
    if ($Prs) {
      # One by one: PowerShell 7 hands a JSON list over as one thing.
      $pulls = @((Invoke-RestMethod "$base/pulls?state=open&per_page=20" -Headers $headers -TimeoutSec 20) | ForEach-Object { $_ })
      if ($Author) { $pulls = @($pulls | Where-Object { $_.user.login -eq $Author }) }
      # Three fit in a widget's few characters.
      $marks = foreach ($pr in ($pulls | Select-Object -First 3)) {
        $checks = (Invoke-RestMethod "$base/commits/$($pr.head.sha)/check-runs?per_page=50" -Headers $headers -TimeoutSec 20).check_runs
        $state = if (-not $checks) { '·' }
          elseif ($checks | Where-Object { $_.status -ne 'completed' }) { '⏳' }
          elseif ($checks | Where-Object { $_.conclusion -notin 'success', 'skipped', 'neutral' }) { '✗' }
          else { '✓' }
        "#$($pr.number)$state"
      }
      $value = if ($pulls.Count) { ($marks -join ' ') } else { '没有' }
      $color = if ($value -match '✗') { '#ff5c6c' } elseif ($value -match '⏳') { '#5e9bff' } else { '#34d27b' }
      Send @{ id = "prs-$key"; icon = 'code'; color = $color; label = "PR · $($pulls.Count) 个开着"; value = $value; ttl = $ttl }
    }
  } catch {
    Write-Warning $_
  }
  if ($Once) { break }
  Start-Sleep -Seconds ([int]($Minutes * 60))
}
