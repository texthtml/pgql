#!/usr/bin/env sh

docker-compose run --rm \
    --entrypoint sh \
    --user root \
    server \
    -c 'chown 1000:1000 --recursive /app/target /cargo'
