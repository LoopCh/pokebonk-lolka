#!/usr/bin/env bash
# Локальный CD для Linux / macOS / WSL: собрать образ здесь и развернуть на VPS.
#
#   ./scripts/deploy.sh deploy@1.2.3.4            # порт 22
#   PORT=2222 SKIP_CHECKS=1 ./scripts/deploy.sh deploy@1.2.3.4
set -euo pipefail

SERVER=${1:?"использование: $0 user@host"}
PORT=${PORT:-22}
DIR=${DIR:-pokebonk-lolka}
PLATFORM=${PLATFORM:-linux/amd64}
cd "$(dirname "$0")/.."

if [ -z "${SKIP_CHECKS:-}" ]; then
    echo "▶ fmt, clippy, тесты"
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
fi

rev=$(git rev-parse --short HEAD 2>/dev/null || date +%Y%m%d-%H%M%S)
tag="pokebonk-lolka:local-$rev"

echo "▶ Сборка $tag"
docker build --platform "$PLATFORM" -t "$tag" .

echo "▶ Отправка на $SERVER"
ssh -p "$PORT" "$SERVER" "mkdir -p ~/$DIR"
scp -P "$PORT" deploy/docker-compose.yml deploy/remote-deploy.sh deploy/bot.env.example "$SERVER:$DIR/"
# Образ идёт прямо по SSH, без промежуточного файла.
docker save "$tag" | gzip | ssh -p "$PORT" "$SERVER" "gunzip | docker load"

echo "▶ Запуск"
ssh -p "$PORT" "$SERVER" "IMAGE=$tag PULL=0 sh ~/$DIR/remote-deploy.sh"
