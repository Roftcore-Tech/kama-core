# 1b Policy-Kernel Adversarial-Simulation (Commit-R+S) [saf-kernel; üretim-wiring-YOK; policy-uygulaması-DEĞİL]

> Kernel: `adaptive-transport/src/budget_kernel.rs` (deneysel-modül; `plan_fec_parity`-KABLOSUZ;
> default-ürün-davranışı-değişmez). Sim: `tests/budget_kernel_sim.rs` — 20-zorunlu-adversarial-trace ×
> 4-candidate. Veri: [data/policy-kernel-sim/bks-results.csv](data/policy-kernel-sim/bks-results.csv).
> Yeniden-üretim: `BKS_OUT=<dir> cargo test --release -p adaptive-transport --test budget_kernel_sim
> budget_kernel_sim -- --ignored` (deterministik; contended-trace-çift-koşu-bit-özdeşliği CI-smoke'ta).

## 1. Sim'in-yakaladığı-İKİ-tasarım-hatası (kernel-revizyonuna-yol-açtı — sürecin-değeri)

**K1 — Akışkan-muhasebe × kuantalı-emisyon:** ilk-kernel step()-içinde grant'ı-credit'ten-düşüyordu;
emisyon-kuantalı (psize) olduğundan kuantum-altı-turlarda credit-hiç-birikmiyor, parity-HİÇ-emit-
edilemiyor, DATA-boşuna-displace-oluyordu. **Çözüm: İKİ-FAZ** — `step()`=yalnız-limit;
`settle(spent)`=GERÇEK-harcama-muhasebesi (harcanmamış-grant-credit-eritmez; kuantum-altı-birikim-
korunur; testli: "harcanmamış-grant-eritmez").

**K2 — Displacement-metrik-artefaktı:** bucket'ta-BİRİKEN-rezerv her-tick-yeniden "displaced"-
sayılıyordu (×5.5-şişme). **Çözüm:** displacement ≡ entitlement'tan-GERÇEKTEN-harcanan-parity-byte
(DATA'nın-kaybettiği-throughput-payı) + `max_reserved_pool` (gecikme-proxy'si; 1-kuantum=1200B-tepe).
Rezerv-havuzu-bucket'ta-birikir → kuantum-doğal-çözülür (ayrıca-kuantize-rezerv-denendi-ve-REDDEDİLDİ:
bucket-birikimini-öldürüyor).

## 2. Ana-sonuçlar (tam-tablo-CSV'de)

| trace (kritik) | L-Leftover | ProtectedShare | FloorCap (F2+C1) | BoundedDebtFloor |
|---|---|---|---|---|
| offered=C (saturated) | **parity=0 (üçgen-T1∧T2⇒¬T3 kanıtı)** | 288000B-parity; displaced=288000 (=spend; ≤%20-pay) | aynı-toplam; **ticket 60/60, miss 0** | ≡FloorCap (borç-hiç-gerekmedi) |
| offered>C (overload) | parity=0; backlog-büyür | parity-akar-bounded; backlog-politika-bağımsız-büyür | ticket 60/60 | ≡ |
| many-blocks>floor-capacity | parity=0 | bounded-akış | admitted-91 (ticket-cap-throttle), serviced-91, **miss-19-SAYILDI** (koşullu-sözleşme-dürüstlüğü) | ≡ |
| rtt-5s / bursty / interleaved | 0 / 0 / 0 | bounded-akış ✓ | ticket-tam-servis ✓ | ≡ |
| low-bw-30kbps | bulk-2714-tick-gecikme | aynı | **floor-ilk-parity-önceliği; 17/22-servis, 22-miss-kayıtlı** | ≡ |
| no-data-demand | tam-work-conserving (288000-bulk) | ≡L ✓ | ≡ (cap-suppression-29-olay: hardcap-akış-sınırı-görünür) | ≡ |
| credit-max-after-idle | — | idle-birikim CAP'te-durur (4800) — lifetime-birikim-YOK ✓ | ✓ | ✓ |
| timer-wrap (elapsed=2^63) | panik-yok (saturating) ✓ | ✓ | ✓ | ✓ |
| mode-transition | — | reset()-temiz (kalıntı-credit/debt-taşınmaz) ✓ | ✓ | ✓ |
| recovered-cancel | — | ticket+bulk-temizliği-bozulmaz ✓ | ✓ | ✓ |
| pending-item/byte-cap | — | admission-typed-red (31/19-admit; kalan-red-sayıldı) ✓ | ✓ | ✓ |

İnvariant'lar-her-tick-assert'li (token-icadı-yok; parity≤avail+borç-izni; work-conserving; debt≤cap;
queue≤item-cap+push-4+ticket-cap) + contended-çift-koşu-bit-özdeşlik (determinizm-DUR-koşulu) ✓.

## 3. Okuma (candidate-karşılaştırma; kernel-düzeyi — CANLI-motor-kanıtı-DEĞİL)

- **L (baseline):** saturated-hedef-rejimde yapısal-starvation (üçgen); temiz-rejimde-yeterli.
- **ProtectedShare (R):** bounded-pay-akışı-her-trace'te; per-block-fairness-YOK (ilk-parity-garantisiz).
- **FloorCap (R+F2+C1):** ProtectedShare + per-block-ilk-parity-önceliği + hard-cap-akış-sınırı +
  DÜRÜST-miss-sayımı. Toplam-parity-aynı (fairness-yeniden-dağıtım). **Birincil-aday.**
- **BoundedDebtFloor (B+C2-dar):** bu-trace-setinde FloorCap'ten-AYRIŞMADI (max_debt=0 — borç-hiç-
  gerekmedi: entitlement+leftover due-time-öncesi-hep-yetti). Marjinal-değeri-kanıtlanmadı →
  **ikincil-varyant** (canlı-prototipte-yalnız-FloorCap-yetersiz-kalırsa).
- Displacement-üst-sınırı = entitlement-payı (%20 [CAL]) — ölçülen-tam-uyum (spend==displaced;
  saturated'da ≤%16-realized).

**SINIR:** kernel-sim kanal-feedback'i (loss/RTT/BtlBw-tepkisi) içermez; contended-hedef-rejimin-
NİHAİ-kanıtı gerçek-motor-prototipi-kapısına-aittir (DR-1b-v2-§canlı-prototip-kapısı). Bu-sonuçlar
"kernel-matematiği-kapalı-ve-tutarlı"-kanıtıdır, politika-seçim-onayı-değil.
