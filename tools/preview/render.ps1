#!/usr/bin/env pwsh
# Shoots every scene of the gallery (scenes.js) in Windows' dark and light
# mode with headless Edge, one PNG each (<id>-<mode>.png), and lays them out
# on one sheet per mode (overview-dark.png, overview-light.png).
#
#   pwsh tools/preview/render.ps1 -Out <folder> [-Only capsule-*,flow-*] [-Modes dark] [-Scale 2]
#   pwsh tools/preview/render.ps1 -Open     # the live gallery in Edge; nothing shot
#
# Each scene is shot.html?scene=<id>&mode=<mode> at its view's size: her
# real page (dist/pet, built from src/pet first) in a frame, run with the stand-in for Tauri, frozen
# before the shot (boot.js). A shot taken before the page froze has a
# magenta square at its top-left corner, one whose page had errors a red
# band at its foot: both are reported at the end (and the exit code is 1).
#
# Needs node (the scene list) and Edge; draws nothing on the screen.
param(
    [string]$Out = (Join-Path ([IO.Path]::GetTempPath()) 'wakuwaku-gallery'),
    [string[]]$Only = @('*'),
    [ValidateSet('dark', 'light')][string[]]$Modes = @('dark', 'light'),
    [double]$Scale = 2,
    [int]$Parallel = 6,
    [switch]$NoOverview,
    [switch]$Open,
    [string]$Edge = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
# -Only a,b as one word too (pwsh -File passes it so).
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if (-not (Test-Path -LiteralPath $Edge)) { throw "Edge not found at $Edge (-Edge <msedge.exe>)" }

# Her pages as they run (dist/), from src/ as it is now: what the frames load.
& node (Join-Path $here '..\..\scripts\build-pages.mjs')
if ($LASTEXITCODE -ne 0) { throw 'her pages did not build (scripts/build-pages.mjs)' }

# The live gallery, in an Edge of its own that may read her pages over file://.
if ($Open) {
    $url = [Uri]::new((Join-Path $here 'gallery.html')).AbsoluteUri + '?mode=dark'
    $viewer = Join-Path ([IO.Path]::GetTempPath()) 'wakuwaku-gallery-edge'
    Start-Process -FilePath $Edge -ArgumentList @('--allow-file-access-from-files', '--no-first-run', "--user-data-dir=$viewer", "`"$url`"")
    return
}
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Out = (Resolve-Path -LiteralPath $Out).Path

# The scenes, as scenes.js lists them.
$listFile = Join-Path $Out 'scenes.json'
& node (Join-Path $here 'scenes.js') --list $listFile
if ($LASTEXITCODE -ne 0) { throw 'node could not list the scenes' }
$list = Get-Content -LiteralPath $listFile -Raw -Encoding UTF8 | ConvertFrom-Json
$scenes = @($list.scenes | Where-Object { $id = $_.id; @($Only | Where-Object { $id -like $_ }).Count -gt 0 })
if (-not $scenes.Count) { throw "no scene matches $($Only -join ', ')" }

$shotUrl = [Uri]::new((Join-Path $here 'shot.html')).AbsoluteUri
$order = 0
$jobs = foreach ($s in $scenes) {
    $order++
    foreach ($m in $Modes) {
        [pscustomobject]@{
            Id = $s.id; Mode = $m; Title = $s.title; Section = $s.section; Order = $order
            W = [int]$s.view[2]; H = [int]$s.view[3]; Budget = [int]$s.budget
            # The browser's language as the shot's: her clock's date in its format.
            Lang = if ($s.lang -eq 'en') { 'en-US' } else { 'zh-CN' }
            Url = "${shotUrl}?scene=$([Uri]::EscapeDataString($s.id))&mode=$m"
            Png = Join-Path $Out "$($s.id)-$m.png"
        }
    }
}

# One shot: Edge with a profile of its own (several run at once).
$shoot = {
    param($job, $edge, $scale, $dataDir)
    if (Test-Path -LiteralPath $job.Png) { Remove-Item -LiteralPath $job.Png -Force }
    $a = @(
        '--headless=new', '--hide-scrollbars', '--allow-file-access-from-files', '--disable-gpu-vsync',
        '--no-first-run', '--no-default-browser-check', "--user-data-dir=$dataDir", "--lang=$($job.Lang)",
        "--window-size=$($job.W),$($job.H)", "--force-device-scale-factor=$scale",
        "--virtual-time-budget=$($job.Budget)", "--screenshot=$($job.Png)", "`"$($job.Url)`""
    )
    Start-Process -FilePath $edge -ArgumentList $a -Wait -WindowStyle Hidden
    Test-Path -LiteralPath $job.Png
}

$profiles = Join-Path $Out '.edge'
$started = Get-Date
"Shooting $($jobs.Count) shots ($($scenes.Count) scenes x $($Modes -join ', ')) into $Out"
if ($PSVersionTable.PSVersion.Major -ge 7 -and $Parallel -gt 1) {
    $shootText = $shoot.ToString()
    $jobs | ForEach-Object -ThrottleLimit $Parallel -Parallel {
        $fn = [scriptblock]::Create($using:shootText)
        # A profile of its own for each shot: two never share one.
        $null = & $fn $_ $using:Edge $using:Scale (Join-Path $using:profiles "$($_.Id)-$($_.Mode)")
    }
} else {
    foreach ($job in $jobs) { $null = & $shoot $job $Edge $Scale (Join-Path $profiles 'one') }
}
# Any missing, once more, one at a time.
foreach ($job in $jobs | Where-Object { -not (Test-Path -LiteralPath $_.Png) }) {
    $null = & $shoot $job $Edge $Scale (Join-Path $profiles 'again')
}
Remove-Item -LiteralPath $profiles -Recurse -Force -ErrorAction SilentlyContinue
"Shot in $([int]((Get-Date) - $started).TotalSeconds) s"

# --- Checking each shot -----------------------------------------------------------------
Add-Type -AssemblyName System.Drawing
$problems = @()
foreach ($job in $jobs) {
    if (-not (Test-Path -LiteralPath $job.Png)) { $problems += "$($job.Id) ($($job.Mode)): no PNG"; continue }
    $bmp = [System.Drawing.Bitmap]::new($job.Png)
    try {
        $c = $bmp.GetPixel(2, 2)
        if ($c.R -gt 240 -and $c.G -lt 20 -and $c.B -gt 190 -and $c.B -lt 230) { $problems += "$($job.Id) ($($job.Mode)): shot before the page froze (a longer budget in scenes.js)" }
        $f = $bmp.GetPixel(2, $bmp.Height - 2)
        if ($f.R -gt 150 -and $f.R -lt 200 -and $f.G -lt 30 -and $f.B -gt 20 -and $f.B -lt 60) { $problems += "$($job.Id) ($($job.Mode)): the page had errors (see the shot's foot)" }
    } finally { $bmp.Dispose() }
}

# --- The overview: every shot of a mode on one sheet --------------------------------------
function Overview([string]$mode) {
    $shots = @($jobs | Where-Object { $_.Mode -eq $mode -and (Test-Path -LiteralPath $_.Png) })
    if (-not $shots.Count) { return }
    $dark = $mode -eq 'dark'
    $bg = if ($dark) { [System.Drawing.Color]::FromArgb(28, 28, 31) } else { [System.Drawing.Color]::FromArgb(243, 243, 243) }
    $ink = if ($dark) { [System.Drawing.Color]::FromArgb(235, 235, 240) } else { [System.Drawing.Color]::FromArgb(28, 28, 32) }
    $soft = if ($dark) { [System.Drawing.Color]::FromArgb(150, 150, 158) } else { [System.Drawing.Color]::FromArgb(100, 100, 108) }
    $line = if ($dark) { [System.Drawing.Color]::FromArgb(60, 60, 66) } else { [System.Drawing.Color]::FromArgb(210, 210, 214) }
    $fontHead = [System.Drawing.Font]::new('Microsoft YaHei UI', 20, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
    $fontLabel = [System.Drawing.Font]::new('Microsoft YaHei UI', 13, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $fontTitle = [System.Drawing.Font]::new('Microsoft YaHei UI', 26, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
    $pad = 28; $gap = 22; $width = 1600 + 2 * $pad; $labelH = 22; $headH = 40

    # Laid out first (each shot at its size on the screen), drawn after.
    $placed = @(); $y = $pad + 50
    foreach ($sec in $list.sections) {
        # The short ones first, so a row's height wastes little room.
        $inSec = @($shots | Where-Object { $_.Section -eq $sec[0] } | Sort-Object H, Order)
        if (-not $inSec.Count) { continue }
        $placed += [pscustomobject]@{ Kind = 'head'; Text = $sec[1]; X = $pad; Y = $y }
        $y += $headH
        $x = $pad; $rowH = 0
        foreach ($s in $inSec) {
            if ($x -gt $pad -and $x + $s.W -gt $width - $pad) { $x = $pad; $y += $rowH + $gap; $rowH = 0 }
            $placed += [pscustomobject]@{ Kind = 'shot'; Job = $s; X = $x; Y = $y }
            $x += $s.W + $gap
            $rowH = [Math]::Max($rowH, $s.H + $labelH)
        }
        $y += $rowH + $gap + 14
    }
    $height = $y + $pad
    $sheet = [System.Drawing.Bitmap]::new($width, $height)
    $g = [System.Drawing.Graphics]::FromImage($sheet)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.PixelOffsetMode = 'HighQuality'
    $g.TextRenderingHint = 'AntiAliasGridFit'
    $g.Clear($bg)
    $inkB = [System.Drawing.SolidBrush]::new($ink); $softB = [System.Drawing.SolidBrush]::new($soft); $pen = [System.Drawing.Pen]::new($line)
    $g.DrawString("Wakuwaku gallery - $mode", $fontTitle, $inkB, $pad, $pad)
    foreach ($p in $placed) {
        if ($p.Kind -eq 'head') { $g.DrawString($p.Text, $fontHead, $inkB, $p.X, $p.Y); continue }
        $j = $p.Job
        $g.DrawString("$($j.Title)   $($j.Id)", $fontLabel, $softB, $p.X, $p.Y)
        $img = [System.Drawing.Image]::FromFile($j.Png)
        try { $g.DrawImage($img, [System.Drawing.Rectangle]::new($p.X, $p.Y + $labelH, $j.W, $j.H)) } finally { $img.Dispose() }
        $g.DrawRectangle($pen, $p.X - 1, $p.Y + $labelH - 1, $j.W + 1, $j.H + 1)
    }
    $file = Join-Path $Out "overview-$mode.png"
    $sheet.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $sheet.Dispose(); $inkB.Dispose(); $softB.Dispose(); $pen.Dispose()
    "Overview: $file ($width x $height)"
}
if (-not $NoOverview) { foreach ($m in $Modes) { Overview $m } }

if ($problems.Count) {
    "Problems:"
    $problems | ForEach-Object { "  $_" }
    exit 1
}
"All $($jobs.Count) shots fine."
