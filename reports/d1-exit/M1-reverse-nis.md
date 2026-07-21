# M1 — REVERSE-NİŞ ÖLÇÜMÜ (bilimsel-deney raporu) — SONUÇ NE-ÇIKTIYSA-O

> **HÜKÜM (dürüstlük-protokolü §4):** Bu sim-modelinde ve bu matris-parametrelerinde **Kama-FEC-arm HİÇBİR
> hücrede goodput_per_sealed_byte'ta kazanmadı (ARQ 12/12); completion her-hücrede-iki-arm 1.000; zaman-metrikleri
> ~eşit.** Beklenen-niş (yüksek-reverse'te Kama>ARQ) **bu-modelde DOĞRULANMADI.** Bu KRİTİK-BULGU gizlenmiyor;
> kök-analiz aşağıda — bulgular niş-varsayımını çürütmekten-çok **modelin-niş-koşullarını-üretemediğini VE
> motorun-kendi-forward-recovery'sinin FEC'i-gölgelediğini** gösteriyor. Suppress-mekanizması (5d-wiring)
> ÇALIŞIYOR (rev95/fwd5'te suppress=1.00) ama hacimce-küçük. Overclaim-yok; sınırlar beyanlı.

## 0. Deney-kimliği
- Harness: `kama-engine/tests/m1_reverse_nis.rs` (production-graph-DIŞI; **sapma-notu:** kama-integration
  yerine kama-engine/tests — Sender+RecoveredMap+register altyapısı orada; aynı-şekilde graph-dışı dev-test).
- ÜÇ-KİLİTLİ-KARAR uygulandı: KARAR-1 overhead-normalize (cap-dayatma-yok; parity+retransmit paydada) ·
  KARAR-2 evicted==0-guard (12/12-hücrede SAĞLANDI; ack_miss salt-teşhis-sütunu) · KARAR-3 iki-arm AYNI
  Pto/SACK-kadansı (tek-motor; tek-fark `fec_enabled`).
- Model: paket-düzeyi deterministik-sim (5-seed medyan); **kapasiteli-FIFO forward-link C=50 pkt/s**
  (serialization 20ms + tek-yön 50ms ⇒ RTT≈140-200ms yük-bağlı); akışlı-yük offered=C/2 (FEC/retx-headroom);
  W=400 unit×1200B; kümülatif-SACK 50ms-kadans; RECOVERED SACK-piggyback (aynı-reverse-drop); MDS-sayım-decode.
  Gerçek Sender-motoru (adaptive-transport) iki-arm'da.

## 1. 12-HÜCRE TABLO (medyan, 5-seed; FEC-satırı / ARQ-satırı)

| rev% | fwd% | arm | goodput/byte | compl. | t_p50 ms | t_p95 ms | cancels | suppress | evict | miss | sealed MB | parity | decoded | recv_batch |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 50 | 1 | FEC | 0.6446 | 1.000 | 8070 | 15270 | 0 | — | 0 | 2 | 0.74 | 199 | 0 | 0 |
| 50 | 1 | ARQ | **0.9653** | 1.000 | 8070 | 15270 | — | — | 0 | 0 | 0.50 | — | — | — |
| 50 | 5 | FEC | 0.6259 | 1.000 | 8070 | 15270 | 2 | 0.02 | 0 | 14 | 0.77 | 199 | 12 | 123 |
| 50 | 5 | ARQ | **0.9328** | 1.000 | 8070 | 15270 | — | — | 0 | 0 | 0.51 | — | — | — |
| 50 | 15 | FEC | 0.5656 | 1.000 | 8230 | 15310 | 4 | 0.09 | 0 | 29 | 0.85 | 198 | 16 | 83 |
| 50 | 15 | ARQ | **0.8309** | 1.000 | 8110 | 15330 | — | — | 0 | 0 | 0.58 | — | — | — |
| 70 | 1 | FEC | 0.6456 | 1.000 | 8070 | 15270 | 0 | — | 0 | 1 | 0.74 | 199 | 0 | 0 |
| 70 | 1 | ARQ | **0.9653** | 1.000 | 8070 | 15270 | — | — | 0 | 0 | 0.50 | — | — | — |
| 70 | 5 | FEC | 0.6269 | 1.000 | 8070 | 15270 | 2 | 0.03 | 0 | 11 | 0.77 | 199 | 8 | 62 |
| 70 | 5 | ARQ | **0.9373** | 1.000 | 8070 | 15270 | — | — | 0 | 0 | 0.51 | — | — | — |
| 70 | 15 | FEC | 0.5656 | 1.000 | 8350 | 15350 | 1 | — | 0 | 26 | 0.85 | 198 | 2 | 12 |
| 70 | 15 | ARQ | **0.8268** | 1.000 | 8130 | 15350 | — | — | 0 | 0 | 0.58 | — | — | — |
| 90 | 1 | FEC | 0.6478 | 1.000 | 8130 | 15270 | 0 | — | 0 | 0 | 0.74 | 198 | 0 | 0 |
| 90 | 1 | ARQ | **0.9677** | 1.000 | 8090 | 15270 | — | — | 0 | 0 | 0.50 | — | — | — |
| 90 | 5 | FEC | 0.6414 | 1.000 | 8270 | 15330 | 0 | — | 0 | 6 | 0.75 | 198 | 0 | 0 |
| 90 | 5 | ARQ | **0.9328** | 1.000 | 8150 | 15270 | — | — | 0 | 0 | 0.51 | — | — | — |
| 90 | 15 | FEC | 0.7808 | 1.000 | 10630 | 15470 | 0 | — | 0 | 0 | 0.61 | 23 | 0 | 0 |
| 90 | 15 | ARQ | **0.8239** | 1.000 | 10630 | 15620 | — | — | 0 | 0 | 0.58 | — | — | — |
| 95 | 1 | FEC | 0.6467 | 1.000 | 8190 | 15270 | 0 | — | 0 | 1 | 0.74 | 198 | 0 | 0 |
| 95 | 1 | ARQ | **0.9653** | 1.000 | 8070 | 15270 | — | — | 0 | 0 | 0.50 | — | — | — |
| 95 | 5 | FEC | 0.6382 | 1.000 | 8370 | 15310 | 2 | **1.00** | 0 | 4 | 0.75 | 192 | 3 | 1 |
| 95 | 5 | ARQ | **0.9328** | 1.000 | 8190 | 15270 | — | — | 0 | 0 | 0.51 | — | — | — |
| 95 | 15 | FEC | 0.7549 | 1.000 | 11850 | 16170 | 0 | — | 0 | 2 | 0.64 | 33 | 0 | 0 |
| 95 | 15 | ARQ | **0.8204** | 1.000 | 11850 | 16170 | — | — | 0 | 0 | 0.59 | — | — | — |

## 2. ÜSTÜNLÜK-HARİTASI

```
                fwd=1%      fwd=5%      fwd=15%
rev=50%         ARQ         ARQ         ARQ
rev=70%         ARQ         ARQ         ARQ
rev=90%         ARQ         ARQ         ARQ        (goodput/byte; zaman ~eşit → ARQ-sweep 12/12)
rev=95%         ARQ         ARQ         ARQ
```
**Hipotez-vs-sonuç:** "yüksek-reverse'te Kama>ARQ" — **bu-modelde ÇÜRÜDÜ.** "düşük-reverse'te ARQ≥Kama" — doğrulandı.

## 3. KÖK-ANALİZ (neden; dört-etken — hepsi ölçüm-teşhislerinden)

1. **Kümülatif-SACK reverse-loss'u massediyor.** %95-reverse'te bile 50ms-kadanslı-SACK ~1s'de-bir geçiyor ve
   KÜMÜLATİF olduğu için TAM-bilgi taşıyor → ARQ'nun kayıp-tespiti gecikiyor-ama-çökmüyor; 16s'lik-akışta ~1s-tespit-
   gecikmesi marjinal (t_p50-farkı ≈0). Reverse-niş "feedback-pahalı" değil "feedback-İMKANSIZ/epizodik" rejimlerde
   aranmalı (uzun-blackout, tek-yönlü-link) — bu-matrisin-parametre-uzayı onu-içermiyor.
2. **Motorun KENDİ forward-recovery'si FEC'i gölgeliyor (mimari-bulgu).** rev-yüksek → ackconf-LOW → `RECOVER_FWD`
   ACK'siz-duplication zaten-devrede: kayıp-source'u motor kendisi (dup) yeniden-yolluyor; dup-arrival bloğu-
   doldurunca FEC-decode'un-kurtaracağı-eksik-kalmıyor (rev=90/95'te decoded=0'ın ana-nedeni). **Kama'nın reverse-
   dayanıklılığı FEC'ten-önce CC-katmanının-kendi-mekanizmasından geliyor** — white-paper-için değerli-pozitif-bulgu,
   ama "FEC-nişi"ni-bu-motorda-daraltıyor.
3. **Parity-starving: DATA-ÖNCE mühürlü-sıralamada parity token-ARTIĞINA akar.** Doygun-yükte (offered≈C) parity
   fiilen-hiç-çıkmadı; offered=2C/3'te retx/dup'la-yarışta yine-kıtlık; ancak offered=C/2-headroom'da tam-aktı.
   Yüksek-fwd-hücrelerde (15%) retx/dup-yoğunluğu parity'yi yeniden-boğdu (parity 199→23-33). FEC'in-işe-yaraması
   parity'nin-ZAMANINDA-çıkmasına bağlı; mevcut-politika bunu-garanti-etmiyor (politika-tartışması: ayrı-DR-konusu,
   mühürlü-sıralamaya-dokunmadan-önce).
4. **Deadline-siz-metrik geç-teslimi-cezalandırmıyor.** completion@300s hep-1.000 → FEC'in "erken-teslim"-değeri
   goodput-metriğine-yansımıyor; FEC'in-parity-maliyeti ise-doğrudan-paydada. Deadline-bounded-completion (M2) ve
   yüksek-RTT/trafo-profilleri (M5) nişi-yeniden-test-edecek doğru-zemin.

**Suppress-kanıtı (mekanizma-çalışıyor):** rev=95/fwd=5 hücresinde suppress=**1.00** (tespit-geç → RECOVERED
pending-retransmit'i-tam-yakaladı; cancels=2) — 5d-wiring'in amaçlanan-davranışı CANLI. Ama decode-hacmi-küçük
(yukarıdaki-1-2-3) → toplam-etki goodput'u-çeviremiyor.

## 4. ÖLÇÜM-ARACI-DOĞRULAMA TARİHÇESİ (measurement-tool-first; ilk-tablolar SONUÇ-DEĞİL)

| İter | Kusur (teşhis-sütunlarıyla yakalandı) | Düzeltme |
|---|---|---|
| 1 | Kapasitesiz-kanal + bulk-yük → parity-yolu-ÖLÜ (cancels=0, FEC≡ARQ-zaman, sealed-farkı≈0) | kapasiteli-FIFO (C=50pps) + akışlı-yük |
| 2 | decoded=50-yanıltıcı (eksiksiz-blok "decode"-sayılıyordu) + offered≈C→parity-starving | decode=yalnız-eksik-kurtarımı; offered=2C/3 |
| 3 | offered=2C/3'te retx/dup-yarışında parity yine-kıt | offered=C/2 (final); teşhis-sütunları-kalıcı |

## 5. SINIRLAR + SONRAKİ-ADIMLAR (mimari-sınır-beyanı)

- **Model-sınırı:** IID-loss (GE-burst değil), sabit-RTT≈150ms, tek-C, kümülatif-SACK-idealize, deadline-siz.
  Niş-koşulları (blackout-epizot, RTT≫, deadline) M2/M5-kapsamında — **niş-varsayımı bu-deneyle NE-kanıtlandı
  NE-tümden-çürüdü; bu-parametre-uzayında yok.**
- **Mimari-izinli-düzeltme-adayları (uygulanmadı; tartışma-konusu):** (i) parity-önceliklendirme/oran-politikası
  (DATA-ÖNCE mühürlü — değişiklik ancak-DR'la), (ii) RECOVER_FWD-dup ↔ FEC-parity bütçe-koordinasyonu (aynı-
  headroom'u-paylaşıyorlar; §8.4-fusion-alanı), (iii) RECOVERED-bildirim-güvenilirliği (piggyback-tekrar zaten-var).
- **K1 (spurious-gap)**: bu-modelde ack_miss FEC-arm'da 1-29 (Recovered→geç-ACK/dup-yarışı, beklenen); gerçek-
  spurious-vergi-ölçümü M3-netem'de.
- evicted=0 12/12 (KARAR-2 ✓; CAP=16384 yeterli — DR-S5-6-gerekçesi-doğrulandı).

## 6. NON-REGRESSION
{A}: engine 15→16 (+1 m1_smoke; m1_matrix #[ignore]-ağır); diğerleri değişmedi (nw94/fec25/sch9/integ12/at130).
{B} desen1 5-pin PASS. {C} FEC-off yapısal PASS. fmt/clippy temiz. Mühürlü-kod dokunulmadı (harness-only).
