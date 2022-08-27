FROM rust:alpine AS build

RUN apk add --no-cache build-base openssl-dev libpq-dev && mkdir -p /app
COPY . /app
WORKDIR /app
RUN cargo build --release && strip target/release/csgo-matchbot

FROM scratch
COPY --from=build /app/target/release/csgo-matchbot .
CMD [ "/csgo-matchbot" ]
