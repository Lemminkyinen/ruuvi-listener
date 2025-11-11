# RuuviTag BLE Scanner for ESP32S3

Supports currently Ruuvi 5 (tag) and Ruuvi E1 (air) formats. 

### Components
- ruuvi-listener: ESP32S3 baremetal firmware that scans BLE extended advertisements from Ruuvi air and tags. Then forwards data over TCP to the gateway. TCP connection is encrypted with `noise` protocol framework.
- ruuvi-gateway: Server that receives encrypted sensor data from listeners and saves the data into a database
- ruuvi-schema: Common schemas for the project. 

### Prerequisites:

#### Tooling:
```bash
cargo install espup --locked
cargo install cargo-espflash --locked
cargo install esp-generate # (Not required, this project template was created with the esp-genarate tool)
```

#### Espup installation: 
`espup install`

#### Template generation
(Not required, this project template was created with the esp-genarate tool)
`esp-generate --chip esp32s3 -o embassy -o unstable-hal -o alloc -o wifi -o ble-trouble -o log -o esp-backtrace -o vscode`

#### Setting environment variables
Has to be done in every session `. $(HOME)/export-esp.sh`

#### Attaching ESP for WSL
```powershell
usbipd list
usbipd bind --busid <bus-id>
usbipd attach --wsl --busid <bus-id>
```

