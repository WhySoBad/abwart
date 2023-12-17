FROM rust:alpine3.18 as builder

WORKDIR /app
RUN apk add --no-cache musl-dev openssl-dev

COPY . .
RUN cargo build --release

FROM alpine:3.19 as runner

WORKDIR /app
COPY --from=builder /app/target/release/abwart ./abwart

CMD [ "./abwart" ]