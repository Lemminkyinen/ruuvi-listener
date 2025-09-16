env:
	@. ${HOME}/export-esp.sh

check:
	cargo check --release

build:
	cargo build --release

flash:
	cargo espflash flash --port /dev/ttyACM0 --release

monitor:
	cargo espflash monitor --port /dev/ttyACM0 --baud 115200