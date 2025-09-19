env:
	@. ${HOME}/export-esp.sh

# run:
# 	cargo run -r

# erase:
# 	espflash erase-flash --port /dev/ttyACM0

# check:
# 	cargo check --release

# flash:
# 	espflash flash --port /dev/ttyACM0 --chip esp32s3 target/xtensa-esp32s3-none-elf/release/ruuvi-listener

# monitor:
# 	cargo espflash monitor --port /dev/ttyACM0 --baud 115200

build-common:
	@echo "Build ruuvi-common:"
	@cargo build -r -p ruuvi-common

build-gateway:
	@echo "Build ruuvi-gateway:"
	@cargo build -r -p ruuvi-gateway

build-listener:
	@echo "Build ruuvi-listener"
	@cargo +esp build -p ruuvi-listener \
		--config ruuvi-listener/.cargo/config.toml \
		--profile ruuvi-listener-release \

build: build-common build-gateway build-listener

run-gateway:
	@echo "Run ruuvi-gateway:"
	@cargo run -r -p ruuvi-gateway

run-listener:
	@echo "run ruuvi-listener:"
	@cargo +esp run -p ruuvi-listener \
		--config ruuvi-listener/.cargo/config.toml \
		--profile ruuvi-listener-release \