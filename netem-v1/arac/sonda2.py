#!/usr/bin/env python3
"""Araç kanıtı sondası (ürün kodu değil). kullanım: sonda2.py a|b <A_IP> <B_IP> <PORT> <DIZIN>
a (ns-A): sync/t0 yazar; ileri patlamalar: t=0 60 paket (kimlik 0..59), t=2,5 20 paket (60..79) —
  hız basamakları 1,5 s ve 4,0 s'de kuyruk doluyken; t=10 80 paket (100..179) — FIFO 64 sınaması.
  IP boyu 1000 B. B'den gelen dönüş patlamasının varışlarını yazar.
b (ns-B): varışları yazar; t0 + 0,5 s'de A'ya 8 × 48 B (IP) dönüş patlaması."""
import json, os, socket, struct, sys, threading, time
role, a_ip, b_ip, port, d = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), sys.argv[5]
me, peer = (a_ip, b_ip) if role == "a" else (b_ip, a_ip)
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 4 << 20)
s.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, 4 << 20)
s.bind((me, port))
out = open(f"{d}/sonda_{role}.jsonl", "w", buffering=1)
lock = threading.Lock()
def w(rec):
    with lock:
        out.write(json.dumps(rec) + "\n")
def reader():
    while True:
        data = s.recv(65535)
        w({"k": "rx", "seq": struct.unpack("!I", data[:4])[0], "t_ns": time.monotonic_ns(),
           "t_unix_ns": time.time_ns(), "udp_len": len(data)})
threading.Thread(target=reader, daemon=True).start()
def burst(first, n, ip_len):
    pad = b"\0" * (ip_len - 28 - 4)
    for i in range(first, first + n):
        s.sendto(struct.pack("!I", i) + pad, (peer, port))
        w({"k": "tx", "seq": i, "t_ns": time.monotonic_ns(), "t_unix_ns": time.time_ns(), "ip_len": ip_len})
def until(ns):
    while time.time_ns() < ns:
        time.sleep(0.0005)
if role == "a":
    os.makedirs(f"{d}/sync", exist_ok=True)
    t0 = time.time_ns()
    with open(f"{d}/sync/t0.tmp", "w") as f:
        f.write(str(t0))
    os.rename(f"{d}/sync/t0.tmp", f"{d}/sync/t0")
    w({"k": "t0", "t0_unix_ns": t0})
    until(t0 + 150_000_000)  # basamak uygulayıcısı t0'ı görsün
    base = time.time_ns(); w({"k": "taban", "t_unix_ns": base})
    burst(0, 60, 1000)
    until(base + 2_500_000_000); burst(60, 20, 1000)
    until(base + 10_000_000_000); burst(100, 80, 1000)
    until(base + 17_000_000_000)
else:
    while not os.path.exists(f"{d}/sync/t0"):
        time.sleep(0.005)
    t0 = int(open(f"{d}/sync/t0").read())
    until(t0 + 650_000_000)
    burst(0, 8, 48)
    while True:
        time.sleep(1)
