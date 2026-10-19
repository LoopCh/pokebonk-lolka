# Локальный CD с Windows: собрать образ на этом ПК и развернуть на VPS
# без GitHub и без реестра образов. Нужны Docker Desktop и ssh/scp (есть в Windows 10+).
#
#   .\scripts\deploy.ps1 -Server deploy@1.2.3.4
#   .\scripts\deploy.ps1 -Server deploy@1.2.3.4 -Port 2222 -SkipChecks

param(
    [Parameter(Mandatory = $true)][string]$Server,
    [int]$Port = 22,
    [string]$Dir = "pokebonk-lolka",
    [string]$Platform = "linux/amd64",
    [switch]$SkipChecks
)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

function Invoke-Native {
    & $args[0] $args[1..($args.Count - 1)]
    if ($LASTEXITCODE -ne 0) { throw "Команда завершилась с кодом ${LASTEXITCODE}: $args" }
}

if (-not $SkipChecks) {
    & "$PSScriptRoot\ci-local.ps1"
}

$rev = $null
try { $rev = (git rev-parse --short HEAD 2>$null) } catch { }
if (-not $rev) { $rev = Get-Date -Format "yyyyMMdd-HHmmss" }
$tag = "pokebonk-lolka:local-$rev"
# Файл в папке проекта (он в .gitignore): Windows-путь вида C:\... scp принял бы за host:path.
$tar = "pokebonk-lolka.tar"

Write-Host "==> Сборка $tag" -ForegroundColor Cyan
Invoke-Native docker build --platform $Platform -t $tag .
Invoke-Native docker save -o $tar $tag

Write-Host "==> Отправка на $Server" -ForegroundColor Cyan
Invoke-Native ssh -p $Port $Server "mkdir -p ~/$Dir"
Invoke-Native scp -P $Port $tar deploy/docker-compose.yml deploy/remote-deploy.sh deploy/bot.env.example "${Server}:$Dir/"
Remove-Item $tar

Write-Host "==> Запуск" -ForegroundColor Cyan
Invoke-Native ssh -p $Port $Server "cd ~/$Dir && docker load -i pokebonk-lolka.tar && rm pokebonk-lolka.tar && IMAGE=$tag PULL=0 sh remote-deploy.sh"
