FROM rust:1.60 as build

# create a new empty shell project
RUN cargo new --bin csgo-matchbot
WORKDIR /csgo-matchbot

COPY . .

RUN cargo build --release

# our final base
FROM rust:1.60-slim-buster

# copy the build artifact from the build stage
RUN apt update
RUN apt-get install libpq5 -y
COPY --from=build /csgo-matchbot/target/release/csgo-matchbot .
# set the startup command to run your binary
CMD ["./csgo-matchbot"]
