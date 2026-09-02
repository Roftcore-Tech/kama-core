# S1-GATE — Ürün FEC-runtime, netem (gerçek RTT) FEC-OFF vs FEC-ON

**Tarih:** 2026-08-26 · **Harness:** `scripts/netem/s1_gate.sh` (iki-netns kapi3 + netem; `examples/p1_endpoint --fec-r/--fec-adaptive`) · profil 0x03 (K8/R4/E64).
**Amaç (S1 tez de-risk):** uykuda FEC ürüne bağlandıktan sonra, gerçek-link üstünde (loopback-artefaktı yok) FEC (a) zarar veriyor mu, (b) ARQ'nun bıraktığını kurtarıyor mu?

## Sonuçlar (completion = acked/units; delivered server-drain-yarışı yüzünden ≤ acked)

| kol | IID %30, RTT50 | burst gemodel40, RTT100, d3s | burst gemodel50 (h3b), RTT100, d12s, 5rep |
|---|---|---|---|
| FEC-OFF (ARQ-only) | 100% | 98.3% | **99.0%** (bir rep 76/80) |
| **FEC-static-r4** | 100% | **100%** | **100.0%** (5/5) |
| FEC-adaptive (Controller) | 100% | 100% | 99.2% |

## Hükümler

1. **NO-HARM (birincil gate — PASS):** ürün FEC hiçbir hücrede OFF'a göre gerilemedi. Sevk-güvenliği: FEC'i ürüne bağlamak transport'u bozmuyor.
2. **FEC burst-residüsünü kurtarıyor:** h3b'de ARQ-alone %99'da residü bırakırken static-r %100 — orijinal negatif bulguyu (Kama burst'te geride) ürün-düzeyinde doğrudan adresliyor. Sinyal yönlü ve modest (küçük Δ, az rep); **belirleyici kanıt** (ARQ'nun İMKANSIZ olduğu kalıcı-erasure → FEC 8/8) in-process deterministik testte (`runtime_e2e::fec_static_r_recovers_permanently_erased_units_over_udp`).
3. **BULGU (gelecek kalem, kablolama-bug'ı DEĞİL):** adaptive Controller burst'te static-r'ı yakalamadı — structure-gate `burst_delta > 50_000` → FEC'i ARQ'ya bırakıyor (Q17 "burst→ARQ" kuralı). Deadline-baskılı gerçek-zamanlı burst'te FEC proaktif-tek-atış ARQ'nun çok-turlu retransmit'ini yendiğinden, **"burst→ARQ" kuralı deadline-kısıtlı op için yeniden-kalibre edilmeli** (fusion-gate kalibrasyonu; ön-kayıtlı kampanya gerektirir, bu tur YAPILMAZ).

## Kapsam / sınır
- quinn kolu YOK (bu harness p1_endpoint; "FEC-ON Kama vs quinn burst-ahead" formal iddiası bench/kapi_calrun'la — daha büyük koşu, ertelendi; tez zaten 868-0 matched-budget + fusion-cell ile bankada).
- Screening/ekran-teşhisi; ön-kayıtlı hüküm değil. UZUN kampanya YOK (Abkarada).
