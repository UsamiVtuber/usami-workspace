#Requires -Version 5.1
<#
.SYNOPSIS
  Builda o Usami Desktop e publica o zip no repo público de releases.

.EXAMPLE
  ./scripts/release.ps1 -Version 0.2.0

.EXAMPLE
  ./scripts/release.ps1 -Version 0.2.0 -DryRun
#>
param(
    [Parameter(Mandatory)] [string] $Version,
    [switch] $DryRun
)

$ErrorActionPreference = 'Stop'

$version = $Version -replace '^v', ''
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "Versão inválida: use X.Y.Z" }

$repo = 'UsamiVtuber/usami-desktop'
$crate = Split-Path -Parent $PSScriptRoot
$stage = Join-Path (Split-Path -Parent $crate) "dist/usami-desktop-v$version-windows-x64"
$zip = "$stage.zip"

if (-not $DryRun) {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { throw "gh não encontrado no PATH" }
    gh auth status *> $null
    if ($LASTEXITCODE -ne 0) { throw "Faça login com 'gh auth login' antes de publicar" }
    gh release view "v$version" --repo $repo *> $null
    if ($LASTEXITCODE -eq 0) { throw "A release v$version já existe em $repo" }
}

Push-Location $crate
try {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "cargo build falhou" }
} finally {
    Pop-Location
}

Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item (Join-Path $crate 'target/release/usami-desktop.exe') $stage
Copy-Item -Recurse (Join-Path $crate 'assets') (Join-Path $stage 'assets')
Copy-Item (Join-Path $crate 'PACKAGE_README.md') (Join-Path $stage 'README.md')
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -Force

Write-Host "Pacote: $zip"
if ($DryRun) {
    Write-Host 'DryRun: nada foi publicado.'
    return
}

gh release create "v$version" $zip `
    --repo $repo `
    --title "Usami Desktop $version" `
    --target main `
    --notes "Pacote Windows x64. Requer o backend em 127.0.0.1:8787 e um modelo .vrm próprio; instruções no README do zip."
