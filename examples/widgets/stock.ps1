# Stocks, exchange rates and coins in Wakuwaku's island: each symbol's price
# and the day's change, from Yahoo Finance's chart data (unofficial, no key
# needed), every 5 minutes. One widget a symbol.
#   Shanghai and Shenzhen shares end in .SS and .SZ: 600519.SS, 000001.SZ
#   exchange rates: USDCNY=X, EURCNY=X; coins: BTC-USD, ETH-USD
#
#   pwsh examples/widgets/stock.ps1 -Symbol 600519.SS -Name 茅台
#   pwsh examples/widgets/stock.ps1 -Symbol USDCNY=X,BTC-USD -Name 美元,比特币
#   pwsh examples/widgets/stock.ps1 -Symbol AAPL -Once
#
# Up is red and down green, as Chinese markets show them; -GreenUp turns it round.
[CmdletBinding(PositionalBinding = $false)]
param(
  [string[]]$Symbol = @('AAPL'),
  [string[]]$Name = @(),
  [double]$Minutes = 5,
  [switch]$GreenUp,
  [int]$Port = 47213,
  [switch]$Once
)

# From cmd or bash, "-Symbol a,b" comes in as one word: the parts.
$Symbol = @($Symbol | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$Name = @($Name | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() })

function Send($widget) {
  $body = $widget | ConvertTo-Json -Compress
  Invoke-RestMethod -Method Post "http://127.0.0.1:$Port/widget" -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
}

# Few digits for big prices, more for small ones (a rate like 7.1234).
function Price([double]$p) {
  if ($p -ge 1000) { '{0:N0}' -f $p } elseif ($p -ge 10) { '{0:N2}' -f $p } else { '{0:N4}' -f $p }
}

while ($true) {
  for ($i = 0; $i -lt $Symbol.Count; $i++) {
    $s = $Symbol[$i]
    try {
      $meta = (Invoke-RestMethod "https://query1.finance.yahoo.com/v8/finance/chart/$([uri]::EscapeDataString($s))?range=1d&interval=1d" -Headers @{ 'User-Agent' = 'Mozilla/5.0' } -TimeoutSec 20).chart.result[0].meta
      $price = [double]$meta.regularMarketPrice
      $before = [double]$meta.chartPreviousClose
      $change = if ($before) { ($price - $before) / $before * 100 } else { 0 }
      $isUp = $change -ge 0
      $icon = if ($s -match '-USD$|-CNY$') { 'coin' } elseif ($s -match '=X$') { 'coin' } else { 'stock' }
      Send @{
        id    = "stock-$($s.ToLower() -replace '[^a-z0-9_-]', '-')"
        icon  = $icon
        color = $(if ($isUp -xor $GreenUp) { '#ff5c6c' } else { '#34d27b' })
        label = $(if ($i -lt $Name.Count -and $Name[$i]) { $Name[$i] } else { $s })
        value = '{0} {1}{2:N2}%' -f (Price $price), $(if ($isUp) { '▲' } else { '▼' }), [math]::Abs($change)
        ttl   = [int]($Minutes * 60) + 120
      }
    } catch {
      Write-Warning "${s}: $_"
    }
  }
  if ($Once) { break }
  Start-Sleep -Seconds ([int]($Minutes * 60))
}
