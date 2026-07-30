# A2-CLOSURE-2 PRE-REGISTRATION — REAL RSS/ALLOCATOR + NETEM [FROZEN]

**Statü:** Koşulardan ÖNCE dondurulur (Abkarada FINAL VALIDATION CLOSURE DIRECTIVE §CLOSURE-2 pre-
registration). Profil-parametreleri, süreler, örnekleme, tekrar, flood-miktarları, RSS-ölçüm-yöntemi,
plateau-eşiği ve cleanup-kontrolleri hash+timestamp ile sabit; **sonuca-göre DEĞİŞTİRİLMEZ.**
Bu doküman CLOSURE-1B kararından bağımsızdır (Abkarada: "Closure-2 şimdi başlasın").

## 0. ÖLÇÜM FELSEFESİ (bağlayıcı — Abkarada)

```text
TOPLAM PROCESS RSS ≤ 16 MiB iddiası KURULMAZ (16 MiB tüm-process-bütçesi DEĞİL).
Ölçülen: A2 incremental retained-state + allocator-aware retained + bounded side-state + zaman-plateau.
BAĞLAYICI BOUND (ledger-model; yapısal + ampirik doğrulama):
  A2 cold retained ledger  ≤ 15 MiB   (GLOBAL_COLD_BYTE_CAP)
  A2 side-state            ≤  1 MiB   (dedup-bitmap + issue-gate + tombstone-registry)
  A2 bounded retained-state ≤ 16 MiB  (toplam)
RSS: peak / retained-plateau / post-cleanup AYRI raporlanır — ≤16MiB-assert-EDİLMEZ.
Cleanup-sonrası RSS'nin OS'ye hemen-dönmemesi leak-DEĞİL; ledger + live-alloc + allocator-arena AYRI.
```

## 1. NETEM 7-HOSTİLE-PROFİL (exact; rootless `unshare -r -n` — netem tx-egress'te)

```text
p1_lowbw       : rate 30kbit
p2_highrtt     : delay 500ms
p3_loss        : loss 30%
p4_jitter      : delay 200ms 100ms distribution normal
p5_burstloss   : loss gemodel 20% 60%                         (Gilbert-Elliott burst)
p6_intermittency: delay 50ms  +  periyodik blackout (loss 100% 2s / clear 3s döngüsü)
p7_combined    : delay 300ms 80ms distribution normal rate 64kbit loss gemodel 15% 50%
```
Kategoriler Abkarada-listesiyle eşleşir: düşük-bant(p1) · yüksek-RTT(p2) · loss(p3) · jitter(p4) ·
burst-loss(p5) · intermittency(p6) · combined-hostile(p7).

## 2. SÜRELER · ÖRNEKLEME · TEKRAR (freeze)

```text
warm-up          : 1000 ms (session-install + baseline-RSS; flood-ÖNCESİ, örneklenmez)
flood süresi     : 5000 ms / profil / rep
sampling periyodu: 100 ms  (RSS + ledger anlık; loop-içi)
patience-tick    : 100 ms  (on_patience_tick; deadline-evict + idle)
cleanup          : flood-sonu on_patience_tick(now = start + 200 s) → tüm-deadline-evict (RX_COLD_ABANDON=75s aşılır)
tekrar           : 3 / profil (netem-loss-seed rastgele → varyans; deterministik-ledger-invariant tekrar-değişmez)
```

## 3. FLOOD-MİKTARLARI (rx product-path; tx seal-flood over UDP)

