FROM rust:1-alpine3.23

RUN apk add --no-cache build-base
EXPOSE 9090

WORKDIR /usr/src/ruuvi-gateway

COPY ruuvi-gateway .
COPY ruuvi-schema .
COPY .env .

RUN cargo install --path .

CMD ["ruuvi-gateway"]