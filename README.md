
### Prerequisites:

#### Tooling:

```bash
cargo install espup --locked
cargo install esp-generate
cargo install cargo-espflash --locked
```

#### Espup installation: 
`espup install`

#### Template generation
(Not required, just a note to myself)
`esp-generate --chip esp32s3 -o embassy -o unstable-hal -o alloc -o wifi -o ble-bleps -o log -o esp-backtrace -o vscode`

#### Setting environment variables
Has to be done in every session `. $(HOME)/export-esp.sh`

#### Attaching ESP for WSL
```powershell
usbipd list
usbipd bind --busid 3-1
usbipd attach --wsl --busid 3-1
```

