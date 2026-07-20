# FEC-POC — kama-fec-codec (İŞ-A çıktı-paketi, D1-ENTRY-GO adayı)

> STATÜ: FULL-DESIGN-GO → **D1-ENTRY-GO şartlarının FEC-tarafı**. Kripto-agnostik codec + schema. İddia-disiplini:
> bu-belge "production-ready/hostile-ready/anti-jamming/blackout-resistant" İDDİA ETMEZ — yalnız ölçülen-POC.

## 1. Ne inşa edildi (crate)

| crate | içerik | test |
|---|---|---|
| `kama-transport-schema` | widths + inner_kind-registry(§2.56) + source-prefix(§2.6) + PARAM-canonical(§8) + control-DTO(§2.57) | golden-v0 **9/9** |
| `kama-fec-codec` | FecCodec trait + `RsSimdCodec` (reed-solomon-simd 3.1.0) + 4-profil + Model-A + §6.5 metadata + golden | **20/20** |

`kama-fec-codec` toplam **20/20** (9 roundtrip + 8 known-zero/metadata + 3 golden). Tüm-kapılar yeşil:
`cargo fmt --check` · `clippy -D warnings` · `test`. SoC: schema/fec-codec Noise/AEAD/nonce/key TİPİ TAŞIMAZ (K-FEC-SOC-1/2).

## 2. Kapsanan normatif POC-seti

- **RS-çekirdek (§5.1):** encode/decode roundtrip her-profil · source/repair/mixed-erasure · duplicate-ESI · wrong-count
  fail-closed · deterministik-repair.
- **Model-A known-zero-injection (§6, NORMATİF):** K_shape=8×{K_actual=1,7} · yalnız-repair · final-block-padding ·
  decode-koşulu `received_source+received_repair+known_zero ≥ K_shape` · fail-closed {KnownZeroTooLarge, SourceInZeroRegion, Insufficient}.
- **Metadata-recoverability (§6.5, NORMATİF):** kayıp-source'un message_id/chunk_offset/actual_payload_len/final-flag
  RS-kodlu-prefix'ten TAM geri gelir; final-flag'li mesaj-sonu tek-pakete-bağlı-DEĞİL.
- **Golden-vector (K-FEC-WIRE-1):** profil 0x01 repair-byte PIN'lendi (RS-lib drift-gate) + reconstructed-METADATA golden.
- **On-demand tek-ESI recompute (§6):** `recompute_repair(esi) == encode()[esi] == golden`.

## 3. İlk-ölçüm (teşhis-seviyesi — indicative, netem/criterion PR-gate DEĞİL)

