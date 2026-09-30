$ErrorActionPreference = 'Stop'
$Root = (git rev-parse --show-toplevel).Trim()
$DefaultBranch = 'sandbox/developer-win-64'
$Branch = $env:PIXI_SANDBOX_BRANCH
$Config = Join-Path $Root '.pixi-sandbox.toml'
if (-not $Branch -and (Test-Path $Config)) {
    # <branch_prefix>/<bundle>-win-64, read off the same reviewed plan the publisher uses.
    $Text = Get-Content -Raw $Config
    $Prefix = if ($Text -match '(?m)^\s*branch_prefix\s*=\s*["'']([^"'']*)') { $Matches[1] } else { 'sandbox' }
    $Bundles = @()
    foreach ($Chunk in ($Text -split '\[\[\s*bundle\s*\]\]')) {
        if ($Chunk -match 'platforms[^\]]*["'']win-64["'']' -and $Chunk -match 'name\s*=\s*["'']([^"'']*)') {
            $Bundles += $Matches[1]
        }
    }
    if ($env:PIXI_SANDBOX_BUNDLE) { $Bundles = @($Bundles | Where-Object { $_ -eq $env:PIXI_SANDBOX_BUNDLE }) }
    if ($Bundles.Count -eq 1) {
        $Branch = "$Prefix/$($Bundles[0])-win-64"
    } elseif ($Bundles.Count -gt 1) {
        Write-Error -Message 'several bundles publish win-64; set PIXI_SANDBOX_BUNDLE to choose' -ErrorAction Continue
    }
}
if (-not $Branch) { $Branch = $DefaultBranch }
git -C $Root rev-parse --verify "$Branch^{commit}" 2>$null | Out-Null
if ($LASTEXITCODE -ne 0) { $Branch = "origin/$Branch" }
$Transport = Join-Path $Root '.pixi/.restore-transport'
$Archive = Join-Path $Root '.pixi/.restore-transport.tar'
Remove-Item -Recurse -Force $Transport,$Archive -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $Transport | Out-Null
git -C $Root archive --format=tar --output=$Archive $Branch
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
tar -xf $Archive -C $Transport
$Binary = Join-Path $Transport '.pixi-sandbox/tools/win-64/pixi-sandbox.exe'
& $Binary restore --branch-location $Transport --output-path $Root --force @args
exit $LASTEXITCODE
