"""Bağımsız AMP (Application Message Protocol) kodlayıcısı.

Otorite: specs/application-message-protocol.md §2 (kayıt başlığı), §3 (tip registry),
§5 (sıkıştırma wire sözleşmesi). Bu betik `kama-app`'teki Rust codec'inin (B2.1, henüz
yazılmadı) bir kopyası DEĞİLDİR — T-00.1 A2.1 ilkesiyle aynı gerekçe: Rust kodu kendi test
verisini üretirse kendi hatasını onaylar. Bu betik yalnız spec metninden okunarak, stdlib-only
(Python 3.14+, `compression.zstd`) ve production Rust koduyla SIFIR paylaşımla vektör üretir.

Kullanım: `python3.14 ref/amp/encode.py` → `vectors/amp-record-v1.json` +
`vectors/amp-record-reject-v1.json` yazar.
"""

from __future__ import annotations

import json
import struct
from pathlib import Path

from compression import zstd

MSG_VERSION = 0x01

TEXT = 0x01
FILE_BEGIN = 0x02
FILE_CHUNK = 0x03
FILE_END = 0x04
FILE_ABORT = 0x05
PTT_OPEN = 0x06
PTT_CLOSE = 0x07
RECEIPT = 0x08

COMPRESS_FLAG_BIT0 = 0x01
ZSTD_COMPRESSION_ID = 0x01

# Vektör üretimi için sabitlenmiş test parametresi (gerçek değer runtime'da
# TransportCapabilities.max_plaintext_unit_bytes'tan türetilir — AMP §2/A2).
# MAX_BODY_BYTES = min(65535, max_plaintext_unit_bytes - 5); burada max_plaintext_unit_bytes=1024.
TEST_MAX_PLAINTEXT_UNIT_BYTES = 1024
MAX_BODY_BYTES = min(65535, TEST_MAX_PLAINTEXT_UNIT_BYTES - 5)  # 1019
MAX_DECOMPRESSED_MESSAGE = MAX_BODY_BYTES
MAX_EXPANSION_RATIO = 64


def record(msg_type: int, body: bytes, *, flags: int = 0x00, msg_version: int = MSG_VERSION) -> bytes:
    if len(body) > 0xFFFF:
        raise ValueError("body exceeds u16_be body_len")
    return struct.pack(">BBBH", msg_version, msg_type, flags, len(body)) + body


# ---- §3 tip registry: gövde kodlayıcıları ----------------------------------------------------

def text_body(msg_id: int, sent_at_ms: int, text: str) -> bytes:
    text_bytes = text.encode("utf-8")
    return struct.pack(">QQH", msg_id, sent_at_ms, len(text_bytes)) + text_bytes


def file_begin_body(
    transfer_id: int, file_size: int, whole_file_hash: bytes, chunk_count: int, name: str
) -> bytes:
    assert len(whole_file_hash) == 32, "whole_file_hash BLAKE2s-256 = 32B"
    name_bytes = name.encode("utf-8")
    assert len(name_bytes) <= 255
    return (
        struct.pack(">QQ", transfer_id, file_size)
        + whole_file_hash
        + struct.pack(">IB", chunk_count, len(name_bytes))
        + name_bytes
    )


def file_chunk_body(transfer_id: int, offset: int, payload: bytes) -> bytes:
    return struct.pack(">QQH", transfer_id, offset, len(payload)) + payload


def file_end_body(transfer_id: int) -> bytes:
    return struct.pack(">Q", transfer_id)


def file_abort_body(transfer_id: int, reason: int) -> bytes:
    return struct.pack(">QB", transfer_id, reason)


def ptt_open_body(stream_id: int, codec: int, bundle_ms: int) -> bytes:
    return struct.pack(">IBB", stream_id, codec, bundle_ms)


def ptt_close_body(stream_id: int) -> bytes:
    return struct.pack(">I", stream_id)


def receipt_body(ref_msg_id: int, kind: int) -> bytes:
    return struct.pack(">QB", ref_msg_id, kind)


# ---- §5 sıkıştırma wire sözleşmesi ------------------------------------------------------------

def compressed_body(uncompressed: bytes) -> bytes:
    """flags.bit0=1 durumundaki gövde: compression_id(1) | uncompressed_len(4 be) | payload."""
    payload = zstd.compress(uncompressed)
    return struct.pack(">BI", ZSTD_COMPRESSION_ID, len(uncompressed)) + payload


def hx(b: bytes) -> str:
    return b.hex()


