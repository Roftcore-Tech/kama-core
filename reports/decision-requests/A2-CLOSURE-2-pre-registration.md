# A2-CLOSURE-2 PRE-REGISTRATION v2 — REAL RSS/ALLOCATOR + NETEM [FROZEN]

**Statü:** Koşulardan ÖNCE dondurulur (Abkarada FINAL VALIDATION CLOSURE DIRECTIVE §CLOSURE-2 pre-
registration). Profil-parametreleri, süreler, örnekleme, tekrar, flood-miktarları, RSS-ölçüm-yöntemi,
plateau-eşiği ve cleanup-kontrolleri hash+timestamp ile sabit; **sonuca-göre DEĞİŞTİRİLMEZ.**

**v2 SUPERSESSION NOTU (dürüst):** v1 (sha256=a6eff6f3, commit 0e2d31a) dondurulduktan sonra netem-
matrisi-ÖNCESİ **loopback tool-validation** (ölçüm-teşhis-doğrulama-ayrımı; netem-DEĞİL) yapıldı ve
ARAÇ-tasarım sorunu bulundu: (1) shard-sıralaması esi-önceydi → "distinct block_id round-robin" (§3)
ile uyumsuz, cap-reject-yavaş; (2) 2000 pkt/s'i 64-session'a yaymak 30kbit-linkte per-session-yükü
cap-altına düşürür → throttled-profilde `rejects>0` MEKANİZMA-DEĞİL ölçüm-artefaktı olarak düşerdi.
v2 bunları netem-koşusundan-ÖNCE düzeltir: (a) distinct-block round-robin sıralama; (b) sessions=8
(yoğunlaştırılmış per-session cap-saturation — tüm-profillerde ulaşılabilir); (c) HARD-boundedness ile
saturation-signature AYRIMI açık. Global SESSION_COUNT_CAP=64 boundedness'ı YAPISAL compile-pin +
`a2_f5_plateau_product` (64-session deterministik-plateau; suite'te) kapsar — netem-matrisinde
tekrar-edilmez (throttled-linkte 64-yayılım saturation'ı-dilüe-eder). v1 tarihçede kalır.

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

## 1. NETEM 7-HOSTİLE-PROFİL (exact; rootless `unshare -r -n` — netem tx-egress v0'da)

```text
p1_lowbw       : rate 30kbit
p2_highrtt     : delay 500ms
p3_loss        : loss 30%
p4_jitter      : delay 200ms 100ms distribution normal
p5_burstloss   : loss gemodel 20% 60%                         (Gilbert-Elliott burst)
p6_intermittency: delay 50ms  +  periyodik blackout (loss 100% 2s / clear 3s döngüsü, v1'de v0-egress)
p7_combined    : delay 300ms 80ms distribution normal rate 64kbit loss gemodel 15% 50%
```
Kategoriler Abkarada-listesiyle eşleşir: düşük-bant(p1) · yüksek-RTT(p2) · loss(p3) · jitter(p4) ·
burst-loss(p5) · intermittency(p6) · combined-hostile(p7).

## 2. SÜRELER · ÖRNEKLEME · TEKRAR (freeze)

```text
warm-up          : 1000 ms (session-install + baseline-RSS; flood-ÖNCESİ, örneklenmez)
flood süresi     : 6000 ms / profil / rep
sampling periyodu: 100 ms  (RSS + ledger anlık; loop-içi)
patience-tick    : 100 ms  (on_patience_tick; deadline-evict + idle)
cleanup          : flood-sonu on_patience_tick(now = 300 s) → tüm-deadline-evict (RX_COLD_ABANDON=75s aşılır)
tekrar           : 3 / profil (netem-loss-seed rastgele → varyans; deterministik-ledger-invariant tekrar-değişmez)
```

## 3. FLOOD-MİKTARLARI (rx product-path; tx seal-flood over UDP)

```text
sessions         : 8   (YOĞUNLAŞTIRILMIŞ per-session cap-saturation; throttled-profilde-bile ulaşılır)
per-session blok : BLOCKS_PER_SESSION=16 (>OPEN_BLOCK_CAP=8) DISTINCT-BLOCK round-robin → 9. blok-reject
per-block shard  : ESI_PER_BLOCK=7 = K-1 (INCOMPLETE — decode-etmez → retained-birikir → plateau)
probe flood      : sürekli (her 3. iter; F-A: cold-ledger-DELTA=0 beklentisi — retained pure-shard)
control flood    : sürekli (her 5. iter; SessionClose monoton control-id; F-C: dedup-bitmap O(1))
tx pacing        : ~30ms/iter (~ per-iter 8 source + periyodik probe/control; netem-altında düşer)
NOT: global SESSION_COUNT_CAP=64 boundedness YAPISAL-pin + a2_f5 (netem-matrisinde-değil).
```
NOT: tx/rx eşleşen simetrik-anahtar (CLOSURE-2 KAYNAK-ölçümü; key-semantiği CLOSURE-1B'de). Shard'lar
AEAD-open-EDER (aksi cold-ledger'a-ulaşmaz) → gerçek cold-admission yolu ölçülür.

## 4. RSS/ALLOCATOR ÖLÇÜM-YÖNTEMİ (freeze)

```text
RSS       : /proc/self/statm alan-2 (resident pages) × page_size(4096)  → bytes  (runner-içi; dosya-oku)
baseline  : ReceiverHost + 8-session kurulu, flood-ÖNCESİ (retained=0 anı)
peak      : flood-boyunca max(RSS)
plateau   : flood son-1s örneklerinin ledger-retained SABİTLİĞİ (büyüme-durdu)
post-clean: cleanup(evict-all)-sonrası RSS + ledger
allocator-arena: RSS-post-cleanup − baseline (OS'ye-dönmeyen-arena PROXY'si; leak-DEĞİL — ayrı-raporlanır)
live-object: ledger retained_bytes + session_count + tombstone_count (deterministik) + dedup Box<[u64;64]> yapısal
```

## 5. PLATEAU KABUL-EŞİĞİ (freeze)

```text
plateau_slope := (retained[son] − retained[son−1s]) / max(retained[son],1)   (son-1s penceresi)
SATURATION-reached ⇔ rejects>0 (cap-doyduğunun operasyonel-tanımı).
KABUL (SATURATION-reached profil): |plateau_slope| ≤ 0.05 (cap-doygunluğu sonrası büyüme ≈ 0).
ledger-retained MONOTON-BOUNDED: her-t'de retained ≤ GLOBAL_COLD_BYTE_CAP (15 MiB) — HARD (her-profil).
```

## 6. HER-PROFİL PASS-KRİTERİ (Abkarada §netem-matrisi)

```text
HARD (her profil — MUTLAK; ihlal → profil FAIL):
  (a) ledger cap AŞILMAZ         : max(retained) ≤ 15 MiB (global) ∧ ≤ 480KiB×active-session
  (d) tombstone ≤ 64             : tombstone_count ≤ TOMBSTONE_REGISTRY_CAP
  (e) dedup-bitmap SABİT         : control-flood working-set büyüme=0 (Box<[u64;64]> yapısal-sabit)
  (f) probe cold-ledger-büyütmez : retained pure-shard (probe cs_echo AYRI; cold-delta=0)
  (g) cleanup-sonrası baseline   : post-clean ledger-retained == 0 (live-alloc başlangıca-döner)

SATURATION-SIGNATURE (cap-saturation ULAŞILAN profilde ZORUNLU; ulaşılmayan/channel-limited → RAPOR):
  (b) typed rejection GÖRÜNÜR    : cold-cap-reject-sayacı > 0
  (c) live retained PLATEAU      : plateau_slope ≤ 0.05 (§5)
Channel-limited (rejects=0): retained « cap → boundedness DAHA-GÜÇLÜ; (b)/(c) N/A-RAPOR (FAIL-DEĞİL).
```

## 7. CLEANUP KONTROLÜ (başlangıç + son)

```text
başlangıç: baseline-RSS + retained==0 (flood-öncesi) KAYDEDİLİR.
son      : on_patience_tick(300s) → evict-all → retained==0 ASSERT (g).
           post-clean RSS − baseline = allocator-arena-proxy (RAPOR; leak-DEĞİL).
```

## 8. HÜKÜM FORMATI

```text
REAL RSS/ALLOCATOR + NETEM: PASS / FAIL
PASS ⇔ 7-profilin HEPSİNDE HARD-invariant (a,d,e,f,g) sağlanır ∧ saturation-reached-profillerde (b,c)
       sağlanır ∧ channel-limited-profiller dürüst-raporlanır (boundedness-güçlü).
FAIL ⇔ herhangi-profilde HARD-invariant ihlali (retained>cap / cleanup≠0 / tombstone>64 / dedup-büyür).
Netem koşulamazsa (capability-blocker) → TYPED-BLOCKER + kısmi-hüküm (sahte-sonuç-üretilmez).
```

## 9. YASAK (§uyum)

```text
1200B-decisive / liveness-solved / recovery-impossible / FEC-ARQ-hükmü YASAK. ARQ-vs-FEC-yok.
TOTAL-PROCESS-RSS ≤16MiB-iddiası YASAK (§0). A1 SEALED. Sonuca-göre-parametre-değişikliği YASAK.
```

## PROVENANCE
```text
sha256: d7b9aea08efd04592a9c577532587f106b7230b6746ddd7d3815e5c137bf684f
tarih : 2026-07-30T20:18:11Z
supersedes: v1 sha256=a6eff6f3dd5652df8d9846074ce18557bac76c8c296f5e3f4b0a45c56bee5e70 (commit 0e2d31a)
```
