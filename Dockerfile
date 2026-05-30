FROM rust:1-alpine3.23

RUN apk add --no-cache build-base
EXPOSE 9090

WORKDIR /usr/src

COPY ruuvi-schema ./ruuvi-schema
COPY ruuvi-gateway ./ruuvi-gateway
COPY .env ./ruuvi-gateway/.env

RUN cargo install --path ruuvi-gateway

CMD ["ruuvi-gateway"]