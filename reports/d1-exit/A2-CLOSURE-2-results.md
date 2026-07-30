# A2-CLOSURE-2 RESULTS — REAL RSS/ALLOCATOR + NETEM

**Statü:** DR-CS-1b/A2 FINAL VALIDATION CLOSURE-2 koşu-sonucu. Pre-registration v2 (sha256=d7b9aea0,
commit 2ff1bd9) parametreleriyle; sonuç-sonrası-parametre-değişikliği YOK. Ham veri:
`reports/d1-exit/data/netem-a2/` (summary.csv 21-satır · ENV.txt · RUNLOG). Runner:
`crates/kama-integration/examples/a2_rx_netem_runner.rs` (gerçek `ReceiverHost` product-path +
`/proc/self/statm` RSS). Harness: `scripts/netem/run_a2_closure2.sh` (rootless two-netns).

## 0. HÜKÜM

> ## **REAL RSS/ALLOCATOR + NETEM: PASS**
> 7-hostile-profilin HEPSİNDE HARD-boundedness invariant'ları (a,d,e,f,g) sağlandı (21/21 koşu);
> typed cap-reject (b) evrensel; retained-BYTE faithful-netem-altında cap'in **%0.21'i** (channel-
> limited → boundedness-plateau'dan-GÜÇLÜ). RSS bounded + ledger'dan ayrı-raporlandı. HARD-ihlal YOK.

## 1. ÖLÇÜM-GEÇERLİLİK (faithful-netem; ölçüm-teşhis-doğrulama-ayrımı)

**İKİ ölçüm-aracı-hatası koşudan-ÖNCE yakalandı** (measurement-tool-first; sahte-sonuç-üretilmedi):

1. **Loopback tool-validation** (netem-DIŞI): esi-önce-sıralama + 64-session-dilüsyon → pre-reg v1→v2
   düzeltmesi (distinct-block round-robin + sessions=8). Bkz. v2 supersession-notu.
2. **FAITHFUL-NETEM catch (KRİTİK):** İLK-matris tek-netns-veth ile koştu → netem **SESSİZCE-YOK-
   SAYILDI** (`delay 200ms` ping-RTT 0.021ms; faithful-netem-harness dersi doğrulandı). O koşu
   netem-inert (loopback-eşdeğer; tüm-profil özdeş max_retained=150528) → **ATILDI**. Harness
   **two-netns**'e (veth-peer → child-PID) taşındı; ping-doğrulama: `delay 200ms → RTT 200.27ms`,
   `loss 100% → %100 drop`. Bu-rapordaki-veri YALNIZ faithful-two-netns koşusundandır.

ENV faithful-check (her-koşu-başı, ham ENV.txt): `delay 200ms → 200.265/300.346/400.427 ms (pipe 2)`.
Sonuçların netem-inert-koşudan KESKİN-farkı (150528 → 21504–33600) netem'in artık ısırdığının kanıtı.

## 2. PER-PROFİL SONUÇ (7 profil × 3 tekrar = 21 koşu)

| profil | netem | max_retained (B) | rejects [min–max] | postclean_retained | peak_rss (B) |
|---|---|---|---|---|---|
| p1_lowbw | rate 30kbit | 24192 | 64–64 | 0 (hepsi) | 2547712 |
| p2_highrtt | delay 500ms | 31248 | 58–64 | 0 | 2551808 |
| p3_loss | loss 30% | 26208 | 66–82 | 0 | 2531328 |
| p4_jitter | delay 200ms 100ms normal | 25200 | 60–75 | 0 | 2547712 |
| p5_burstloss | loss gemodel 20% 60% | 25872 | 64–64 | 0 | 2551808 |
| p6_intermittency | delay 50ms + blackout(2s/3s) | 31584 | 62–64 | 0 | 2613248 |
| p7_combined | delay 300ms±80ms + 64kbit + GE 15/50 | 33600 | 52–58 | 0 | 2551808 |

**Sabitler:** peak_sessions=8, peak_tombstones=0, delivered=0 (hepsi; INCOMPLETE-blok teslim-etmez).
baseline_rss ≈ 2.43 MiB; peak_rss ≈ 2.52–2.61 MiB.

## 3. HARD-BOUNDEDNESS INVARIANT (v2 §6; her-profil MUTLAK; 21/21 koşu)

```text
(a) retained ≤ 15 MiB cap : max_observed = 33600 B = %0.21 (cap) / %22.3 (loopback-doygunluk-tavanı) → PASS
(d) tombstone ≤ 64        : 0 (hepsi) → PASS
(e) dedup-bitmap sabit    : Box<[u64; 64]> yapısal-sabit (512 B/session); control-flood O(1) → PASS
(f) probe cold-ledger-büyütmez : retained pure-shard (≤33600); cs_echo (0–48) AYRI eksen → PASS
(g) cleanup → baseline    : post_clean_retained == 0 (21/21) → PASS
```
Otomatik-denetim (summary.csv): `retained>15MiB=0 · postclean!=0=0 · tombstone>64=0 · rejects>0=21/21`.

## 4. TYPED CAP-REJECT + İKİ-BOYUTLU-DOYGUNLUK BULGUSU

**(b) typed rejection GÖRÜNÜR: 21/21 koşuda rejects ∈ [52,82] > 0.** OPEN_BLOCK_CAP=8 aşımında
`Dropped("cold cap exceeded")` — sessiz-eviction YOK, her hostile-profilde.

**BULGU (v2 §5 op-def netleştirmesi — dürüst):** "cap-doygunluğu" İKİ-AYRI-eksen:
```text
BLOCK-COUNT ekseni : OPEN_BLOCK_CAP=8 → 9. distinct-blok reject. HER-profilde ULAŞILDI (rejects>0).
                     Cap-mekanizması (typed-reject) faithful-hostile-netem-altında ÇALIŞIR.
RETAINED-BYTE ekseni: 8-blok × 7-esi × 336 = 150528 B doygunluk-tavanı. faithful-netem-altında
                     ULAŞILMADI — retained 21–34 KB (%0.21 cap). Hostile-link flood'u THROTTLE eder
                     → adversary retained'i cap'e YAKLAŞTIRAMAZ (channel-limited).
```
v2 §5 "saturation-reached ⇔ rejects>0" op-def bu-iki-ekseni-birleştirmişti; rejects=block-count-cap,
retained«cap=byte-boundedness. plateau_slope (0.099–0.264) retained-BYTE'ın yavaş-throttled-fill'ini
yansıtır (cap-DEĞİL; 33600«15MiB). Bu v2 §6 **channel-limited branch**'idir (retained«cap →
boundedness-DAHA-GÜÇLÜ; byte-plateau-at-cap N/A — throttled-link'in kendisi adversary'yi bound eder).
**FAIL-belirleyici HARD-invariant'lar (a,d,e,f,g) sağlandığından hüküm PASS.**

