# N7 — WIRE-SIZING / PMTU UYUM-RAPORU (P0-3) [RAPOR-ONLY, KOD-YAZILMADI]

> **Statü:** RAPOR-ONLY (kod-envanteri + spec-türevli PMTU-bütçe-tablosu; kod YAZILMADI). noise-wire-D0-A
> N7 maddesi (§12, P0-3 YENİ).
> **Kapsam (§12-N7):** "exact-outer-bytes · packet-class-başına-exact-inner · AEAD-tag · padding ·
> IPv4/IPv6/tunnel · sealed_len()-sözleşme-taslağı · profile-başına-max-E · sub-kbps-serialization-tablosu."
> **Otorite:** `specs/kama-noise-wire-tdd-v2.0.md` §2.6/§2.56/§2.57/§0.5-P0-3 + `specs/fec-core-tdd-v3.0.md` §11 ·
> LOCKED byte-facts `crates/kama-transport-schema/src/lib.rs:37,41,45` · profile-katalog
> `crates/kama-fec-codec/src/profile.rs:56-89`.
> **HÜKÜM (özet):** exact-byte-tabloları **spec-sabitlerinden HESAPLANABİLİR** ve aşağıda tam-doldurulmuştur.
> Ancak bu byte'ları **ÜRETEN kod (seal / `sealed_len()`) YOK** (crypto 3-satır iskelet, noise-wire crate yok,
> `DEFAULT_MTU=1200` statik + admission'a bağlı-değil) → tüm sayılar **D1-EKSİĞİ** olarak işaretli, PMTU-bütçesi
> spec-sabitleriyle yine de dolduruldu. E=64-katalog TÜM tipik PMTU'larda geniş-headroom'la sığar (sealed
> FEC-shard = **101B**). Bir **DR-ADAYI**: control-frame count-bound'ları ([CAL]/profile-bounded) sayısal-sabit
> DEĞİL → max-control sealed_len exact-bounded edilemez.

---

## (1) SPEC NE DİYOR (§2.6 · §2.56 · §2.57 · §0.5 · fec-core §11)

### fec-core §11 — max_udp_payload_bytes + sealed_len [İKİ-BELGE-ORTAK]
```
max_udp_payload_bytes = effective_ip_path_mtu − ip_header_bytes − udp_header_bytes − tunnel_overhead_bytes
WIRE-ADMISSION:  sealed_len(packet) ≤ max_udp_payload_bytes
sealed_len (EXTENDED)            = 26 + encrypted_inner_len      # h_ext4+wf1+flags1+pn4+tag16 (§2.6)
sealed_len (legacy-SESSION_DATA) = 22 + plaintext_payload_len    # AEAD-expansion=yalnız-tag varsayımı
```
Kurallar: **E-hiçbir-zaman-doğrudan-1200-DEĞİL** · IPv4/IPv6/tunnel-AYRI-hesap · IP-fragmentation'a-güvenilmez ·
profile-shard-size: even-byte + mümkünse-64-multiple (reed-solomon-simd) + sub-kbps + deadline · MID-BLOCK-PMTU-
SHRINK: Profile.E block-boyunca-IMMUTABLE (küçülüp-sığmazsa block-abort → küçük-profile-YENİ-block). §0.5-P0-3
(K-WIRE-SIZE-1): `seal(p).len() == sealed_len(p)` — üst-sınır DEĞİL, **KESİN**.

### §2.6 — exact widths (schema-crate LOCKED)
- **EXTENDED-outer = 26B:** h_ext 4 + wire_format 1 + wire_flags 1 + pn_trunc 4 + tag 16 (`lib.rs:41`).
- **Legacy-SESSION_DATA-outer = 22B:** hub_handle 4 + seq 2 + tag 16 (`lib.rs:45`).
- **FEC-inner-header = 11B:** inner_version 1 + inner_kind 1 + flags 1 + profile_id 1 + block_id 4 + esi 2 +
  k_actual 1. Sonra `symbol_bytes[profile.E]` — **symbol_len wire-alanı YOK** (MADDE-6; her shard TAM-E).
- **Source-symbol-prefix = 11B (E-içinde, RS-kodlu):** message_id 4 + chunk_offset 4 + actual_payload_len 2 +
  source_flags 1 = `SOURCE_PREFIX_LEN` (`lib.rs:37`). Usable-payload/source = E − 11.

