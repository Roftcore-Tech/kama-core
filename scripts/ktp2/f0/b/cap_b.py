#!/usr/bin/env python3
"""[KTP2-F0 / Ajan B] Uç-arayüz paket kaydı — çerçeve başlığıyla (salt gözlem).

`scripts/netem/pkt_capture.py` yalnız ip_len/port yazar; B şeridinin sorularına (yeniden gönderimin
faydalı/gereksiz ayrımı, ACK kapsamı, gerçek RTT) çerçeve türü, birim kimliği ve PN gerekir. Bench
mühür katmanı (IdentitySeam) şifrelemez: çerçeve düz metindir, PN etiketin ilk 8 baytındadır
(crates/client-core/src/seam.rs:105-108, 166). Bu araç o baytları kaydeder; trafiğe dokunmaz.

Satır biçimi (CSV, başlıksız):
  t_unix_ns,dir,proto,ip_len,sport,dport,x
    dir   : o = arayüzden çıkan (PACKET_OUTGOING), i = giren
    x     : UDP → UDP yükünün onaltılığı; yük ≤ FULL_MAX ise TAM, değilse ilk HEAD + ':' + son TAIL
            TCP → seq:ack:flags:doff:plen   (doff bayt cinsinden TCP başlığı)
            diğer → boş
Saat: time.time_ns() (CLOCK_REALTIME; bench RunMeta.t0_unix_ns ile aynı saat).
Kapanış: SIGTERM/SIGINT → AF_PACKET düşüş sayacı son satıra '#stats,<paket>,<düşüş>' yazılır.

Kullanım: python3 -I cap_b.py <arayüz> <çıktı.csv>   (hazır olunca stdout'a READY)
"""
import signal
import socket
import sys
import time

ETH_P_ALL = 3
PACKET_OUTGOING = 4
FULL_MAX = 200
HEAD = 40
TAIL = 16


def main() -> int:
    if len(sys.argv) != 3:
        print("kullanım: cap_b.py <arayüz> <çıktı.csv>", file=sys.stderr)
        return 2
    iface, out_path = sys.argv[1], sys.argv[2]
    s = socket.socket(socket.AF_PACKET, socket.SOCK_RAW, socket.htons(ETH_P_ALL))
    s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 8 * 1024 * 1024)
    s.bind((iface, 0))
    f = open(out_path, "w", buffering=1 << 16)

    def term(_sig, _frm):
        try:
            st = s.getsockopt(263, 6, 8)  # SOL_PACKET, PACKET_STATISTICS: (packets, drops) u32
            pk = int.from_bytes(st[0:4], sys.byteorder)
            dr = int.from_bytes(st[4:8], sys.byteorder)
            f.write(f"#stats,{pk},{dr}\n")
        except OSError:
            pass
        f.flush()
        f.close()
        sys.exit(0)

    signal.signal(signal.SIGTERM, term)
    signal.signal(signal.SIGINT, term)
    print("READY", flush=True)
    while True:
        data, addr = s.recvfrom(65535)
        t = time.time_ns()
        if len(data) < 34 or data[12:14] != b"\x08\x00":
            continue
        ip = data[14:]
        ihl = (ip[0] & 0x0F) * 4
        ip_len = (ip[2] << 8) | ip[3]
        proto = ip[9]
        d = "o" if addr[2] == PACKET_OUTGOING else "i"
        sport = dport = 0
        x = ""
        l4 = ip[ihl:ip_len]
        if proto == 17 and len(l4) >= 8:
            sport = (l4[0] << 8) | l4[1]
            dport = (l4[2] << 8) | l4[3]
            pl = l4[8:]
            x = pl.hex() if len(pl) <= FULL_MAX else pl[:HEAD].hex() + ":" + pl[-TAIL:].hex()
        elif proto == 6 and len(l4) >= 20:
            sport = (l4[0] << 8) | l4[1]
            dport = (l4[2] << 8) | l4[3]
            seq = int.from_bytes(l4[4:8], "big")
            ack = int.from_bytes(l4[8:12], "big")
            doff = (l4[12] >> 4) * 4
            flags = l4[13]
            plen = ip_len - ihl - doff
            x = f"{seq}:{ack}:{flags}:{doff}:{plen}"
        f.write(f"{t},{d},{proto},{ip_len},{sport},{dport},{x}\n")


if __name__ == "__main__":
    sys.exit(main())
