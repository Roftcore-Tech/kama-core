#!/usr/bin/env python3
"""[KAPI-1 W5] AF_PACKET per-interface paket kaydı — bağımlılıksız capture katmanı.

tcpdump bu makinede yok; bu araç AF_PACKET/SOCK_RAW ile arayüzdeki HER IPv4 paketini
yön bilgisiyle (sll_pkttype: PACKET_OUTGOING=4 → "out", diğerleri → "in") JSONL'e yazar:
  {"t_ns": <CLOCK_MONOTONIC ns>, "dir": "out|in", "ip_len": N, "proto": P, "sport": S, "dport": D}

Saat: time.monotonic_ns() — aynı host'taki tüm netns'lerde AYNI kernel saati (KAPI-1 §7).
Kapsam: yalnız gördüğünü kaydeder; OTORİTER enjeksiyon sayacı BudgetGovernor'dadır (W2),
bu katman bağımsız çapraz-teyittir. tcpdump kurulursa pcap YANINA eklenir (kapi3_harness).

Kullanım: pkt_capture.py <iface> <out.jsonl>   (SIGTERM ile temiz kapanır; line-buffered)
"""

import json
import signal
import socket
import sys
import time

ETH_P_ALL = 3
PACKET_OUTGOING = 4


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: pkt_capture.py <iface> <out.jsonl>", file=sys.stderr)
        return 2
    iface, out_path = sys.argv[1], sys.argv[2]
    s = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(ETH_P_ALL))
    # kbps-sınıfı trafik için fazlasıyla geniş tampon (drop'suz kayıt).
    s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 8 * 1024 * 1024)
    s.bind((iface, 0))
    f = open(out_path, "w", buffering=1)

    def term(_sig, _frm):
        # AF_PACKET ring drop sayacı — kayıt bütünlüğü kanıtı (drops>0 → capture eksik) [#32]
        try:
            st = s.getsockopt(263, 6, 8)  # SOL_PACKET, PACKET_STATISTICS: (packets, drops) u32
            pkts = int.from_bytes(st[0:4], sys.byteorder)
            drops = int.from_bytes(st[4:8], sys.byteorder)
            f.write(json.dumps({"capture_stats": {"packets": pkts, "drops": drops}}) + "\n")
        except OSError:
            pass
        f.flush()
        f.close()
        sys.exit(0)

    signal.signal(signal.SIGTERM, term)
    signal.signal(signal.SIGINT, term)
    # Hazır işareti: capture başlamadan koşu başlamasın (yarış önleme).
    print("READY", flush=True)

    while True:
        data, addr = s.recvfrom(65535)
        t = time.monotonic_ns()
        if len(data) < 34 or data[12:14] != b"\x08\x00":  # yalnız IPv4
            continue
        ip = data[14:]
        rec = {
            "t_ns": t,
            "t_unix_ns": time.time_ns(),  # RunMeta.t0_unix_ns ile hizalama [inceleme #30]
            "dir": "out" if addr[2] == PACKET_OUTGOING else "in",
            "ip_len": (ip[2] << 8) | ip[3],
            "proto": ip[9],
        }
        if ip[9] == 17:  # UDP portları (akış ayrıştırma için)
            ihl = (ip[0] & 0x0F) * 4
            if len(ip) >= ihl + 4:
                rec["sport"] = (ip[ihl] << 8) | ip[ihl + 1]
                rec["dport"] = (ip[ihl + 2] << 8) | ip[ihl + 3]
        f.write(json.dumps(rec) + "\n")


if __name__ == "__main__":
    sys.exit(main())