def case(id_, kind, *, input_hex=None, input_=None, expected_hex=None, expected=None, note=None, spec_ref=None, flags=None):
    c = {"id": id_, "kind": kind}
    if flags:
        c["flags"] = flags
    if note:
        c["note"] = note
    if spec_ref:
        c["spec_ref"] = spec_ref
    if input_hex is not None:
        c["input_hex"] = input_hex
    if input_ is not None:
        c["input"] = input_
    if expected_hex is not None:
        c["expected_hex"] = expected_hex
    if expected is not None:
        c["expected"] = expected
    return c


def build_positive() -> dict:
    cases = [
        case(
            "amp-record-v1.encode.text",
            "encode",
            note="TEXT (0x01): msg_id, sent_at_ms, text_len, text (UTF-8).",
            spec_ref="application-message-protocol.md §3 0x01 TEXT",
            input_={
                "msg_id": 1,
                "sent_at_ms": 1_758_000_000_000,
                "text": "merhaba",
            },
            expected_hex=hx(record(TEXT, text_body(1, 1_758_000_000_000, "merhaba"))),
        ),
        case(
            "amp-record-v1.encode.file-begin",
            "encode",
            note="FILE_BEGIN (0x02): transfer_id, file_size, whole_file_hash[32], chunk_count, name_len+name.",
            spec_ref="application-message-protocol.md §3 0x02 FILE_BEGIN",
            input_={
                "transfer_id": 42,
                "file_size": 4096,
                "whole_file_hash_hex": "11" * 32,
                "chunk_count": 4,
                "name": "rapor.pdf",
            },
            expected_hex=hx(
                record(
                    FILE_BEGIN,
                    file_begin_body(42, 4096, bytes.fromhex("11" * 32), 4, "rapor.pdf"),
                )
            ),
        ),
        case(
            "amp-record-v1.encode.file-chunk",
            "encode",
            note="FILE_CHUNK (0x03): transfer_id, offset, payload_len+payload (18B sabit önek).",
            spec_ref="application-message-protocol.md §3 0x03 FILE_CHUNK",
            input_={
                "transfer_id": 42,
                "offset": 0,
                "payload_hex": "de" * 16,
            },
            expected_hex=hx(record(FILE_CHUNK, file_chunk_body(42, 0, bytes.fromhex("de" * 16)))),
        ),
        case(
            "amp-record-v1.encode.file-end",
            "encode",
            note="FILE_END (0x04): yalnız transfer_id.",
            spec_ref="application-message-protocol.md §3 0x04 FILE_END",
            input_={"transfer_id": 42},
            expected_hex=hx(record(FILE_END, file_end_body(42))),
        ),
        case(
            "amp-record-v1.encode.file-abort",
            "encode",
            note="FILE_ABORT (0x05): transfer_id, reason (0x02 = hash_mismatch).",
            spec_ref="application-message-protocol.md §3 0x05 FILE_ABORT",
            input_={"transfer_id": 42, "reason": 0x02},
            expected_hex=hx(record(FILE_ABORT, file_abort_body(42, 0x02))),
        ),
        case(
            "amp-record-v1.encode.ptt-open",
            "encode",
            note="PTT_OPEN (0x06): stream_id, codec (0x01=opus), bundle_ms.",
            spec_ref="application-message-protocol.md §3 0x06 PTT_OPEN",
            input_={"stream_id": 7, "codec": 0x01, "bundle_ms": 20},
            expected_hex=hx(record(PTT_OPEN, ptt_open_body(7, 0x01, 20))),
        ),
        case(
            "amp-record-v1.encode.ptt-close",
            "encode",
            note="PTT_CLOSE (0x07): yalnız stream_id.",
            spec_ref="application-message-protocol.md §3 0x07 PTT_CLOSE",
            input_={"stream_id": 7},
            expected_hex=hx(record(PTT_CLOSE, ptt_close_body(7))),
        ),
        case(
            "amp-record-v1.encode.receipt-delivered",
            "encode",
            note="RECEIPT (0x08): ref_msg_id, kind (0x01=delivered, M3a kapsamı — AMP A4).",
            spec_ref="application-message-protocol.md §3 0x08 RECEIPT",
            input_={"ref_msg_id": 1, "kind": 0x01},
            expected_hex=hx(record(RECEIPT, receipt_body(1, 0x01))),
        ),
        case(
            "amp-record-v1.encode.text-compressed",
            "encode",
            flags=["compressed"],
            note="TEXT, flags.bit0=1: body = compression_id(1)|uncompressed_len(4be)|zstd(payload). "
            "Gerçek zstd (Python 3.14 compression.zstd) ile üretildi.",
            spec_ref="application-message-protocol.md §5",
            input_={
                "msg_id": 2,
                "sent_at_ms": 1_758_000_000_000,
                "text": "tekrar eden metin " * 8,
            },
            expected_hex=hx(
                record(
                    TEXT,
                    compressed_body(text_body(2, 1_758_000_000_000, "tekrar eden metin " * 8)),
                    flags=COMPRESS_FLAG_BIT0,
                )
            ),
        ),
    ]
    return {
        "schema_version": 2,
        "suite": "amp-record-v1",
        "spec_version": "application-message-protocol@v1-B1.2+2026-09-23",
        "generator": "ref/amp/encode.py (stdlib-only, Python 3.14, production Rust ile sıfır paylaşım)",
        "description": "AMP §2/§3/§5 AppMessageRecord kodlaması — her msg_type için 1 pozitif vektör "
        "+ bir sıkıştırılmış TEXT örneği. B1.2 (WS-B/Hayri); tüketici B2.1'deki gerçek Rust codec'i.",
        "notes": {
            "compressed": "flags.bit0=1; body = compression_id(1)|uncompressed_len(4be)|zstd payload (§5).",
        },
        "case_count": len(cases),
        "cases": cases,
    }


