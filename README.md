# RuuviTag BLE Scanner for ESP32S3

### Components
- Ruuvi Listener: ESP32S3 baremetal firmware that scans BLE advertisements from Ruuvi tags and forwards data over TCP to the gateway. TCP connection is encrypted with `noise` protocol framework.
- Ruuvi Gateway: Server that receives encrypted sensor data from listeners (TODO and saves the data into a database)


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