### §2.57 — control-body widths (ortak: inner_version 1 + inner_kind 1 + control_id u32 4 = 6B, sonra body)
`ACK(0x10)`: acked_dh_epoch 1 + ack_range_count 2 + [pn_start u64, pn_end u64]×n (16n) · `SACK(0x11)`:
acked_dh_epoch 1 + base_pn 8 + gap_ack_block_count 2 + [gap u16, ack_len u16]×n (4n) · `RECOVERED(0x12)`:
block_id 4 + esi_range_count 2 + [esi_start u16, esi_end u16]×n (4n) · `BLOCK_COMPLETE(0x13)`: block_id 4 +
final_source_count 2 · `PARAM_SET(0x14)`: param_hash 32 + profile_id 1 + code_suite_id 1 · `PARAM_ACK(0x15)`:
param_hash 32 · `BLOCK_ABORT(0x16)`: block_id 4 + reason_code 1 · `SESSION_CLOSE(0x17)`: reason_code 1 ·
`SESSION_RENEW_REQUIRED(0x18)`: reason_code 1 + renew_hint 1. **count'lar profile-bounded** (max_ack_ranges /
max_sack_blocks / max_recovered_ranges — §2.57 "her-count-profile'da-BOUNDED").

### Profile-katalog (`kama-fec-codec/src/profile.rs:56-89`) — TÜMÜ E=64
`0x01 K2R2E64` · `0x02 K4R4E64` · `0x03 K8R4E64` · `0x04 K16R8E64` (code_suite_id=0x01 RS_SIMD_V1). E even +
64-multiple (`profile.rs` validate :35-37). B4: 1 ≤ K_shape ≤ 255, E ≥ 11.

---

## (2) MEVCUT KOD NE YAPIYOR (dosya:satır)

- **PMTU/max_udp_payload admission — YOK.** `grep pmtu|max_udp|mtu` tüm-kod: yalnız `rate.rs:18`
  `pub const DEFAULT_MTU: u64 = 1200;` (BBR cwnd hesabı için statik; **sealed_len/admission'a BAĞLI DEĞİL**;
  §11-max_udp_payload_bytes türevi HİÇ yok). IPv4/IPv6/tunnel-header ayrımı hiçbir yerde.
- **`sealed_len()` / `KamaNoiseWireSizer` — YOK.** `grep sealed_len` yalnız schema doc-comment (`lib.rs:40,43`)
  + golden-test-yorumu (`tests/golden_v0.rs:183`); fonksiyon/trait yok. noise-wire crate yok.
- **Exact-outer/inner seal — YOK.** crypto 3-satır iskelet (`crypto/src/lib.rs:1-3`); gerçek 16B-AEAD-tag yok
  (fec.rs=wire-size-placeholder `SHARD_TAG_LEN=16` `:24`; bench=zero-fill-sim `TAG_STUB=16` `kama.rs:38`).
- **Byte-sabitleri (schema, doğrulanmış):** `EXTENDED_OUTER_OVERHEAD=26` · `LEGACY_SESSION_DATA_OVERHEAD=22` ·
  `SOURCE_PREFIX_LEN=11` (`lib.rs:37,41,45`; assert `golden_v0.rs:185-189`). **symbol==profile.E** kilidi
  `prefix::SourcePrefix::serialize_symbol` (`prefix.rs`) + `SymbolLenMismatch` (`lib.rs:86`).
- **Profile-katalog (doğrulanmış):** `PROFILES` 4-giriş, E=64 (`profile.rs:56-89`); `by_id` allowlist (`:91-98`).
- **padding:** yalnız FEC canonical_zero_pad (E-içi, `prefix.rs` `out[..]=0`); wire-seviyesi obfuscation-padding/
  size-class YOK. Bench `TAG_STUB` `b.resize(...,0)` (`kama.rs:59`) = sabit-16 zero-fill (tag-sim, padding değil).

→ N7'nin istediği hiçbir byte-üreten yol production-formda mevcut değil; aşağıdaki tablolar **spec-sabitlerinden**
hesaplanmıştır (§11-yetki: "PMTU/profile-tablosu gerçek-byte-ölçümüyle" D1-IMPLEMENTATION-GO şartı).

---

## (3) HÜKÜM: UYUMLU / UYUMSUZ / YOK

