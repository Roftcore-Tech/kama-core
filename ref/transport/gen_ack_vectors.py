#!/usr/bin/env python3
"""ACK çerçevesi (transport-control 0x14 V2 / 0x10 V1) sabit kodlama vektörleri — BAĞIMSIZ Python
referansından üretilir.

    python3 -I ref/transport/gen_ack_vectors.py            # vectors/transport/ altına yazar
    python3 -I ref/transport/gen_ack_vectors.py --check    # yeniden üretir, DİSKTEKİYLE karşılaştırır

KURAL: hiçbir beklenen bayt Rust'tan üretilmez. Tek kaynak `specs/adaptive-transport-engine-spec.md`
§2 / §2.1 / §2.2 tel düzeni (fixed-width big-endian, varint yok):

    V2 0x14: frame_type u8 · ack_epoch u8 · largest_seq u16 · ack_delay u16 (256 µs) ·
             block0_len u16 · range_count u8 · ranges[]{gap u16, len u16} ·
             seen_count u32 · arrival_rate u16                       (15 B + 4 B × aralık)
    V1 0x10: aynı baş, ölçüm eki YOK                                   ( 9 B + 4 B × aralık)

Alınan kümeden çerçeve: kümede ardışık bloklar, en yüksek blok block0; altındakiler azalan sırada
(gap, len) çiftleri, en çok MAX_ACK_RANGES = 16 (en yüksek seq'ler korunur). İlk aralığın gap'i
block0'ın alt ucu − bloğun üst ucu; sonrakilerin gap'i (önceki bloğun alt ucu − 1) − bloğun üst ucu
(motorun §2 azalan yeniden kurgusu: cursor −= gap; blok = [cursor−len+1 .. cursor]; cursor −= len).
Alanlar u16 tel değeri (mod 2^16).

Vaka `kind` sözlüğü (tüketici: `crates/kama-integration/tests/ack_ref_vectors.rs`):
  from-received   girdi = alınan tam sayaç kümesi + alanlar; beklenen = V2 çerçeve baytları.
                  `acked` = çerçevenin taşıdığı bilgi kümesi (en yüksek 1+16 blok) — tüketici
                  çözülmüş çerçevenin teyit kümesini buna karşı sınar.
  encode-fields   girdi = çerçeve alanları (V1 ya da V2); beklenen = baytlar.
  decode          girdi = ham bayt; pozitifte beklenen = aynı baytlar (çöz → yeniden kodla).
                  negatifte `expected.reason`:
                    too-many-ack-ranges · truncated · trailing-bytes · unknown-frame-type
"""

import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SUITE = "transport-ack-frame-v1"
SPEC = "adaptive-transport-engine-spec@s2.2"
GEN = "ref/transport/gen_ack_vectors.py 1"
MAX_RANGES = 16
SENTINEL = 0xFFFF


def frame(ft, epoch, largest, delay, b0, ranges, seen=0, arrival=SENTINEL):
    out = struct.pack(">BBHHHB", ft, epoch, largest & 0xFFFF, delay, b0, len(ranges))
    for gap, ln in ranges:
        out += struct.pack(">HH", gap & 0xFFFF, ln)
    if ft == 0x14:
        out += struct.pack(">IH", seen, arrival)
    return out


def blocks_desc(received):
    s = sorted(set(received))
    blocks = []
    for x in reversed(s):
        if blocks and blocks[-1][0] == x + 1:
            blocks[-1][0] = x
        else:
            blocks.append([x, x])
    return [tuple(b) for b in blocks]


def from_received(received, epoch, delay, seen, arrival):
    bl = blocks_desc(received)
    lo0, hi0 = bl[0]
    ranges = []
    cursor = lo0
    for lo, hi in bl[1:1 + MAX_RANGES]:
        ranges.append((cursor - hi, hi - lo + 1))
        cursor = lo - 1
    acked = sorted(x for lo, hi in bl[:1 + MAX_RANGES] for x in range(lo, hi + 1))
    return frame(0x14, epoch, hi0, delay, hi0 - lo0 + 1, ranges, seen, arrival), acked


def case_from_received(name, received, note, epoch=0, delay=0, seen=None, arrival=SENTINEL):
    seen = len(set(received)) if seen is None else seen
    b, acked = from_received(received, epoch, delay, seen, arrival)
    return {
        "id": f"{SUITE}.from-received.{name}",
        "kind": "from-received",
        "note": note,
        "spec_ref": "adaptive-transport-engine-spec.md §2, §2.1, §2.2",
        "input": {
            "received": sorted(set(received)),
            "ack_epoch": epoch,
            "ack_delay": delay,
            "seen_count": seen,
            "arrival_rate": arrival,
            "acked": acked,
        },
        "expected_hex": b.hex(),
    }


def case_fields(name, ft, epoch, largest, delay, b0, ranges, seen, arrival, note):
    return {
        "id": f"{SUITE}.encode-fields.{name}",
        "kind": "encode-fields",
        "note": note,
        "spec_ref": "adaptive-transport-engine-spec.md §2.2",
        "input": {
            "frame_type": ft,
            "ack_epoch": epoch,
            "largest_seq": largest,
            "ack_delay": delay,
            "block0_len": b0,
            "ranges": [[g, ln] for g, ln in ranges],
            "seen_count": seen,
            "arrival_rate": arrival,
        },
        "expected_hex": frame(ft, epoch, largest, delay, b0, ranges, seen, arrival).hex(),
    }


def case_decode_ok(name, b, note):
    return {
        "id": f"{SUITE}.decode.{name}",
        "kind": "decode",
        "note": note,
        "spec_ref": "adaptive-transport-engine-spec.md §2.2",
        "input_hex": b.hex(),
        "expected_hex": b.hex(),
    }


