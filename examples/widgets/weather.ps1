# A weather widget for Wakuwaku's island: the sky and the temperature in a
# city, from wttr.in (no key needed), every 10 minutes.
#
#   pwsh examples/widgets/weather.ps1 -City Shanghai
#   pwsh examples/widgets/weather.ps1 -City Berlin -Once
param(
  [string]$City = 'Shanghai',
  [int]$Port = 47213,
  [int]$Minutes = 10,
  [switch]$Once
)

# wttr.in's own Chinese words are often English: the common skies, by their code.
$sky = @{
  113 = '晴'; 116 = '多云'; 119 = '阴'; 122 = '阴'; 143 = '雾'; 248 = '雾'; 260 = '雾'
  176 = '小雨'; 263 = '小雨'; 266 = '小雨'; 293 = '小雨'; 296 = '小雨'; 353 = '阵雨'
  299 = '中雨'; 302 = '中雨'; 305 = '大雨'; 308 = '大雨'; 356 = '大雨'; 359 = '暴雨'
  200 = '雷阵雨'; 386 = '雷阵雨'; 389 = '雷雨'
  179 = '小雪'; 227 = '小雪'; 323 = '小雪'; 326 = '小雪'; 329 = '中雪'; 332 = '中雪'; 335 = '大雪'; 338 = '大雪'; 230 = '暴雪'
}

while ($true) {
  try {
    $now = (Invoke-RestMethod "https://wttr.in/$([uri]::EscapeDataString($City))?format=j1" -TimeoutSec 20).current_condition[0]
    $words = $sky[[int]$now.weatherCode]
    if (-not $words) { $words = $now.weatherDesc[0].value.Trim() }
    $widget = @{
      id    = 'weather'
      icon  = 'weather'
      color = '#ffb340'
      label = "$City · $words"
      value = "$($now.temp_C)°"
      # A little longer than the wait, so it stays between two updates.
      ttl   = $Minutes * 60 + 120
    } | ConvertTo-Json -Compress
    # As UTF-8 bytes: Windows PowerShell 5 would send a string in another encoding.
    Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($widget)) | Out-Null
  } catch {
    Write-Warning $_
  }
  if ($Once) { break }
  Start-Sleep -Seconds ($Minutes * 60)
}
