# FS-RTT-REALITY-AUDIT — RTT/PATH-TIMING OBSERVABILITY (DR-FEC-FS-1 §FS-5 yeniden-değerlendirme)

**Statü:** FS-5 observability-riski kod-gerçeğiyle yeniden-değerlendirildi (2026-07-31). Amaç: "FEC işe
yarıyor ama controller RTT'yi göremiyor" iddiasının **mimari-imkânsızlık mı yoksa yalnız plumbing-eksiği mi**
olduğunu SONUÇ-ÜRETMEDEN kesinleştirmek. Sınır (§D): production-flip/default-FEC/cold-start DEĞİŞMEDİ.

## 0. HÜKÜM (baştan)

> ## RTT/path-timing state **ENGINE'DE TAM MEVCUT VE ÖLÇÜLÜYOR**. Sorun mimari-DEĞİL → **PLUMBING_GAP**
> (FEATURE_NOT_EXPOSED): `FusionInputs` yalnız-ORAN taşıdığından mutlak srtt/min_rtt FEC-selector'a
> ulaşmıyordu. **FS-5A ÇÖZÜLDÜ** (read-only `path_timing_snapshot()` exposure eklendi; production DEĞİŞMEDİ).
> **FS-5B (RTT gerçekten discriminative-feature mı) AÇIK** — confirmatory-run'la gösterilecek, VARSAYILMADI.

Önceki rapor ("RTT gözlemlenemiyor") **yanlış-genellemeydi**: "FusionInputs'ta-yok" ≠ "engine-ölçmüyor".

## A. RTT REALITY

### A.1 RTT MEVCUT MU? → **EVET, tam decompose-edilmiş**
`crates/adaptive-transport/src/rtt.rs` — `RttEstimator` (rtt.rs:65) ZATEN üretir:

| metrik | accessor | üretim |
|---|---|---|
| latest sample | `observe(now, sample)` işler | ACK-yolu (send↔ack eşleştirme; `rtt_sample`/`apply_ack_delay`) |
| smoothed RTT (srtt) | `srtt() -> Option<Duration>` | RFC-6298 EWMA α=1/8 (rtt.rs:162-167) |
| RTT varyansı | `rttvar() -> Duration` | RFC-6298 β=1/4 (rtt.rs:160) |
| **min/base RTT (RTprop)** | `rtprop() -> Option<Duration>` | pencereli-min-filter (BBR-tarzı; W=10s kanal-türevi; rtt.rs:203) |
| **queue_delay** | `queue_delay() -> Duration` | `min_W_qd(rtt) − rtprop` (ayakta-kuyruk; rtt.rs:212-219) |
| freshness | `rtprop_age_ns(now)` + `rtprop_window_ns()` | min-örnek-yaşı vs pencere (rtt.rs:189/197) |

