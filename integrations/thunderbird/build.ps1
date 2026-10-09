# Packs the Thunderbird extension: integrations/thunderbird/wakuwaku-mail.xpi
# (an .xpi is a zip with the manifest at its top).
$here = $PSScriptRoot
$zip = Join-Path $here 'wakuwaku-mail.zip'
$xpi = Join-Path $here 'wakuwaku-mail.xpi'
Remove-Item $zip, $xpi -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $here 'wakuwaku-mail\*') -DestinationPath $zip
Move-Item $zip $xpi
Write-Host "Packed: $xpi"
