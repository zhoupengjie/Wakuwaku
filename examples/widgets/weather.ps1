# A weather widget for Wakuwaku's island: the sky and the temperature in a
# city, from Open-Meteo (no key needed), every 10 minutes. The city can be
# named in English or Chinese.
#
#   pwsh examples/widgets/weather.ps1 -City Shanghai
#   pwsh examples/widgets/weather.ps1 -City 杭州 -Once
#   pwsh examples/widgets/weather.ps1 -City Berlin -Lang en
param(
  [string]$City = 'Shanghai',
  [int]$Port = 47213,
  [int]$Minutes = 10,
  [switch]$Once,
  # zh or en: Chinese if Windows' language or its formats are.
  [string]$Lang = $(if ((Get-UICulture).Name -like 'zh*' -or (Get-Culture).Name -like 'zh*') { 'zh' } else { 'en' })
)

# The sky by its WMO code, in Chinese and in English.
$sky = @{
  0 = '晴', 'Clear'; 1 = '晴', 'Mostly clear'; 2 = '多云', 'Partly cloudy'; 3 = '阴', 'Overcast'
  45 = '雾', 'Fog'; 48 = '雾', 'Fog'
  51 = '毛毛雨', 'Drizzle'; 53 = '毛毛雨', 'Drizzle'; 55 = '毛毛雨', 'Drizzle'; 56 = '冻雨', 'Freezing drizzle'; 57 = '冻雨', 'Freezing drizzle'
  61 = '小雨', 'Light rain'; 63 = '中雨', 'Rain'; 65 = '大雨', 'Heavy rain'; 66 = '冻雨', 'Freezing rain'; 67 = '冻雨', 'Freezing rain'
  71 = '小雪', 'Light snow'; 73 = '中雪', 'Snow'; 75 = '大雪', 'Heavy snow'; 77 = '雪粒', 'Snow grains'
  80 = '阵雨', 'Showers'; 81 = '阵雨', 'Showers'; 82 = '强阵雨', 'Heavy showers'; 85 = '阵雪', 'Snow showers'; 86 = '阵雪', 'Snow showers'
  95 = '雷阵雨', 'Thunderstorm'; 96 = '雷阵雨伴冰雹', 'Thunderstorm, hail'; 99 = '雷阵雨伴冰雹', 'Thunderstorm, hail'
}

# Where the city is: found once, again next time if the network was down.
$place = $null
while ($true) {
  try {
    if (-not $place) {
      $found = Invoke-RestMethod "https://geocoding-api.open-meteo.com/v1/search?name=$([uri]::EscapeDataString($City))&count=1&language=$Lang" -TimeoutSec 20
      $place = $found.results | Select-Object -First 1
      if (-not $place) { throw $(if ($Lang -eq 'zh') { "找不到这个城市：$City" } else { "no city called $City" }) }
    }
    $now = (Invoke-RestMethod "https://api.open-meteo.com/v1/forecast?latitude=$($place.latitude)&longitude=$($place.longitude)&current=temperature_2m,weather_code" -TimeoutSec 20).current
    $words = $sky[[int]$now.weather_code]
    $words = if (-not $words) { '' } elseif ($Lang -eq 'zh') { $words[0] } else { $words[1] }
    $widget = @{
      id    = 'weather'
      icon  = 'weather'
      color = '#ffb340'
      label = "$City · $words".TrimEnd(' ', '·')
      value = "$([Math]::Round([double]$now.temperature_2m))°"
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