def case_reject(name, b, reason, note):
    return {
        "id": f"{SUITE}.decode.{name}",
        "kind": "decode",
        "note": note,
        "spec_ref": "adaptive-transport-engine-spec.md §2.1 (MAX_ACK_RANGES), §2.2",
        "input_hex": b.hex(),
        "expected": {"result": "reject", "reason": reason},
    }


def gen():
    burst = [x for x in range(1000, 1300) if not (1010 <= x < 1014 or 1100 <= x < 1130 or x % 17 == 0)]
    cases = [
        case_from_received("contiguous", [5, 6, 7], "tek blok, aralık yok"),
        case_from_received("three-singletons", [3, 5, 7], "üç tekil blok, iki aralık"),
        case_from_received("u16-wrap", [65535, 65536, 65537], "tam sayaç u16 sınırını aşar; tel 0xffff,0,1"),
        case_from_received("seventeen-blocks-exact", list(range(0, 34, 2)),
                           "17 blok = block0 + 16 aralık: tavan, kesme YOK"),
        case_from_received("eighteen-blocks-truncated", list(range(0, 36, 2)),
                           "18 blok: en eski blok düşer (en yüksek seq'ler korunur)"),
        case_from_received("forty-singletons-truncated", [i * 3 for i in range(40)],
                           "40 tekil blok: yalnız en yüksek 17 blok taşınır"),
        case_from_received("large-counter-burst", [(1 << 40) + x for x in burst],
                           "büyük tam sayaç + patlamalı kayıp", epoch=1, delay=1234, seen=999, arrival=500),
        case_from_received("delay-max", [10, 11, 12, 20], "ack_delay tavanı 65535 (×256 µs)", delay=65535),
        case_fields("v2-max-16-ranges", 0x14, 7, 0xFFFE, 0x0102, 3, [(i + 1, 2 * i + 1) for i in range(16)],
                    0xDEADBEEF, 0x1234, "en büyük V2 çerçevesi: 15 + 16×4 = 79 B"),
        case_fields("v1-two-ranges", 0x10, 0, 100, 40, 3, [(2, 1), (1, 1)], 0, SENTINEL,
                    "V1 (yalnız çözme uyumluluğu; ölçüm eki yok): 9 + 2×4 = 17 B"),
        case_decode_ok("v2-roundtrip", frame(0x14, 0, 7, 100, 3, [(2, 1)], 42, 500), "çöz → yeniden kodla"),
        case_decode_ok("v1-roundtrip", frame(0x10, 0, 7, 100, 3, []), "V1 çöz → yeniden kodla (V1 kalır)"),
    ]
    v2 = frame(0x14, 0, 7, 0, 1, [(1, 1), (2, 2)], 3, SENTINEL)
    over = struct.pack(">BBHHHB", 0x14, 0, 7, 0, 1, 17) + b"\x00" * 68 + struct.pack(">IH", 0, SENTINEL)
    over_v1 = struct.pack(">BBHHHB", 0x10, 0, 7, 0, 1, 255) + b"\x00" * 1020
    cases += [
        case_reject("range-count-17", over, "too-many-ack-ranges", "aralık sayısı 17 > 16 (gövde tam)"),
        case_reject("v1-range-count-255", over_v1, "too-many-ack-ranges", "V1, aralık sayısı 255"),
        case_reject("empty", b"", "truncated", "boş girdi"),
        case_reject("header-cut", v2[:5], "truncated", "baş kesik"),
        case_reject("range-cut", v2[:11], "truncated", "aralık ortasında kesik"),
        case_reject("appendix-cut", v2[:-1], "truncated", "V2 ölçüm eki kesik"),
        case_reject("trailing-one", v2 + b"\x00", "trailing-bytes", "1 artık bayt"),
        case_reject("v1-with-appendix", frame(0x10, 0, 7, 0, 1, []) + b"\x00" * 6, "trailing-bytes",
                    "V1 başı + V2 eki: 6 artık bayt"),
        case_reject("unknown-0x15", b"\x15" + v2[1:], "unknown-frame-type", "0x15 tanımsız"),
        case_reject("unknown-0x00", b"\x00" + v2[1:], "unknown-frame-type", "0x00 ACK değil"),
    ]
    return {
        "schema_version": 2,
        "suite": SUITE,
        "spec_version": SPEC,
        "generator": GEN,
        "description": "ACK çerçevesi sabit kodlama vektörleri (bugünkü V2 0x14 emisyonu + V1 0x10 çözme "
                       "uyumluluğu). Bayt eşitliği YALNIZ bu dosyadaki sabit kodlama için iddia edilir; "
                       "farklı bir kodlama aynı bilgiyi taşıyabilir.",
        "case_count": len(cases),
        "cases": cases,
    }


def _sorted_deep(v):
    if isinstance(v, dict):
        return {k: _sorted_deep(v[k]) for k in sorted(v)}
    if isinstance(v, list):
        return [_sorted_deep(x) for x in v]
    return v


def render(doc):
    out = dict(doc)
    out["cases"] = [{k: (_sorted_deep(v) if k == "input" else v) for k, v in c.items()} for c in doc["cases"]]
    return json.dumps(out, indent=2, ensure_ascii=True) + "\n"


def main(argv):
    path = os.path.join(ROOT, "vectors", "transport", SUITE + ".json")
    text = render(gen())
    rel = os.path.relpath(path, ROOT)
    if "--check" in argv:
        try:
            with open(path, encoding="utf-8") as f:
                on_disk = f.read()
        except OSError:
            on_disk = None
        if on_disk != text:
            print(f"FARK: {rel} üreticinin çıktısıyla aynı değil")
            return 1
        print(f"aynı: {rel}")
        return 0
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    print(f"yazıldı: {rel}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
