#!/usr/bin/env python3
"""[KAPI-1 W5] UDP echo aracı — harness kabul-pinleri için (ping/ICMP bağımlılığı yok).

server: udp_echo.py server <bind_ip> <port>
client: udp_echo.py client <ip> <port> <count> <timeout_s>
  → stdout tek satır JSON: {"sent":N,"received":M,"rtt_ms_min":..,"rtt_ms_avg":..}
"""

import json
import socket
import sys
import time


def server(ip: str, port: int) -> None:
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.bind((ip, port))
    print("READY", flush=True)
    while True:
        data, addr = s.recvfrom(2048)
        s.sendto(data, addr)


def client(ip: str, port: int, count: int, timeout_s: float) -> None:
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.settimeout(timeout_s)
    rtts = []
    received = 0
    for i in range(count):
        t0 = time.monotonic_ns()
        s.sendto(i.to_bytes(4, "big") + b"\x5a" * 60, (ip, port))
        try:
            while True:
                data, _ = s.recvfrom(2048)
                if data[:4] == i.to_bytes(4, "big"):
                    break
            rtts.append((time.monotonic_ns() - t0) / 1e6)
            received += 1
        except socket.timeout:
            pass
    out = {"sent": count, "received": received}
    if rtts:
        out["rtt_ms_min"] = round(min(rtts), 3)
        out["rtt_ms_avg"] = round(sum(rtts) / len(rtts), 3)
    print(json.dumps(out))


def main() -> int:
    if len(sys.argv) < 4:
        print(__doc__, file=sys.stderr)
        return 2
    if sys.argv[1] == "server":
        server(sys.argv[2], int(sys.argv[3]))
    else:
        client(
            sys.argv[2],
            int(sys.argv[3]),
            int(sys.argv[4]) if len(sys.argv) > 4 else 10,
            float(sys.argv[5]) if len(sys.argv) > 5 else 1.0,
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
