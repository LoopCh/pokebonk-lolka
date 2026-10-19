# Локальный CI на Windows: те же проверки, что в GitHub Actions.
# Rust ставить не нужно — всё выполняется в контейнере (нужен Docker Desktop).
#
#   .\scripts\ci-local.ps1
#   .\scripts\ci-local.ps1 -Image   # плюс сборка Docker-образа

param([switch]$Image)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path $PSScriptRoot -Parent)

function Invoke-Native {
    & $args[0] $args[1..($args.Count - 1)]
    if ($LASTEXITCODE -ne 0) { throw "Команда завершилась с кодом ${LASTEXITCODE}: $args" }
}

# Кеш зависимостей и сборки в именованных томах, чтобы повторные запуски были быстрыми.
Write-Host "==> fmt, clippy, тесты" -ForegroundColor Cyan
Invoke-Native docker run --rm `
    -v "${PWD}:/src" -w /src `
    -v pokebonk-cargo-registry:/usr/local/cargo/registry `
    -v pokebonk-target:/src/target `
    rust:1 sh -c "rustup component add rustfmt clippy >/dev/null 2>&1 && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"

if ($Image) {
    Write-Host "==> Сборка образа" -ForegroundColor Cyan
    Invoke-Native docker build --platform linux/amd64 -t pokebonk-lolka:dev .
    docker image ls pokebonk-lolka:dev
}
Write-Host "OK: всё зелёное" -ForegroundColor Green
