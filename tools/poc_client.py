from __future__ import annotations

import argparse
import hashlib
import hmac
import os
import random
import socket
import struct
import sys
import time
from collections.abc import Iterable
from dataclasses import dataclass

from dotenv import load_dotenv

MAGIC = b"RGW1"
VERSION = 0x01
FLAGS_DEFAULT = 0x00
HANDSHAKE_SIZE = 52
HMAC_LEN = 32

# ACK / Error markers (from server spec)
ACK_MARKER = 0x03
ERR_MARKER = 0x10


@dataclass
class RuuviRawV2:
    format: int = 0x05
    temp: int = 4934
    humidity: int = 19595
    pressure: int = 49453
    acc_x: int = 940
    acc_y: int = 428
    acc_z: int = 24
    power_info: int = 23638
    movement_counter: int = 250
    measurement_seq: int = 9463
    mac: bytes = bytes.fromhex("DEADBEEF0609")  # 6 bytes

    def to_postcard_some(self) -> bytes:
        """Serialize as postcard Option::Some(self)."""
        if len(self.mac) != 6:
            raise ValueError("mac must be 6 bytes")
        parts: list[bytes] = []
        parts.append(b"\x01")  # Option discriminant: Some
        parts.append(struct.pack("<B", self.format))
        parts.append(struct.pack("<h", self.temp))
        parts.append(struct.pack("<H", self.humidity))
        parts.append(struct.pack("<H", self.pressure))
        parts.append(struct.pack("<h", self.acc_x))
        parts.append(struct.pack("<h", self.acc_y))
        parts.append(struct.pack("<h", self.acc_z))
        parts.append(struct.pack("<H", self.power_info))
        parts.append(struct.pack("<B", self.movement_counter))
        parts.append(struct.pack("<H", self.measurement_seq))
        parts.append(self.mac)
        return bytes().join(parts)


def parse_device_id(s: str) -> bytes:
    raw = s.replace(":", "").replace("-", "")
    if len(raw) != 12:
        raise ValueError("device id must be 6 bytes (12 hex chars)")
    return bytes.fromhex(raw)


def build_handshake(auth_key: bytes, device_id: bytes, flags: int) -> bytes:
    if len(device_id) != 6:
        raise ValueError("device_id must be 6 bytes")
    buf = bytearray(HANDSHAKE_SIZE)
    # MAGIC
    buf[0:4] = MAGIC
    buf[4] = VERSION
    buf[5] = flags & 0xFF
    buf[6:12] = device_id
    # Nonce: 8 random bytes
    nonce = random.getrandbits(64).to_bytes(8, "big")
    buf[12:20] = nonce
    # HMAC over first 20 bytes
    tag = hmac.new(auth_key, buf[0:20], hashlib.sha256).digest()
    buf[20:52] = tag
    return bytes(buf)


def send_handshake(
    sock: socket.socket, auth_key: bytes, device_id: bytes, flags: int
) -> None:
    hs = build_handshake(auth_key, device_id, flags)
    sock.sendall(hs)
    resp = sock.recv(1)
    if len(resp) == 0:
        raise RuntimeError("EOF waiting handshake reply")
    code = resp[0]
    if code == 0x01:
        print("[handshake] accepted")
        return
    errors = {0xFF: "bad_magic", 0xFE: "bad_version", 0xFD: "bad_hmac"}
    raise RuntimeError(f"Handshake rejected: {errors.get(code, hex(code))}")


def build_frame(ftype: int, payload: bytes) -> bytes:
    if not (0 <= ftype <= 0xFF):
        raise ValueError("ftype range")
    total_len = 1 + len(payload)
    if total_len == 0 or total_len > 64 * 1024:
        raise ValueError("invalid frame length")
    return struct.pack(">I", total_len) + bytes([ftype]) + payload


def recv_exact(sock: socket.socket, n: int) -> bytes:
    chunks = []
    need = n
    while need > 0:
        part = sock.recv(need)
        if not part:
            raise RuntimeError("EOF during recv_exact")
        chunks.append(part)
        need -= len(part)
    return b"".join(chunks)


def send_data_frame(sock: socket.socket, ruuvi: RuuviRawV2) -> None:
    payload = ruuvi.to_postcard_some()
    frame = build_frame(0x01, payload)
    sock.sendall(frame)
    ack = recv_exact(sock, 2)
    if ack[0] == ACK_MARKER:
        if ack[1] == 0x01:
            print("[data] ACK (0x03,0x01)")
        else:
            print(f"[data] ACK variant code=0x{ack[1]:02X}")
    elif ack[0] == ERR_MARKER:
        raise RuntimeError(f"[data] server error code=0x{ack[1]:02X}")
    else:
        raise RuntimeError(f"[data] unexpected ack bytes={ack.hex()}")


def send_ping(sock: socket.socket) -> None:
    frame = build_frame(0x02, b"")
    sock.sendall(frame)
    ack = recv_exact(sock, 2)
    if ack == b"\x03\x02":
        print("[ping] ACK")
    else:
        print(f"[ping] unexpected={ack.hex()}")


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Ruuvi Gateway PoC client")
    p.add_argument("--host", default="127.0.0.1", help="Server host/IP")
    p.add_argument("--port", type=int, default=9090, help="Server port (default 9090)")
    p.add_argument("--auth-key", help="Shared AUTH_KEY (or AUTH_KEY env)")
    p.add_argument(
        "--device-id",
        default="DE:AD:BE:EF:06:06",
        help="6-byte device id hex (DE:AD:BE:EF:06:09)",
    )
    p.add_argument(
        "--frames", type=int, default=1, help="Number of data frames to send"
    )
    p.add_argument("--interval", type=float, default=2.0, help="Seconds between frames")
    p.add_argument("--ping", action="store_true", help="Send a ping before data frames")
    return p.parse_args()


def main(argv: Iterable[str] | None = None) -> int:
    load_dotenv()
    args = parse_args()
    auth_key = (args.auth_key or os.getenv("AUTH_KEY") or "").encode()
    if not auth_key:
        print("ERROR: Provide --auth-key or AUTH_KEY env", file=sys.stderr)
        return 2

    device_id = parse_device_id(args.device_id)
    ruuvi_sample = RuuviRawV2()

    addr = (args.host, args.port)
    print(f"Connecting to {addr} ...")
    with socket.create_connection(addr, timeout=5) as sock:
        sock.settimeout(5)
        send_handshake(sock, auth_key, device_id, FLAGS_DEFAULT)
        if args.ping:
            send_ping(sock)
        for i in range(args.frames):
            send_data_frame(sock, ruuvi_sample)
            if i + 1 < args.frames:
                time.sleep(args.interval)
    print("Done.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