`cargo run -p kama-fec-codec --example measure --release` (20k iter/profil, warmup'lı, tek-koşu):

| profile | encode µs/blok | encode MB/s | reconstruct µs/blok | recompute/encode |
|---|---|---|---|---|
| 0x01 K2R2E64  | 0.11 | ~1168 | 146 | 0.81× |
| 0x02 K4R4E64  | 0.17 | ~1521 | 149 | 0.99× |
| 0x03 K8R4E64  | 0.26 | ~2005 | 153 | 0.93× |
| 0x04 K16R8E64 | 0.51 | ~1993 | 159 | 0.87× |

**Bulgular:**
1. **reconstruct ≫ encode (~1000×):** `RsSimdDecoder` HER çağrıda taze-kurulur (stateless-trait) → maliyet decode-değil
   **decoder-inşa-baskın**. D1-optimizasyon: profile-başına engine-reuse (`reset`) / cache. (İşlevsellik-etkilenmez.)
2. **recompute/encode ≈ 0.8–1.0×:** reed-solomon-simd tüm-R'yi tek encode()'da üretir → bu-backend'de tek-ESI
   **compute-saving YOK** (encode-then-select). On-demand-repair faydası latency/scheduling/bellek-erteleme, CPU-DEĞİL (§6 POC-bulgusu KANITLANDI).
3. **wire-byte deterministik:** sealed FEC-shard = 101 B (E=64); IPv4/IPv6/tünel « PMTU-tabanı (bkz. `pmtu-profile-table.md`).

> **"M1-M5" notu:** M1-M5 metrik-seti FEC-spec'lerinde TANIMLI-DEĞİL; repodaki `M4/M5` = kanal-durumu-sensing-katmanı
> (ayrı-track, FEC-ilgisiz). Bu-paketteki ölçüm-seti spec-çıpalı: {golden-byte, recompute==golden, PMTU-byte, throughput-indicative,
> supply-chain}. Sayısal-metrik-etiketi gerekiyorsa Abkarada-tanımı beklenir (fabrike-DEĞİL).

### 3.1 Sprint-3 (D1) ölçüm güncellemesi — engine-reuse + E=1024

| profile | encode µs | encode MB/s | recon-STATELESS µs | recon-REUSE µs | reuse-hızlanma |
|---|---|---|---|---|---|
| 0x03 K8R4E64  | 0.29 | ~1741 | 174 | 177 | **1.0×** |
| 0x05 K8R4E1024 | 0.94 | ~8685 | 196 | 188 | **1.0×** |
| 0x06 K16R8E1024 | 2.40 | ~6832 | 192 | 190 | **1.0×** |

**KRİTİK BULGU — D0-hipotezi ÇÜRÜDÜ:** D0, reconstruct'ın ~1000×-encode olmasını "decoder-İNŞA-baskın" varsayıp
`RsSimdEngine` reset()-reuse'unu ZORUNLU kılmıştı. **Ölçüm reuse'un hızlanma VERMEDİĞİNİ gösterdi (~1.0×).** Maliyet
`new()`/`reset()`-inşasında DEĞİL, reed-solomon-simd'in **decode-işleminin-kendisinde** (FFT-tabanlı fixed-cost;
küçük-K/R'de amortize-olmaz). `RsSimdEngine` yine de korunur (doğru, byte-eş `reuse==stateless==golden`, alloc-azaltır);
decode-fixed-cost'un düşürülmesi AYRI-optimizasyon-takibi (spec-kararı-değil; şimdilik büyük-E amortize-eder).

**E=1024 (Sprint-3, sub-kbps/M3):** encode ~8.7 GB/s (E=64'ün ~5×'i — büyük-blok fixed-cost'u amortize eder);
sealed FEC-shard = **1061B** (26+11+1024), app-payload 1013B, sabit-overhead **%3.5** (vs %36.6@E=64). IPv4/IPv6
datagram 1089/1109B — 1280-tabanının altında. Yeni profiller: 0x05 K8R4E1024 · 0x06 K16R8E1024.

## 4. STATÜ-MERDİVENİ (§ PATCH-H) — FEC-tarafı durum

| şart | durum |
|---|---|
| RS-POC + golden-vectors geçti | ✓ |
| known-zero + metadata-recovery testleri geçti | ✓ |
| PMTU/profile tablosu gerçek-byte-ölçümüyle | ✓ (`pmtu-profile-table.md`) |
| supply-chain doğrulandı | ✓ version/checksum/license/SBOM (`supply-chain-sbom.md`); `audit`/`deny` CI = D1 |
| sealed-type compile-time-enforce · seal(p).len()==sealed_len | ○ noise-wire-crate D1 (henüz-yok; N-raporları envanterledi) |
| zeroization (repair+scratch hassas) | ○ D1 (RS-scratch/repair-buffer zeroize kapsamı) |

## 5. Açık-uçlar → decision-request

- **DR-1** (N7): §2.57 control count-bounds [CAL]-sayısal-tanımsız → max-control sealed_len üst-sınırı hesaplanamaz.
- **DR-2** (N10): §0.7-B PRE_CONVERGE/KEYSTONE ↔ bounded-idempotent-INIT-flight-machine katman-özdeşleştirmesi (kod-envanteri farklı-katman gösteriyor).

## 6. Noise D0 UYUM-raporları (İŞ-B, RAPOR-ONLY)

`reports/noise-d0/N1..N10` — mevcut-kod §2.5-2.8/§2.56/§2.57/§4'e karşı envanterlendi. Özet: N1 seq16→full-PN
ccr-§35.8-UYUMLU; **INV-RATE-1 UYUMLU + FEC-recovered≠ACK UYUMLU** (N8, ihlal-yolu-YOK); control-allowlist UYUMLU
(N6, NACK/KEY_UPDATE-yok); kripto/session/handshake = iskelet (N2/N5/N9/N10 → D1-inşa); fec.rs FEC_SHARD(0x20)
§2.6-EXTENDED'e UYUMSUZ (N3/N4 → D1-migrasyon, plaintext-metadata-leak giderilir).
