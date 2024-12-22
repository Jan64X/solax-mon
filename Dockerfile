FROM rust:1.75-alpine3.19 as builder

WORKDIR /usr/src/app
COPY . .

RUN apk add --no-cache musl-dev perl openssl make
RUN cargo build --release

FROM alpine:3.19

RUN apk add --no-cache \
    sudo \
    curl \
    openssh \
    sshpass \
    perl

WORKDIR /srv/solax-mon

COPY --from=builder /usr/src/app/target/release/solax-mon .
COPY --from=builder /usr/src/app/target/release/ssh .

COPY init.sh .
RUN chmod +x init.sh

ENTRYPOINT ["/srv/solax-mon/init.sh"]