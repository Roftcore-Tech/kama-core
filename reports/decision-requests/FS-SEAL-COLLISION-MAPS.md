# FS-SEAL-COLLISION-MAPS — in-stack FEC/duplication family map (DR-FEC-FS-1 v5 §7.5/§7.8)

**Statü:** Her in-stack family için KODDAN-ÖNCE seal-collision map. Sealed-ordering gerçeği: `sender.rs:2287`
**"FEC-parity DATA/recovery SONRASI (DATA-ÖNCE-sıralaması mühürlü)"** + `:1495` "mühürlü-DATA-önce-sıralama".
Bu, `schedule_transmit` içinde **per-batch precedence**tir: `drain_new_data` → (recovery) → `plan_fec_parity`.

## Harita (family · owner · wire? · ordering? · scheduler-precedence? · resource · security · DUR-B?)

| family | owner/module | wire change | ordering change | sched-precedence | resource | security | **DUR-B?** |
|---|---|---|---|---|---|---|---|
| **A0** | sender ARQ substrate (mevcut) | — | — | — | mevcut | — | **NO** (baseline) |
| **A1-EARLY** | §11 dup machinery (`dup_queue`/`plan_forward_duplication`) — proaktif-reuse | HAYIR (source-kopyası = mevcut-wire; receiver-dedup var) | HAYIR (**dup=DATA, parity-DEĞİL** → DATA-önce-mührü İHLAL-ETMEZ) | drain-içinde (source-sınıfı) | bounded (dup_queue-cap mevcut) | HAYIR | **NO** *(reuse-fizibilite §7.2'de doğrulanır; reuse-olmazsa alternatif-için-ayrı-map)* |
| **F0** | mevcut experimental FEC (`open_fec_block_authority`; block-close) | HAYIR | HAYIR (**mührün-kendisi**: parity@block-close, drain-sonrası) | `plan_fec_parity` (drain-sonrası) | mevcut-bound | HAYIR | **NO** (historical control) |
| **F1** | early **logical**-close block-FEC | HAYIR (aynı parity-shard wire'ı) | **HASSAS** — parity-eligibility fiziksel-tam-dispatch'i beklemez (§7.4-pin) AMA `plan_fec_parity` HÂLÂ per-batch drain-SONRASI çağrılır → **sealed per-batch precedence KORUNUR** (blok mantıksal-erken-kapanır, parity yine drain-sonrası planlanır) | drain-sonrası (korunur) | bounded | HAYIR | **NO** *(design-constraint: intra-batch parity-before-own-block-source TALEP-EDERSE → o-an DUR-B; mevcut-precedence'la uyumlu tasarlanacak)* |
| **F2** | incremental/**interleaved** parity | HAYIR | **EVET** — parity, blok'un-kendi-source-uçuşu-İÇİNDE (sonraki-source'lardan-ÖNCE) araya-girer → **DATA-önce-mührünü İHLAL EDER** | **İHLAL** (parity ⟂ source-arası) | — | — | **YES → DUR-B → NO-CODE → DROPPED THIS TOUR** |
| **F3** | time/deadline partial-block | HAYIR | HAYIR — partial-blok'un **mevcut-source'u tamamlanmış**; parity o-partial-set-üstünde drain-sonrası (timer-trigger yalnız K-fill-yerine-zaman) | drain-sonrası (korunur) | bounded (partial-blok-cap) | HAYIR | **NO** |
| **F4** | adaptive-K block-FEC | HAYIR (K = mevcut `k_actual` header-alanı) | HAYIR (F0-gibi block-close; drain-sonrası) | drain-sonrası (korunur) | bounded | HAYIR | **NO** *(path_timing_snapshot TÜKETEN TEK-arm — §4.6-izinli; future-label-leakage-yasak)* |

## F2 — DUR-B KARARI (§7.5)

> **F2 (incremental/interleaved parity) SEALED DATA-BEFORE-PARITY ORDERING İLE ÇELİŞİR → DUR-B.**
> Interleaved-parity tanım-gereği blok'un-kendi-source-flight'ı-içinde araya-girer; bu, `sender.rs:2287`'de
> mühürlü "FEC-parity DATA-SONRASI" per-batch-precedence'ini İHLAL eder. **NO CODE.** F2 **bu turdan DROPPED**,
> Abkarada-kararına **PENDING**. FS-1 arm-seti 7→6 (A0,A1-EARLY,F0,F1,F3,F4); FS-1 540 (630-değil). 90-koşu-
> tasarrufu başka-faza SESSİZCE-AKTARILMAZ (§7.5). F2'yi açmak sealed-ordering-invariant-değişikliği =
> ayrı-DR + Abkarada-onayı (bu-tur-kapsamı-DIŞI).

## SONUÇ
```text
Bu-tur in-stack aileler: A0, A1-EARLY, F0, F1, F3, F4  (6)   ·  F2 = DUR-B/DROPPED/PENDING
F5 (sliding-window reference) = OUT-OF-STACK harness (v1.0 §arm; in-stack-entegrasyon-wire-gerektirirse ayrı-DUR-B)
Hiçbir aile wire-format-değiştirmez · yalnız F2 ordering-mührünü-ihlal-eder (dropped).
```
