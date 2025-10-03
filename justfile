env:
	@. ${HOME}/export-esp.sh

# run:
# 	cargo run -r

# erase:
# 	espflash erase-flash --port /dev/ttyACM0

# flash:
# 	espflash flash --port /dev/ttyACM0 --chip esp32s3 target/xtensa-esp32s3-none-elf/release/ruuvi-listener

# monitor:
# 	cargo espflash monitor --port /dev/ttyACM0 --baud 115200

check-common:
	@echo "Check ruuvi-common:"
	@cargo check -r -p ruuvi-common \
		--manifest-path "ruuvi-common/Cargo.toml"

check-gateway:
	@echo "Check ruuvi-gateway:"
	@cargo check -r -p ruuvi-gateway \
		--manifest-path "ruuvi-gateway/Cargo.toml"

check-listener:
	@echo "Check ruuvi-listener"
	@cargo +esp check -p ruuvi-listener \
		--config ruuvi-listener/.cargo/config.toml \
		--profile release \
		--manifest-path "ruuvi-listener/Cargo.toml"

check: check-common check-gateway check-listener

build-common:
	@echo "Build ruuvi-common:"
	@cargo build -r -p ruuvi-common \
		--manifest-path "ruuvi-common/Cargo.toml"

build-gateway:
	@echo "Build ruuvi-gateway:"
	@cargo build -r -p ruuvi-gateway \
		--manifest-path "ruuvi-gateway/Cargo.toml"

build-listener:
	@echo "Build ruuvi-listener"
	@cargo +esp build -p ruuvi-listener \
		--config ruuvi-listener/.cargo/config.toml \
		--profile release \
		--manifest-path "ruuvi-listener/Cargo.toml"

build: build-common build-gateway build-listener

run-gateway:
	@echo "Run ruuvi-gateway:"
	@cargo run -r -p ruuvi-gateway \
		--manifest-path "ruuvi-gateway/Cargo.toml"

run-listener:
	@echo "run ruuvi-listener:"
	@cargo +esp run -p ruuvi-listener \
		--config ruuvi-listener/.cargo/config.toml \
		--profile release \
		--manifest-path "ruuvi-listener/Cargo.toml"

rust-analyzer $project:
	@.venv/bin/python tools/toggle_rust_analyzer.py ${project}