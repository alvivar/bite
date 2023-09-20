FROM rust:1.72.1 as build-env

WORKDIR /app
RUN cargo new --bin bite
COPY ./Cargo.toml ./Cargo.lock ./bite/

WORKDIR /app/bite
RUN cargo build --release
RUN rm ./src/*.rs
RUN rm ./target/release/deps/bite*

COPY ./src ./src
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=build-env /app/bite/target/release/bite /
CMD ["./bite"]
