FROM rust:1.72.0 as build-env

WORKDIR /app
RUN USER=root cargo new --bin bite
COPY ./Cargo.toml ./Cargo.lock ./bite/
WORKDIR /app/bite
RUN cargo build --release
RUN rm src/*.rs

ADD . .
RUN rm ./target/release/deps/bite*
RUN cargo build --release

FROM gcr.io/distroless/cc
COPY --from=build-env /app/bite/target/release/bite /
CMD ["./bite"]
