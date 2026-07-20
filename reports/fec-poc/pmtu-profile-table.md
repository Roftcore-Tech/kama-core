# FEC-POC — PMTU / Profile Byte-Tablosu (K-FEC-MTU-1)

> Byte-otoritesi: kama-noise-wire-tdd §2.6 (FINAL-BYTE-LAYOUT, KİLİTLİ) + schema-crate sabitleri. Bu tablo
> **gerçek-byte** (deterministik hesaplama, tahmin-DEĞİL); §11 ortak-PMTU-sözleşmesi. N4/N7-raporlarıyla ortak.

## 1. Sabit overhead'ler (§2.6)

```
EXTENDED-OUTER (fixed)        = h_ext 4 + wire_format 1 + wire_flags 1 + pn_trunc 4 + tag 16   = 26 B
FEC-SHARD ENCRYPTED-INNER hdr = inner_version 1 + inner_kind 1 + flags 1 + profile_id 1
                                + block_id 4 + esi 2 + k_actual 1                              = 11 B
CANONICAL-SOURCE-SYMBOL-PREFIX (E-içinde, RS-kodlu)                                            = 11 B
                                (message_id 4 + chunk_offset 4 + actual_payload_len 2 + source_flags 1)
```

`symbol_bytes == profile.E` (MADDE-6 symbol_len-KİLİDİ — ayrı-uzunluk-alanı YOK). AEAD: ciphertext-len == plaintext-len,
tag=16B outer-26'nın-içinde. **sealed_len(EXTENDED-FEC) = 26 + (11 + E)** = `37 + E`.

## 2. Profile-başına sealed-wire byte (E=64 tüm-Alpha-profilleri)

| profile | K_shape | R_code_max | E | sealed FEC-shard (UDP-payload) | app-source-payload = E−11 |
|---|---|---|---|---|---|
| 0x01 | 2  | 2 | 64 | **101 B** | 53 B |
| 0x02 | 4  | 4 | 64 | **101 B** | 53 B |
| 0x03 | 8  | 4 | 64 | **101 B** | 53 B |
| 0x04 | 16 | 8 | 64 | **101 B** | 53 B |

sealed-shard profile-bağımsız (E sabit=64) → her-shard 101 B on-wire (UDP-payload). Kaynak-shard'ın 53 B'i
app-payload, kalan 11 B RS-kodlu-prefix (metadata-recoverability, §6.5).

## 3. Datagram-toplamı vs PMTU-tabanları

| katman | L3+L4 overhead | datagram (E=64) | 1280 (IPv6-min) headroom | 1500 (Ethernet) headroom |
|---|---|---|---|---|
| IPv4 + UDP | 20 + 8 = 28 | **129 B** | — | 1371 B |
| IPv6 + UDP | 40 + 8 = 48 | **149 B** | 1131 B | 1351 B |
| IPv6 + WireGuard-tünel (~60) + UDP | ~108 | ~209 B | 1071 B | 1291 B |

**HÜKÜM (K-FEC-MTU-1):** dört-profil de (E=64) IPv4/IPv6/tünel datagram'ında **PMTU-tabanının ÇOK ALTINDA** (en-kötü
tünel ~209 B « 1280). IP-fragmentation'a-güven YOK; shard-datagram tek-parça sığar. `E ≠ 1200` (§11): büyük-E
profilleri (E→~1100) D1'de eklenirse bu tablo yeniden-ölçülür (mid-block-PMTU-shrink §11: block-boyu-immutable).

## 4. Açık uç

- **Control-frame sealed_len üst-sınırı:** §2.57 count'lar (`esi_range_count`, ACK-range-sayısı) profile-bounded/[CAL],
  sayısal-tanımsız → max-control datagram üst-sınırı bugün-hesaplanamaz. Bkz. **DR-1** (D1-CAL gap).