## 5. RSS / ALLOCATOR (ayrı-raporlama; ≤16MiB-iddiası KURULMAZ — v2 §0)

```text
baseline_rss        ≈ 2.43 MiB  (ReceiverHost + 8-session; retained=0)
peak_rss            ≈ 2.52–2.61 MiB
post_clean_rss      ≈ peak_rss  (ledger-retained=0 olsa da RSS DÜŞMEZ)
allocator-arena-proxy = post_clean_rss − baseline ≈ 115 KB (bounded; profiller-arası SABİT ~2.55MiB)
```
**post_clean_retained=0 (ledger baseline) ↔ post_clean_rss≈2.55MiB (allocator-arena):** ledger canlı-
kaynağı tam-iade-eder; glibc-malloc serbest-sayfayı OS'ye-hemen-vermez = **LEAK-DEĞİL** (v2 §0). İki-
büyüklük AYRI: boundedness-kanıtı ledger-retained→0'dır; RSS-arena bağlam. RSS profiller-arası
büyümez (peak ~2.55MiB sabit) → RSS-side de bounded. TOPLAM-PROCESS-RSS ≤16MiB-iddiası KURULMADI.

## 6. BAĞLAYICI BOUND (v2 §0) — DOĞRULAMA

```text
A2 cold retained ledger ≤ 15 MiB : max 33600 B « 15 MiB (yapısal-cap + ampirik %0.21) ✓
A2 side-state          ≤  1 MiB  : 8×(dedup 512B + issue-gate + tombstone) « 1 MiB (yapısal) ✓
A2 bounded retained    ≤ 16 MiB  : ≤ 15 + side « 16 MiB ✓
global SESSION_COUNT_CAP=64      : YAPISAL compile-pin + a2_f5_plateau_product (netem-dışı; suite) ✓
```

## 7. KORUNAN SINIRLAR
A1 SEALED. 1200B-yasak-iddialar. ARQ-vs-FEC-karşılaştırma-YOK. merge/default-flip-YOK. [CAL]-SEALED
gevşetilmedi. `experimental-cold-start` DEFAULT-OFF (runner example-only; lib byte-özdeş, INV-CS-5).

## 8. PROVENANCE
```text
pre-registration : reports/decision-requests/A2-CLOSURE-2-pre-registration.md v2 sha256=d7b9aea0
raw data         : reports/d1-exit/data/netem-a2/{summary.csv, ENV.txt, RUNLOG}
runner           : crates/kama-integration/examples/a2_rx_netem_runner.rs
harness          : scripts/netem/run_a2_closure2.sh (rootless two-netns; faithful-netem ping-verified)
faithful-check   : delay 200ms → RTT 200.27ms · loss 100% → %100 drop  (ENV.txt)
```
