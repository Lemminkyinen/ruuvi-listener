env:
	@. ${HOME}/export-esp.sh

run:
	cargo run -r

erase:
	espflash erase-flash --port /dev/ttyACM0

check:
	cargo check --release

build:
	cargo build --release

flash:
	espflash flash --port /dev/ttyACM0 --chip esp32s3 target/xtensa-esp32s3-none-elf/release/ruuvi-listener

monitor:
	cargo espflash monitor --port /dev/ttyACM0 --baud 115200