→ Kullanıcının §5'te istediği decompose **birebir mevcut**: base_rtt/min_rtt(rtprop) · srtt · queue_delay
(= srtt−min_rtt'den DAHA-iyi: kısa-min−rtprop) · rtt_var(rttvar) · measurement-confidence(srtt-var-mı +
rtprop-tazeliği). ACK-timestamp/send-timestamp eşleştirmesi de var (`apply_ack_delay` RFC-9002 min-rtt-guard'lı).

### A.2 ÜRETİCİ MODÜL / VERİ AKIŞI
```text
ACK gelir → sender on_ack_received (sender.rs:1717) → apply_ack_delay(raw, ack_delay, rtprop)
          → RttEstimator.observe(now, sample)  [rtt.rs:142]
          → srtt/rttvar EWMA + rtprop.observe(min-filter) + qd_min.observe(kısa-min-filter)
```
Yaşam-süresi: per-session (Sender.rtt); her-ACK güncellenir. Kanal-türevi pencereler (`set_rtprop_window`,
`set_qd_window`, `set_conv_interval`) sender tarafından yazılır (ZM-1/3a estimator-clock).

### A.3 FRESHNESS / CONFIDENCE
```text
srtt = None  → henüz RTT-sample-yok (confidence-yok)
rtprop = None → aynı
rtprop_age_ns(now) < rtprop_window/2 → min-örnek TAZE (aksi tazeleme-gerek; kanal-saatli, sabit-tick-değil)
```

### A.4 MEVCUT CONSUMER'LAR (RTT zaten çok-yerde kullanılıyor)
```text
srtt() : pacing/PTO fallback (INITIAL_RTT), loss-deadline (next_time_loss_deadline_ns), cold-start bootstrap
         (cs_phase2_resolve), BtlBw-pencere, expected_ack_interval, jitter-türevi. (sender.rs:1148/1307/1336/
         1348/1373/1580/1763/1827/1887 …)
rtprop(): apply_ack_delay-guard, qd_ratio-türevi, PTO-hesabı (sender.rs:1606/1717/2003/1898)
queue_delay()/rtprop: diag_qd() SALT-GÖZLEM (sender.rs:3512) — kısmi-exposure zaten var (srtt/rttvar-hariç)
```

### A.5 FUSIONINPUTS'A KADAR EKSİK HALKA (kök-neden)
`fuzzy_evaluate` (sender.rs:1887-1909) fusion-girdisini kurarken **srtt'yi (1887) ve rtprop'u (1898)
OKUR** — ama **ORAN'a çevirir ve mutlak-magnitude'ı DÜŞÜRÜR**:
```text
jitter_ppm     = rttvar / srtt              (boyutsuz volatilite-oranı)   ← srtt magnitude ATILDI
qd_ratio_permille = queue_delay / rtprop    (boyutsuz kuyruk-oranı)       ← rtprop/base magnitude ATILDI
FusionInputs { loss_ppm, bandwidth_bps, jitter_ppm, qd_ratio_permille, ack_confidence_permille }
```
`FusionInputs`'ta mutlak srtt/min_rtt taşıyacak **ALAN YOK**. FEC-selector fuzzy'den geçtiği için tarihsel-
fayda-ayırıcısı **base-RTT-magnitude'unu göremiyordu**. Bu **tek eksik halka** = plumbing.

## B. MINIMAL PATCH — TASARIM + UYGULANAN

Engine yeni-estimator-GEREKTİRMİYOR (RTT zaten ölçülü). Eksik = mutlak-değerlerin selector'a forward'ı.
```text
ACK/path estimator (rtt.rs RttEstimator — MEVCUT; DEĞİŞMEDİ)
    ↓  (salt-okuma; yeni-ölçüm-yok)
PathTimingState  [UYGULANDI: sender.rs]
    ├── srtt_ns          (Option)
    ├── min_rtt_ns       (Option; = rtprop)
    ├── rttvar_ns
    ├── queue_delay_ns   (= min_W_qd − rtprop)
    ├── sample_present   (confidence)
    └── rtprop_fresh     (freshness)
    ↓
Sender::path_timing_snapshot()  [UYGULANDI; #[must_use] read-only]  → experimental FEC-selector + FS-harness
```
**UYGULANAN (bu-tur):** `PathTimingState` struct + `Sender::path_timing_snapshot()` read-only accessor.
- **Production-policy DEĞİŞMEDİ:** fusion/`FusionInputs`/fuzzy-decide/wire DOKUNULMADI. Üretim-çağıranı YOK.
- flag-OFF byte-özdeş korunur (additive salt-getter; davranış-değişimi-yok). Kapılar: fmt+clippy(2cfg) +
  FLAG-OFF 380→382 + ALL-FEATURES 499→501 (+2 additive) + A1-pin (cs_liveness4/dr_cs1_wire12) sağlam.
- Test: `tests/fs5_path_timing.rs` — Case-A/B engine-discrimination + snapshot-forward + confidence.

**GELECEK (FS-5B-sonrası, AYRI-onay):** RTT discriminative-çıkarsa, `FusionInputs`'a mutlak-alan-ekleme
VEYA experimental-selector'ı doğrudan `path_timing_snapshot()`'a bağlama — ama **production-fuzzy-vektörü
ancak §10-offline/paired-replay-değerlendirmesi + ayrı-onaydan sonra** (şimdi DEĞİL).

## C. FS-5 KARAR TABLOSU

| Durum | Koşul | Bu-repo |
|---|---|---|
| **PLUMBING_GAP** | RTT mevcut, yalnız expose-edilmemiş | ✅ **BU** — state MEVCUT (rtt.rs); FusionInputs-oranı düşürüyordu |
| IMPLEMENTATION_GAP | RTT ölçülebilir ama estimator-eksik | ✗ değil (RFC-6298 srtt + BBR-rtprop + qd ZATEN var) |
| OBSERVABILITY_INSUFFICIENT | RTT var ama FEC-region'ı ayıramıyor | **FS-5B AÇIK** (confirmatory-run-gerekir; VARSAYILMADI) |
| ONLINE_SELECTABLE | RTT/path-timing region'ı güvenilir-ayırır | **FS-5B AÇIK** (kanıtlanınca) |

**FS-5 ikiye-bölündü (§7):**
- **FS-5A — Engine güvenilir RTT/path-timing üretiyor mu (runtime)?** → **EVET; PLUMBING_GAP; ÇÖZÜLDÜ** (exposure eklendi).
- **FS-5B — Bu-state FEC-beneficial-region'ı güvenilir-ayırıyor mu?** → **AÇIK.** Veri-gelmeden-VARSAYILMAZ;
  RTT'nin gerçekten causal/discriminative-feature olup-olmadığı FS-1/confirmatory-run'la (özellikle C-P1/C-P2
  RTT-eksenli hücreler) gösterilecek. (Not: tarihsel-fayda RTT≤250'de; ayırıcı base-RTT-magnitude — artık expose.)

## Case-A / Case-B ANALİZİ (Abkarada; kod-doğrulandı + test-pinli)
```text
Case-A: min/base≈220ms, srtt≈240ms, qd düşük  → rtprop()≈220, queue_delay()≈düşük
Case-B: min/base≈30ms,  srtt≈240ms, qd yüksek → rtprop()≈30,  queue_delay()≈yüksek
```
İkisi de AYNI srtt (≈240) → **srtt-only AYIRAMAZ** (kullanıcının-uyarısı). AMA engine `rtprop` (base) ve
`queue_delay`'i AYRI tuttuğundan **AYIRIR**. Kanıt: `fs5a_case_a_vs_b_engine_discriminates_at_equal_srtt`
(srtt-diff<30ms iken min_rtt 220-vs-30, queue_delay düşük-vs-yüksek DECISIVE). Bilgi engine'de MEVCUT;
tek-`latest_rtt` veya tek-`srtt` selector'a-vermek YETERSİZ olurdu (bu ayrım kaybolurdu) — bu yüzden
`PathTimingState` **beşli-decompose** (srtt+min_rtt+rttvar+queue_delay+confidence) taşır.

## §6 — JITTER, RTT-YERİNE GEÇER Mİ? → **HAYIR (açık-red)**
`jitter_ppm = rttvar/srtt` **boyutsuz volatilite-oranı**; base-RTT-magnitude'u TAŞIMAZ. İki-path aynı-jitter'a
sahip olup base-RTT 30ms vs 220ms olabilir. Tarihsel-fayda-rejimi RTT-SEVİYESİYLE (≤250ms) tanımlı, volatilite'yle
değil. "Jitter var → RTT gereksiz" **REDDEDİLDİ**: jitter varyansı ölçer, rejim-ayıran RTT-seviyesini değil.

## D. PRODUCTION-DEĞİŞİKLİK YOK (teyit)
Production-flip YOK · default-FEC-activation YOK · cold-start-değişiklik YOK · FusionInputs/fuzzy-vektör/wire
DEĞİŞMEDİ. Eklenen tek-şey read-only `path_timing_snapshot()` (üretim-çağıranı-yok; experimental/FS-harness-only).

## PROVENANCE
```text
kod: crates/adaptive-transport/src/rtt.rs (RttEstimator) · src/sender.rs (PathTimingState + path_timing_snapshot + fuzzy_evaluate:1887-1909)
test: crates/adaptive-transport/tests/fs5_path_timing.rs (2/2 PASS)
supersedes-partial: FS-REALITY-AUDIT.md §3.3 "CONTROLLER OBSERVABILITY BLOCKED" → daraltıldı:
  FS-5A=PLUMBING_GAP-ÇÖZÜLDÜ; yalnız FS-5B(discrimination) açık.
```