def build_negative() -> dict:
    # Sıkıştırma-negatif vakalar için: uncompressed_len > MAX_DECOMPRESSED_MESSAGE ve
    # expansion-ratio kontrolleri gerçek zstd'ye HİÇ ihtiyaç duymaz (decoder decompress'e
    # başlamadan, yalnız beyan edilen uzunluk alanlarına bakarak reddeder — §5 kural 1/4).
    # truncated-zstd-stream ve invalid-zstd-frame vakaları GERÇEK zstd (compression.zstd)
    # kullanır: biri gerçek bir frame'in kesilmiş hali, diğeri hiç zstd olmayan rastgele bayt.

    reserved_type = record(0x00, b"")

    reserved_flags = record(TEXT, text_body(1, 0, ""), flags=0x02)

    # body_len (dış) = gerçek gönderilen bayt sayısı; TEXT'in İÇ text_len alanı, kalan body
    # baytlarından daha büyük bir değer beyan ediyor → §2 A1: "toplam ≤ body_len — aşarsa DROP".
    inner_prefix = struct.pack(">QQH", 1, 0, 100)  # text_len=100 ama arkasında 3 bayt var
    inner_mismatch_body = inner_prefix + b"\x61\x62\x63"
    inner_mismatch = record(TEXT, inner_mismatch_body)

    over_max_uncompressed = struct.pack(">BI", ZSTD_COMPRESSION_ID, MAX_DECOMPRESSED_MESSAGE + 1) + b"\x00" * 8
    over_max = record(TEXT, over_max_uncompressed, flags=COMPRESS_FLAG_BIT0)

    # ratio = uncompressed_len / body_len; body_len = 5 (önek) + 5 (payload) = 10; 1000/10=100 > 64.
    expansion_payload = struct.pack(">BI", ZSTD_COMPRESSION_ID, 1000) + b"\x01\x02\x03\x04\x05"
    expansion = record(TEXT, expansion_payload, flags=COMPRESS_FLAG_BIT0)

    genuine_uncompressed = (b"gercek zstd frame - kesilecek icerik " * 6)
    genuine_compressed = zstd.compress(genuine_uncompressed)
    truncated_payload = struct.pack(">BI", ZSTD_COMPRESSION_ID, len(genuine_uncompressed)) + genuine_compressed[:-4]
    truncated = record(TEXT, truncated_payload, flags=COMPRESS_FLAG_BIT0)

    garbage_payload = struct.pack(">BI", ZSTD_COMPRESSION_ID, 16) + b"\xff" * 16
    invalid_frame = record(TEXT, garbage_payload, flags=COMPRESS_FLAG_BIT0)

    unknown_compression_id = struct.pack(">BI", 0x00, 4) + b"\x00\x00\x00\x00"
    unknown_compression = record(TEXT, unknown_compression_id, flags=COMPRESS_FLAG_BIT0)

    unknown_version = record(TEXT, text_body(1, 0, "hi"), msg_version=0x02)

    cases = [
        case(
            "amp-record-reject-v1.decode.reserved-msg-type",
            "decode",
            note="msg_type=0x00 rezerve (§2: 0x00 ve 0xF0..0xFF → REJECT).",
            spec_ref="application-message-protocol.md §2",
            input_hex=hx(reserved_type),
            expected={"result": "reject", "reason": "reserved-msg-type"},
        ),
        case(
            "amp-record-reject-v1.decode.reserved-flags-nonzero",
            "decode",
            note="flags bit1 set; bit1..7 reserved-zero olmalı (§2).",
            spec_ref="application-message-protocol.md §2",
            input_hex=hx(reserved_flags),
            expected={"result": "reject", "reason": "reserved-flags-nonzero"},
        ),
        case(
            "amp-record-reject-v1.decode.inner-field-exceeds-body-len",
            "decode",
            note="TEXT.text_len=100 ama body_len içinde yalnız 3 bayt kaldı — A1: toplam ≤ body_len, aşarsa DROP.",
            spec_ref="application-message-protocol.md §2 (A1)",
            input_hex=hx(inner_mismatch),
            expected={"result": "drop", "reason": "inner-field-exceeds-body-len"},
        ),
        case(
            "amp-record-reject-v1.decode.uncompressed-len-exceeds-max",
            "decode",
            flags=["compressed", "compression-length-only"],
            note=f"uncompressed_len={MAX_DECOMPRESSED_MESSAGE + 1} > MAX_DECOMPRESSED_MESSAGE={MAX_DECOMPRESSED_MESSAGE} "
            "(test parametresi: max_plaintext_unit_bytes=1024) — decompress'e HİÇ başlanmaz (§5 kural 1, bomb koruması).",
            spec_ref="application-message-protocol.md §5 kural 1",
            input_hex=hx(over_max),
            expected={"result": "drop", "reason": "uncompressed-len-exceeds-max-decompressed"},
        ),
        case(
            "amp-record-reject-v1.decode.expansion-ratio-exceeded",
            "decode",
            flags=["compressed", "compression-length-only"],
            note="uncompressed_len/body_len = 1000/10 = 100 > MAX_EXPANSION_RATIO=64 (§5 kural 4, ek bomb sınırı).",
            spec_ref="application-message-protocol.md §5 kural 4",
            input_hex=hx(expansion),
            expected={"result": "drop", "reason": "expansion-ratio-exceeded"},
        ),
        case(
            "amp-record-reject-v1.decode.truncated-zstd-stream",
            "decode",
            flags=["compressed"],
            note="Gerçek zstd frame'in son 4 baytı kesilmiş — decompress denenir, tamamlanamaz (§5 kural 3).",
            spec_ref="application-message-protocol.md §5 kural 3",
            input_hex=hx(truncated),
            expected={"result": "drop", "reason": "invalid-zstd-frame"},
        ),
        case(
            "amp-record-reject-v1.decode.invalid-zstd-frame",
            "decode",
            flags=["compressed"],
            note="0xFF dolgu — hiçbir zaman geçerli bir zstd frame'i olmadı (§5 kural 3).",
            spec_ref="application-message-protocol.md §5 kural 3",
            input_hex=hx(invalid_frame),
            expected={"result": "drop", "reason": "invalid-zstd-frame"},
        ),
        case(
            "amp-record-reject-v1.decode.unknown-compression-id",
            "decode",
            flags=["compressed"],
            note="compression_id=0x00 tanımsız (0x01=zstd tek algoritma; 0x00 yasak — bit0=0 kullanılır).",
            spec_ref="application-message-protocol.md §5",
            input_hex=hx(unknown_compression),
            expected={"result": "reject", "reason": "unknown-compression-id"},
        ),
        case(
            "amp-record-reject-v1.decode.unknown-msg-version",
            "decode",
            note="msg_version=0x02 bilinmeyen → DROP, REJECT değil (§6: ileri uyum).",
            spec_ref="application-message-protocol.md §6",
            input_hex=hx(unknown_version),
            expected={"result": "drop", "reason": "unknown-msg-version"},
        ),
    ]
    return {
        "schema_version": 2,
        "suite": "amp-record-reject-v1",
        "spec_version": "application-message-protocol@v1-B1.2+2026-09-23",
        "generator": "ref/amp/encode.py (stdlib-only, Python 3.14, production Rust ile sıfır paylaşım)",
        "description": "AMP §2/§5/§6 negatif corpus: reserved type/flag, iç alan/body_len uyumsuzluğu, "
        "sıkıştırma bomb korumaları (uncompressed_len aşımı, expansion oranı), gerçek zstd ile "
        "kesilmiş/geçersiz frame, bilinmeyen compression_id/msg_version.",
        "notes": {
            "compressed": "flags.bit0=1 gövde (§5).",
            "compression-length-only": "Decoder yalnız beyan edilen uzunluk alanlarına bakarak "
            "reddeder; zstd decompress'e HİÇ başlanmaz — gerçek bir zstd frame'i olması gerekmez.",
        },
        "case_count": len(cases),
        "cases": cases,
    }


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    vectors_dir = root / "vectors"

    positive = build_positive()
    negative = build_negative()

    (vectors_dir / "amp-record-v1.json").write_text(
        json.dumps(positive, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    (vectors_dir / "amp-record-reject-v1.json").write_text(
        json.dumps(negative, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"wrote {positive['case_count']} positive + {negative['case_count']} negative cases")


if __name__ == "__main__":
    main()
