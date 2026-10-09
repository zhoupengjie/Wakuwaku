# A stock widget for Wakuwaku's island: a symbol's price and the day's change,
# from Yahoo Finance's chart data (unofficial, no key needed), every 5 minutes.
# Shanghai and Shenzhen shares end in .SS and .SZ: 600519.SS, 000001.SZ.
#
#   pwsh examples/widgets/stock.ps1 -Symbol 600519.SS -Name 茅台
#   pwsh examples/widgets/stock.ps1 -Symbol AAPL -Once
#
# Up is red and down green, as Chinese markets show them; -GreenUp turns it round.
param(
  [string]$Symbol = 'AAPL',
  [string]$Name = '',
  [int]$Port = 47213,
  [int]$Minutes = 5,
  [switch]$GreenUp,
  [switch]$Once
)

while ($true) {
  try {
    $meta = (Invoke-RestMethod "https://query1.finance.yahoo.com/v8/finance/chart/$([uri]::EscapeDataString($Symbol))?range=1d&interval=1d" -Headers @{ 'User-Agent' = 'Mozilla/5.0' } -TimeoutSec 20).chart.result[0].meta
    $price = [double]$meta.regularMarketPrice
    $before = [double]$meta.chartPreviousClose
    $change = if ($before) { ($price - $before) / $before * 100 } else { 0 }
    $isUp = $change -ge 0
    $widget = @{
      id    = "stock-$($Symbol.ToLower() -replace '[^a-z0-9_-]', '-')"
      icon  = 'stock'
      color = if ($isUp -xor $GreenUp) { '#ff5c6c' } else { '#34d27b' }
      label = if ($Name) { $Name } else { $Symbol }
      value = '{0:N2} {1}{2:N2}%' -f $price, $(if ($isUp) { '▲' } else { '▼' }), [math]::Abs($change)
      ttl   = $Minutes * 60 + 120
    } | ConvertTo-Json -Compress
    Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($widget)) | Out-Null
  } catch {
    Write-Warning $_
  }
  if ($Once) { break }
  Start-Sleep -Seconds ($Minutes * 60)
}
