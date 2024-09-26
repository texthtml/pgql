FROM rust:1.81 as dev

RUN cargo install watchexec

WORKDIR /app

ENTRYPOINT [ "watchexec", "-i", "target", "-r", "cargo run --color always" ]

FROM node:12 as dev-tests

RUN apt update

RUN apt install -y jq
RUN apt install -y postgresql-client

RUN yarn global add get-graphql-schema

# install envsubst
RUN apt install -y gettext-base

ENTRYPOINT [ "/app/tests/run.sh" ]