```text
sessions         : 64  (SESSION_COUNT_CAP — global-cap-stresi)
per-session blok : OPEN_BLOCK_CAP-üstü hedeflenir (distinct block_id round-robin; cap-reject görünür)
per-block shard  : K-1 = 7 (INCOMPLETE — decode-etmez → retained-birikir → plateau)
probe flood      : sürekli (round-robin; F-A: cold-ledger-DELTA=0 beklentisi)
control flood    : sürekli (SessionClose monoton+geri control-id; F-C: dedup-bitmap O(1))
tx pacing        : ~2000 pkt/s nominal (netem-altında düşer; flood-yeterli)
```
NOT: tx/rx eşleşen simetrik-anahtar (CLOSURE-2 KAYNAK-ölçümü; key-semantiği CLOSURE-1B'de). Shard'lar
AEAD-open-EDER (aksi cold-ledger'a-ulaşmaz) → gerçek cold-admission yolu ölçülür.

## 4. RSS/ALLOCATOR ÖLÇÜM-YÖNTEMİ (freeze)

```text
RSS       : /proc/self/statm alan-2 (resident pages) × page_size(4096)  → bytes  (runner-içi; syscall-yok, dosya-oku)
baseline  : ReceiverHost + 64-session kurulu, flood-ÖNCESİ (retained=0 anı)
peak      : flood-boyunca max(RSS)
plateau   : flood son-1s örneklerinin ledger-retained SABİTLİĞİ (büyüme-durdu)
post-clean: cleanup(evict-all)-sonrası RSS + ledger
allocator-arena: RSS-post-cleanup − baseline (OS'ye-dönmeyen-arena PROXY'si; leak-DEĞİL — ayrı-raporlanır)
live-object: ledger retained_bytes + session_count + tombstone_count + dedup-word-sayısı (deterministik)
```

## 5. PLATEAU KABUL-EŞİĞİ (freeze)

```text
plateau_slope := (retained[son] − retained[son−1s]) / retained[son]  (son-1s penceresi)
KABUL: |plateau_slope| ≤ 0.02  (cap-doygunluğu sonrası büyüme ≈ 0)  VEYA retained cap-altında-SABİT.
ledger-retained MONOTON-BOUNDED: her-t'de retained ≤ GLOBAL_COLD_BYTE_CAP (15 MiB) — HARD.
typed-reject: cap-aşımında Dropped("cold cap exceeded") sayacı > 0 (sessiz-eviction-YOK).
```

## 6. HER-PROFİL INVARIANT (Abkarada §netem-matrisi; PASS-koşulu)

```text
(a) ledger cap AŞILMAZ            : max(retained) ≤ 15 MiB (global) ∧ ≤ 480KiB×active-session
(b) typed rejection GÖRÜNÜR       : cold-cap-reject-sayacı > 0  (incomplete-flood cap-doyurur)
(c) live retained-object PLATEAU  : plateau_slope ≤ 0.02 (§5)
(d) tombstone ≤ 64                : tombstone_count ≤ TOMBSTONE_REGISTRY_CAP
(e) dedup-bitmap SABİT            : control-flood working-set büyüme=0 (O(1); word-sayısı = 64 sabit)
(f) probe-flood cold-ledger-büyütmez: probe-only-delta retained=0 (F-A ayrık-kontrol)
(g) cleanup-sonrası baseline      : post-clean ledger-retained == 0 (live-alloc başlangıca-döner)
```

## 7. CLEANUP KONTROLÜ (başlangıç + son)

```text
başlangıç: baseline-RSS + retained==0 (flood-öncesi) KAYDEDİLİR.
son      : on_patience_tick(start+200s) → evict-all → retained==0 ASSERT (g).
           post-clean RSS − baseline = allocator-arena-proxy (RAPOR; leak-DEĞİL).
```

## 8. HÜKÜM FORMATI

```text
REAL RSS/ALLOCATOR + NETEM: PASS / FAIL
PASS ⇔ 7-profilin HEPSİNDE (a)-(g) sağlanır (ledger-invariant HARD; RSS-plateau raporlanır).
Netem koşulamazsa (capability-blocker) → TYPED-BLOCKER + kısmi-hüküm (sahte-sonuç-üretilmez).
```

## 9. YASAK (§uyum)

```text
1200B-decisive / liveness-solved / recovery-impossible / FEC-ARQ-hükmü YASAK. ARQ-vs-FEC-yok.
TOTAL-PROCESS-RSS ≤16MiB-iddiası YASAK (§0). A1 SEALED. Sonuca-göre-parametre-değişikliği YASAK.
```

## PROVENANCE
```text
sha256: a6eff6f3dd5652df8d9846074ce18557bac76c8c296f5e3f4b0a45c56bee5e70
tarih : 2026-07-30T20:09:30Z
```
