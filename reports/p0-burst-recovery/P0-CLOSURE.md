# P0 — Patlamalı-Kayıp Kurtarma Düzeltmesi (KAPANIŞ)

**Statü:** MEKANİZMA DÜZELTİLDİ + DOĞRULANDI. Kaynak: kapi-eval-v1 exploratory h3 negatif bulgusu
(Kama patlamalı-kayıpta Quinn'in gerisinde — jammer/EW iddiasının kalbi).

## Kök neden (birim-izinden doğrulandı — tahmin değil)

`KAMA_BENCH_TRACE=1` ile birim-düzeyi TX/ACK izi eklendi. h3b'de 12 teslim-edilmeyen birimin 9'u
aynı imza: `Data → Retransmit×3 → Duplicate → [koşu sonuna dek SESSİZLİK]`. Mekanizma: orta-akışta
`arq_retx_cap=3` aşılınca `RetxDecision::CapExceeded{HandToForward}` **tek-atış** (`interval_ns: 0`)
forward kopyası planlıyordu; o kopya patlamada kaybolunca `queue_losses` Duplicate-kökenli kaybı
`forget_in_flight` ile düşürüp **birimi terk ediyordu** → birim ~150 s boyunca bir daha hiç denenmedi.
Bu, recovery-obsesif tabanın (§9.1 "veri ULAŞSIN") doğrudan ihlaliydi; **bilinçli trade-off değil, kusur.**

## Düzeltme (minimal, mevcut vetted makine)

`queue_losses` Duplicate-branch: birim hâlâ outstanding'se (ACK olsaydı `on_ack` inflight'tan siler +
`dup_queue.retain` kopyayı iptal ederdi → buraya ancak gerçekten-outstanding birim düşer) **geometrik
self-reschedule** (×2 backoff, ACK-ile-iptal) forward kopya yeniden kurulur. Saf-sessizlik/PreConverge
yollarının kullandığı makinenin aynısı. Birim-başına ≤1 dup guard (bounded-alloc, SR-AUDIT-001 korunur).
INV-RATE-1'e dokunmaz (dup_queue token-paced), loss metriğine dokunmaz, FEC'e dokunmaz.

## Doğrulama

**Deterministik regresyon kapısı** (`crates/adaptive-transport/tests/burst_loss_recovery.rs`; LinkSim,
RNG'siz; CI'da `cargo test` ile otomatik): Sınıf-A (orta-akış, orijinal+3retx+kopya aynı patlamada
kaybolur → birim YİNE teslim) ve Sınıf-B (kuyruk, sonraki paket yok → saf-sessizlik yolu kurtarır).
Fix-öncesi Sınıf-A FAIL, fix-sonrası ikisi de PASS.

**netem (gerçek 3-netns, yeni binary, AYNI drand tohumu, h3b rep0-2):**

| kol | fix-öncesi | fix-sonrası | Δ | undeliv öncesi→sonrası |
|---|---|---|---|---|
| **kama-tail-v1** (primary) | 0.927 | **0.983** | **+0.057** | 7.3 → 1.7 |
| kama-arq | 0.910 | 0.917 | +0.007 | 8.7 → 8.3 |
| q-dgram-arq (kontrol) | 1.000 | 1.000 | 0 | 0 → 0 |
| q-stream (kontrol) | 1.000 | 1.000 | 0 | 0 → 0 |

Primary Kama kolunun q-dgram'a açığı **−7.3pp → −1.7pp** (büyük ölçüde kapandı). Rep-bazı tutarlı
(rep0 6→0, rep1 11→2, rep2 5→3 undeliv).

## Gerileme yok

Motor lib 146/179/145/180 (4 feature-matrisi), clippy 0 (default+all-features), fmt temiz, bench 27/27.
Bankalar (2290-satır tail-grid, FS-1, kapi-eval-v1) yeniden koşulmadı (r5 M: motor fix = yeni commit).

## Kalan (P0-sonrası, ayrı kalem — kapsam genişletme YOK)
- **kama-arq residual:** tail-OFF kolu daha az iyileşti (0.910→0.917); bu kol primary değil, ama başka
  bir terk-yolu olabilir — teşhis kuyruğunda (izin gösterdiği: kama-arq birimleri farklı ölür).
- **Tam yeniden-doğrulama** (n=10, h3a/b/c, iki kol) exploratory tamamlama backlog'unda birleşir.
- **p5-burst rep5 stall** (kama-tail-v1 0/100, gönderici ~4kB sonra sustu) ayrı anomali; ayrı teşhis.