**HÜKÜM: YOK (byte-üreten seal/sealed_len/PMTU-admission katmanı) — tablolar spec-sabitlerinden dolduruldu.**
Byte-otoritesi (schema-sabitleri 26/22/11 + E=64-katalog) UYUMLU ve tutarlı; ancak bunları wire'a döken kod
(seal, `sealed_len()`, max_udp_payload admission, IPv4/IPv6/tunnel-hesabı) hiçbir yerde yok → **N7-çıktısı D1'de
üretilecek sözleşmenin taslağıdır**, mevcut-koddan ölçülen değer değil.

### (a) Exact-outer-bytes / paket-class
| Class | Outer alanları | Outer-overhead |
|---|---|---|
| EXTENDED (FEC + secure-control) | h_ext 4 · wire_format 1 · wire_flags 1 · pn_trunc 4 · **tag 16** | **26B** |
| Legacy SESSION_DATA (mühürlü) | hub_handle 4 · seq 2 · **tag 16** | **22B** |

### (b) Exact-inner-bytes / paket-class  (E=64 katalog için sayısal)
| Inner-class | Inner-header/gövde | encrypted_inner (E=64) | Not |
|---|---|---|---|
| FEC-**source** | 11B-inner-hdr + symbol[E]; symbol-içi ilk 11B = source-prefix | 11 + 64 = **75B** | usable-app/source = E−11 = **53B** |
| FEC-**repair** | 11B-inner-hdr + symbol[E] (saf RS-çıktı) | 11 + 64 = **75B** | usable-app = 0 (kod-symbol) |
| Control-ortak | inner_version 1 + inner_kind 1 + control_id 4 | 6B + body | §2.57 |

Control inner+sealed (n = range/block sayısı):
| Control | body | inner (6+body) | sealed (26+inner) |
|---|---|---|---|
| ACK 0x10 | 1+2+16n | 9+16n | **35+16n** |
| SACK 0x11 | 1+8+2+4n | 17+4n | **43+4n** |
| RECOVERED 0x12 | 4+2+4n | 12+4n | **38+4n** |
| BLOCK_COMPLETE 0x13 | 4+2 | 12 | **38** |
| PARAM_SET 0x14 | 32+1+1 | 40 | **66** |
| PARAM_ACK 0x15 | 32 | 38 | **64** |
| BLOCK_ABORT 0x16 | 4+1 | 11 | **37** |
| SESSION_CLOSE 0x17 | 1 | 7 | **33** |
| SESSION_RENEW_REQUIRED 0x18 | 1+1 | 8 | **34** |

