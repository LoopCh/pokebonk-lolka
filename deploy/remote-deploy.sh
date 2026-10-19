#!/bin/sh
# Выполняется НА VPS из каталога с docker-compose.yml.
# Его вызывают и GitHub Actions, и локальные скрипты деплоя.
#
#   IMAGE=ghcr.io/user/pokebonk-lolka:sha-abc1234 sh remote-deploy.sh
#   IMAGE=pokebonk-lolka:local-abc1234 PULL=0 sh remote-deploy.sh   # образ уже загружен
set -eu

: "${IMAGE:?нужна переменная IMAGE}"
cd "$(dirname "$0")"

if [ ! -f bot.env ]; then
    echo "❌ Нет bot.env. Сделайте: cp bot.env.example bot.env && nano bot.env"
    exit 1
fi

echo "▶ Разворачиваю $IMAGE"
printf 'IMAGE=%s\n' "$IMAGE" > .env

if [ "${PULL:-1}" = "1" ]; then
    docker compose pull
fi
docker compose up -d --remove-orphans

# Убеждаемся, что контейнер не упал сразу (например, из-за ошибки в bot.env).
sleep 10
status=$(docker inspect -f '{{.State.Status}} {{.RestartCount}}' pokebonk-lolka)
docker compose logs --tail 20
if [ "$status" != "running 0" ]; then
    echo "❌ Бот не запустился (состояние: $status)"
    exit 1
fi

# Удаляем старые образы, чтобы не забивать диск.
docker image prune -f >/dev/null
echo "✅ Готово: $IMAGE"