**AEAD-tag:** her §2.6-sealed-paket (EXTENDED + legacy) = **16B** (outer 26/22'ye dahil). Gerçek Noise-tag D1
(şu an placeholder/zero-fill sim, §2). **padding:** Alpha wire-seviyesi obfuscation-padding YOK; yalnız FEC-symbol-
içi canonical_zero_pad (E − 11 − actual_payload_len). Cryptex outer source/repair AYNI-size-class önerir (§2 —
timing/hacim/boyut GARANTİ-ETMEZ; cover-traffic kapsam-dışı).

### (c) sealed_len() SÖZLEŞME-TASLAĞI (D1 — §0.5/§11 restatement; spec-formülleri)
```rust
// noise-wire crate (YENİ, D1). NORMATİF: seal(p).len() == sealed_len(p) — KESİN (K-WIRE-SIZE-1).
pub trait KamaNoiseWireSizer { fn sealed_len(&self, p: &UnsealedTransportPacket) -> Result<usize, WireSizeError>; }

sealed_len(EXTENDED) = 26 + encrypted_inner_len
  encrypted_inner_len =
    FEC source/repair : 11 (inner-hdr) + E                       // E = profile.e (katalog: 64)
    Control(inner_kind): 6 (inner_version+inner_kind+control_id) + body_len(inner_kind, counts)  // (b)-tablosu
sealed_len(legacy SESSION_DATA) = 22 + plaintext_payload_len     // AEAD-expansion = tag-only varsayımı (§11)
```
**Varsayım (D1-teyit):** AEAD-expansion = yalnız-tag (16B); nonce/PN wire'da AAD-outer'da (ciphertext-length
UDP-length'ten implicit, §2.8) → length-expansion YOK. Gerçek AEAD gelince EXACT-teyit (fec-core §11:
"farklıysa-EXACT-formül N7-raporunda güncellenir").

### (d) Profile-başına max-E tablosu (4-katalog; E-sabit-64, sealed FEC-shard = 26+11+E)
| profile_id | shape | E | sealed-shard (26+11+E) | usable-app/source (E−11) | Hüküm |
|---|---|---|---|---|---|
| 0x01 | K2 R2 | 64 | **101B** | 53B | UYUMLU (katalog) |
| 0x02 | K4 R4 | 64 | **101B** | 53B | UYUMLU |
| 0x03 | K8 R4 | 64 | **101B** | 53B | UYUMLU |
| 0x04 | K16 R8 | 64 | **101B** | 53B | UYUMLU |

**PMTU'nun izin verdiği max-E** (sabit-katalog-64 DEĞİL, teorik-tavan): sealed = 37 + E ≤ max_udp_payload →
`E_max = max_udp_payload − 37`, sonra even + 64-multiple'a yuvarla (aşağı):
| Path | max_udp_payload | E_max (=−37) | 64-multiple-floor |
|---|---|---|---|
| IPv4@1280 | 1252 | 1215 | 1152 |
| IPv6@1280 | 1232 | 1195 | 1152 |
| IPv4@1500 | 1472 | 1435 | 1408 |
| IPv6@1500 | 1452 | 1415 | 1408 |
| tunnel(+60)@1500-IPv4 | 1412 | 1375 | 1344 |
| tunnel(+60)@1280-IPv4 | 1192 | 1155 | 1152 |
→ Katalog-E=64 (sealed 101B) TÜM satırlarda geniş-headroom'la sığar; max-E katalog-tarafından (64) sınırlı,
PMTU-tarafından değil (bu MTU'larda). MID-BLOCK-PMTU-SHRINK ancak PMTU 101B'nin altına düşerse tetiklenir.

### (e) IPv4 / IPv6 / tunnel bütçe-satırları (sealed FEC-shard @E=64 = 101B'ye karşı)
| MTU | Path | ip_hdr | udp_hdr | tunnel | max_udp_payload | headroom @101B | Hüküm |
|---|---|---|---|---|---|---|---|
| 1280 | IPv4 | 20 | 8 | 0 | **1252** | 1151 | UYUMLU (sığar) |
| 1280 | IPv6 | 40 | 8 | 0 | **1232** | 1131 | UYUMLU |
| 1500 | IPv4 | 20 | 8 | 0 | **1472** | 1371 | UYUMLU |
| 1500 | IPv6 | 40 | 8 | 0 | **1452** | 1351 | UYUMLU |
| 1500 | IPv4+tunnel | 20 | 8 | 60 | **1412** | 1311 | UYUMLU |
| 1280 | IPv4+tunnel | 20 | 8 | 60 | **1192** | 1091 | UYUMLU |
(tunnel=60B illüstratif; gerçek tunnel_overhead operatör/link-bağımlı, §11-ayrı-hesap. E≠doğrudan-1200 kuralı
korunur: E=64 türetilmiş, 1200/1280'e sabitlenmemiş.)

### (f) sub-kbps serialization-tablosu (bytes-on-wire / shard, her profile — TÜMÜ E=64)
| profile | per-shard wire | app/source (53B) | non-app/wire | non-app oranı | full-block wire (K_shape src + R_code_max rep) |
|---|---|---|---|---|---|
| 0x01 K2R2 | 101B | 53B | 48B | 47.5% | (2+2)×101 = **404B** (app 2×53=106B) |
| 0x02 K4R4 | 101B | 53B | 48B | 47.5% | (4+4)×101 = **808B** (app 4×53=212B) |
| 0x03 K8R4 | 101B | 53B | 48B | 47.5% | (8+4)×101 = **1212B** (app 8×53=424B) |
| 0x04 K16R8 | 101B | 53B | 48B | 47.5% | (16+8)×101 = **2424B** (app 16×53=848B) |
non-app 48B/shard = 26 (outer) + 11 (inner-hdr) + 11 (source-prefix). Sub-kbps'te bu **sabit 48B/shard** amortize-
zorluğudur; §2 INNER-KÜÇÜLTME (session/epoch outer'dan · K/R/E profile'dan) bunu Beta'da azaltabilir (Alpha sabit-layout).

---

## (4) KOD-DÜZELTME-PLANI (D1) — spec DEĞİŞMEZ

- **E1 — noise-wire crate + `KamaNoiseWireSizer::sealed_len()`:** (c)-taslağını uygula; `seal(p).len()==sealed_len(p)`
  KESİN-assert (K-WIRE-SIZE-1). Pacer-akışı: logical → sealed_len → CC-token-RESERVE → seal → reserve==len-doğrula (§0.5).
- **E2 — max_udp_payload_bytes admission:** `effective_ip_path_mtu − ip_hdr − udp_hdr − tunnel_overhead`; IPv4(20)/
  IPv6(40)/tunnel ayrı-hesap; `sealed_len ≤ max_udp_payload` WIRE-ADMISSION. `rate.rs:18 DEFAULT_MTU=1200` yalnız-
  cwnd; sealed-admission'a **bağlanmaz** (E≠doğrudan-1200 kuralı, §11).
- **E3 — MID-BLOCK-PMTU-SHRINK:** Profile.E block-boyunca-IMMUTABLE; PMTU 101B-altına-düşerse yeni-shard-YOK →
  block-abort/expire → küçük-profile-YENİ-block (§11). ICMP-black-hole-fallback + padding-size-class-etkisi D1-ölç.
- **E4 — profile-shard-size doğrulama:** E even + 64-multiple + `E ≥ 11` (prefix-sığar) — `profile.rs` validate
  ZATEN VAR (`:35-37`); noise-wire sealed_len bunu tüketir. Katalog-E=64 tüm-PMTU'da sığar (headroom tablosu).
- **E5 — control-count-bound sabitleri:** max_ack_ranges / max_sack_blocks / max_recovered_ranges profile-başına
  KİLİTLE (aşağı DR-ADAYI) → max-control sealed_len bounded olur.

---

## (5) D1-EKSİKLERİ

- **[D1-1] Seal/sealed_len/PMTU-admission katmanı tümüyle YOK:** (a)-(f) tabloları spec-sabitlerinden; gerçek-byte-
  ölçümü D1-IMPLEMENTATION-GO şartı ("PMTU/profile-tablosu gerçek-byte-ölçümüyle", STATÜ-MERDİVENİ). crypto 3-satır.
- **[D1-2] IPv4/IPv6/tunnel-hesabı YOK:** kodda hiç ip/udp/tunnel-header ayrımı yok; yalnız statik `DEFAULT_MTU=1200`
  (admission'a bağlı değil). (e)-satırları D1'de gerçek-path-MTU-discovery/config'e bağlanır.
- **[D1-3] Gerçek AEAD-expansion teyidi:** (c)-taslağı "tag-only (16B)" varsayar; crypto-katmanı gelince EXACT-
  doğrula (nonce/PN wire'da AAD-outer'da, ciphertext-length UDP-length'ten implicit §2.8 → length-expansion-YOK beklenir).
- **[D1-4] padding / size-class:** Alpha wire-padding yok; Cryptex source/repair-AYNI-size-class (§2) + ICMP-black-
  hole padding-etkisi (§11 N7-notu) D1-tasarımı. Bench `TAG_STUB` zero-fill (`kama.rs:59`) padding değil, tag-sim.
- **[DR-ADAYI] Control-count-bound'ları sayısal-sabit DEĞİL — spec'te tanımsız (profile-bounded / [CAL]):** §2.57
  "her-count-profile'da-BOUNDED" der ama max_ack_ranges/max_sack_blocks/max_recovered_ranges için **sayısal-değer
  spec'te-yok** → (b)-tablosundaki `35+16n / 43+4n / 38+4n` için `n_max` bilinmediğinden **max-control sealed_len
  exact-üst-sınırı hesaplanamaz** (PMTU-admission için gerekli). `ack.rs:38 MAX_ACK_RANGES=16 [CAL]` var ama bu
  legacy-cleartext-ACK'in sabiti, §2.57-secure-ACK'in değil. **KURAL-2: spec-değişikliği ÖNERİLMEZ** — D1-profile-
  sabitleri kilitlenmeli (bir spec-hatası değil, kalibrasyon-boşluğu; D1-CAL kapatır).

**Cross-ref → N4:** bu rapor N4'ün symbol-dışı-overhead sabitlerini (outer 26 / legacy 22 / inner-hdr 11 / prefix 11
/ tag 16) girdi-alır ve E=64-katalog + IPv4/IPv6/tunnel-MTU ile per-class exact-byte + max-E + sub-kbps tablosuna
dönüştürür. N4-bulgusu (fec.rs 26B tesadüfi, 11B-inner-eksik, gerçek-FEC-overhead 37B) burada sealed-shard 101B =
26+11+64 olarak yapısal-doğrulanır.
