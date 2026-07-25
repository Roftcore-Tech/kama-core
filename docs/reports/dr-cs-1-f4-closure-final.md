# DR-CS-1 F4-CLOSURE — FINAL RAPOR + HÜKÜM

> **ÇERÇEVE (zorunlu-ilk-cümle):** Bu deney ARQ ile FEC'i karşılaştırmaz; ARQ bütün kollarda aktiftir;
> ölçülen şey ARQ üzerine eklenen proaktif FEC'in marjinal faydasıdır. DR-CS-1 bir cold-start
> LIVENESS/bootstrap DR'ıdır — FEC-marjı-ölçümü değil; FEC-utility ayrı raporda (RAPOR-cold-start-ve-fec-utility-butun.md).

**Tarih:** 2026-07-26 · **Branch:** `tur-s-sensing` (d1-sprint5c-çalışması) · **PR:** #44 (DRAFT).
**Feature:** `experimental-cold-start` — **DEFAULT-OFF** (INV-CS-5 flag-OFF byte-özdeş). **Merge/default-flip YASAK.**
**Ürün-alıcı (receiver patience contract):** UYGULANMADI. Hüküm bu gerçeği yansıtır (§15).

---

## 1. KAPSAM + ÖNKOŞULLAR + YASAKLAR (uyum-beyanı)

**Amaç (talimat FC0–FC7):** DR-CS-1 kabul-matrisinin EKSİK F4 hücrelerini tamamla + bilimsel-doğru
closure. **Yeni-mekanizma/tasarım/v8/codepoint YOK.** Tüm eklemeler test-only + cfg-gated + flag-OFF-özdeş.

**Yasaklara uyum (bu-tur):** 0x19/0x1A layout DEĞİŞMEDİ · 0x12=RECOVERED DOKUNULMADI · FEC/DR-1b/P2/fuzzy
DOKUNULMADI · seen_count karar-yoluna SOKULMADI · targeted-retransmit YOK · ACK-refresh/SACK-snapshot YOK ·
production-receiver-patience YOK · default-flip YOK · PR-merge YOK · kırmızı-test-beklentisi-gevşetme YOK.

**Önkoşul (her faithful-koşu):** acceptance-RTT-pin (=100ms) + provenance-blok. Geçmezse → **INVALID RUN —
NO EVIDENCE** (kabul-tablosuna girmez).

---

## 2. METODOLOJİ — FAITHFUL TWO-NETNS WALL-CLOCK HARNESS

Rootless iki-ayrı-network-namespace: `unshare -rnm` (parent A + userns-CAP_NET_ADMIN) → child B
(`unshare -n sleep BIG`, PID-referanslı) → `ip link set v1 netns $CHILD` → cross-namespace veth-forwarding
egress-qdisc'i SADAKATLE uygular (tek-ns-veth netem'i SESSİZCE-yutar; kanıt: prior ping RTT≈0.02ms vs
cross-ns RTT-pin=100ms). Statik-ARP (her-iki-yön permanent) → reverse-blackout-senaryolarında forward-teslimin
ARP'a-bağımlı-olmaması (aksi netem-loss ARP-reply'i-düşürür → forward-de-durur; DI-2B/W-3).

**Provenance (her koşu):** git_commit · dirty · kernel · tc · netns-topology · acceptance_rtt100.

**IP:** tx=10.9.0.1:47001 · rx=10.9.0.2:47002. Forward=parent-v0-egress · Reverse=child-v1-egress.

---

## 3. ⚠ HARNESS-BÜTÜNLÜĞÜ BULGULARI (DÜRÜST — ölçüm-öncesi-düzeltildi)

Bu closure sırasında iki HARNESS-hatası bulundu ve düzeltildi; **her ikisi de wire-kanıtını geçersiz-kılardı**.
Şeffaflık gereği raporlanır (measurement-tool-first disiplini).

### 3.1 BUG-H1 — `$(runrx)` komut-ikamesi rx-ömrü-boyunca BLOKLAR (KRİTİK)
`runrx()` rx'i arka-plana-atarken YALNIZ stderr'i yönlendiriyordu; rx'in **stdout**'u `$(runrx)` komut-ikame
BORUSUNA bağlı kalıyordu → ikame, rx ÇIKANA-KADAR (66s) **bloklamıştı** → tx ancak rx-öldükten-SONRA
başlıyordu → **rx/tx HİÇ örtüşmüyordu → receiver_events=0 (tüm-wire-hücreleri).** İzole-tekrar-üretim:
yönlendirmesiz `$(runrx)` 6s-bloklar, `>/dev/null 2>&1` ile 0s-döner; düzeltme-sonrası receiver_events=9.
**Kök-neden bu; CPU-yükü/netem/binary DEĞİL** (ilk-yanlış-teşhis "load-contamination" düzeltildi). Fix: `a6d199b`.

### 3.2 BUG-H2 — DI-2B küçük-workload tx settle-erken-çıkışı
65B-birimlerde tüm-pencere ilk-cwnd'e-sığar → `workload_done@t0` → tx, cold-start give-up'tan (60s) ÖNCE
settle-çıkıyordu → `cs_terminals=none` artefaktı (300/1200B TimedOut@60s verirken 65B vermiyordu). Fix:
armed-VE-terminal-olmayan cold-start-epizodu = BEKLEYEN-sender-işi → settle-exit bloklanır → give-up-terminal
HER boyutta gözlemlenebilir. **HARNESS-gözlem-kapısı; mekanizma (give-up-timer) DEĞİŞMEZ; flag-OFF byte-özdeş.**
Fix: `33558d9`. Doğrulama: 65/300/1200B üçü de `TimedOut@60000ms` (tutarlı).

---

## 4. HÜCRE ENVANTERİ + DURUM

| Hücre | Tip | Durum |
|---|---|---|
| W-1 (frozen-bound + OFF-vs-ON) | faithful | DATA-PENDING (temiz-rerun; T_W1_BOUND=30s DONDU §11) |
| W-2 (exact-once ACK-drop self-heal) | faithful | DATA-PENDING (ayrı W-2 koşusu; §6) |
| DI-2B (reverse-blackout TimedOut) | faithful | DATA-PENDING (sender-terminal §7; settle-fix-doğrulandı) |
| W-3 (blackout-heal 3-boyut) | faithful | DATA-PENDING (temiz-rerun; §8) |
| DI-10 (rx-caps + O(1)-probe-path) | codec+pure-fn | **TAMAM** (§9; g10-g10d + f4_di10 PASS) |
| Mixed-version (5-satır) | codec | **TAMAM** (§10; g9 PASS) |

---

## 5. W-1 — FROZEN-BOUND + OFF-vs-ON (RTT100 × 65/300/1200B)

**Bound:** T_W1_BOUND = **30_000 ms**, birinci-ilke-türetmesiyle DONDU (reports/d1-exit/T_W1_BOUND-calibration-freeze.md;
temiz-veri-GÖRÜLMEDEN commit). Kabul: `T_ON(block0) < 30s < T_OFF-envelope`.

> **DATA-PENDING (temiz-rerun a6d199b):** aşağıdaki tablo temiz-koşudan doldurulacak.

| size | T_ON block0 (ms) | ON finalized? | T_OFF block0 (90s-ufuk) | ON<30s? | OFF≫ON? | Hüküm |
|---|---|---|---|---|---|---|
| 65 | — | — | — | — | — | — |
| 300 | — | — | — | — | — | — |
| 1200 | — | — | — | — | — | — |

---

## 6. W-2 — EXACT-ONCE ACK-DROP (self-heal; RANDOM-LOSS YASAK)

**Katman-ayrımı (dürüstlük):**
- **DI-2 (in-process, `f2b_di2_single_ack_erasure_spurious_expected`):** tekil-ACK-frame-erasure →
  sender-PTO SPURIOUS-retransmit üretir (BEKLENEN; alıcı-seq-dedup-atar); `bootstrap_induced_retransmit_bytes`
  EXACT=1200 (birim-başına-bir-kez). Yön-teşhisi İDDİA-EDİLMEZ. Bu, sender-dedup-güvenliğinin in-process-pini.
- **W-2 (faithful wire):** İLK-non-boş-SACK exact-once-suppress edilir; **50ms-cadence 2nci-cumulative-SACK
  re-ack'ler** → PTO-öncesi bootstrap → SELF-HEAL. Ölçüm spurious-retransmit'i İDDİA-ETMEZ, ÖLÇER.

> **DATA-PENDING (ayrı W-2 koşusu; run_dr_cs1_w2.sh):** RTT{100,250,1000}×size{65,300,1200}×3-rep.

**12-KRİTER TABLOSU (her satır = RTT×size×rep):**

| # | Kriter | Kaynak |
|---|---|---|
| 1 | acceptance_rtt_pin | provenance |
| 2 | cs_first_sack_dropped=true | rxdone.marker |
| 3 | loss-model = EXACT-ONCE (random-YOK) | provenance scenario |
| 4 | finalized_blocks ≥ 1 (SELF-HEAL) | rxdone.marker |
| 5 | pending_blocks = 0 | rxdone.marker |
| 6 | bootstrap_induced_retransmit_bytes (spurious ölçüm) | cstx |
| 7 | cs_dup_deliveries (alıcı-dedup) | rxdone.marker |
| 8 | app-delivery-once (block-completion-ile) | finalized=1 ⇒ tek-teslim |
| 9 | cs_terminals (ResolvedByDataAck beklenir) | cstx |
| 10 | clean_shutdown=true | rxdone.marker |
| 11 | receiver_events > 0 (overlap-kanıtı; BUG-H1-sonrası) | rxdone.marker |
| 12 | tx-emit budget ≤ nominal (INV-CS budget) | cstx |

| RTT | size | rep | (2)drop | (4)heal | (6)spur-B | (7)dup | (9)terminal | (11)rx-ev |
|---|---|---|---|---|---|---|---|---|
| — | — | — | — | — | — | — | — | — |

---

## 7. DI-2B — SUSTAINED REVERSE-BLACKOUT (sender-terminal)

**Senaryo:** forward-açık (delay 50ms) · reverse-blackout (delay 50ms loss 100%). Probe forward'a-ulaşır,
echo-reverse'te-kaybolur → sender echo-alamaz → give-up@60s TimedOut. **settle-fix (BUG-H2) doğrulandı:**
65/300/1200B üçü de `cs_probe_emitted=4, cs_armed=true, cs_disarmed=true, cs_terminals=TimedOut@60000ms`.

> **DATA-PENDING (temiz-rerun):** sender-terminal tutarlılığı + rx-tarafı probe-reach (BUG-H1-sonrası
> receiver_events>0 ⇒ forward-teslim; reverse-blackout'ta echo-kayıp) doldurulacak.

| size | cs_probe_emitted | terminal | rx receiver_events | rx cs_probe_rx | rx cs_echo_tx | Hüküm |
|---|---|---|---|---|---|---|
| 65 | — | — | — | — | — | — |
| 300 | — | — | — | — | — | — |
| 1200 | — | — | — | — | — | — |

**70s-restore PostTimeoutRecovery varyantı:** in-process F3-D (`f3_*` A/B/C/D) TimedOut@60s → sonraki-ACK =
`PostTimeoutRecovery` (ikinci-terminal-değil) PIN'li (dr_cs1_acceptance.rs). Faithful-70s-restore: DATA-PENDING/opsiyonel.

---

## 8. W-3 — BLACKOUT-HEAL (65/300/1200B; restore 20s/61s)

**Senaryo:** t0..restore çift-yön-blackout → restore'da temizle → heal (blok-completion). restore=20s
(giveup-öncesi → ResolvedByDataAck beklenir) + restore=61s (giveup-sonrası → TimedOut sonra heal =
PostTimeoutRecovery). deficit=1 (first-source-loss katmanı).

> **DATA-PENDING (temiz-rerun):**

| size | restore | terminal | finalized(heal) | T_heal(ms) | receiver_events | Hüküm |
|---|---|---|---|---|---|---|
| 65/300/1200 | 20 | — | — | — | — | — |
| 65/300/1200 | 61 | — | — | — | — | — |

---

## 9. DI-10 — RECEIVER RESOURCE-CAP + O(1) PROBE-PATH ✅ TAMAM

Ürün-alıcı UYGULANMADI; bu kanıtlar DR-CS-1 WIRE-eklerinin (0x19/0x1A) alıcıya UNBOUNDED-durum
DAYATMADIĞINI **production-codec + saf-fonksiyon** seviyesinde gösterir (harness-artefaktı-değil).

| Test | Kanıt |
|---|---|
| `g10_di10_wire_additions_are_noretry` | probe/echo NoRetry → sender retry-kuyruğu-durumu-biriktirmez (O(1)) |
| `g10b_di10_fixed_width_no_amplification` | body ALAN-DEĞERİNDEN-bağımsız SABİT (probe 12B/echo 11B); length-prefix-YOK → wire UNBOUNDED-alloc-İSTEYEMEZ; ekstrem-değer (u32::MAX) → aynı-boyut; sealed 44/43 sabit |
| `g10c_di10_probe_flood_must_not_dedup_unbounded` | ADVERSARIAL: 1024-farklı-probe `InboundControlDedup.admit()`'e → `seen` LİNEER-büyür (unbounded; Sprint-4-TTL-öncesi bilinen-sınır) → probe admit()'ten-GEÇMEMELİ; STATELESS-echo O(1) (working-set sabit) |
| `g10d_di10_echo_idempotent_regeneration` | echo = (probe_seq, ledger)'in SAF-fonksiyonu → byte-özdeş-yeniden-üretim; saklı-durum-GEREKMEZ |
| `f4_di10_receiver_cold_block_cap` | `cold_admit_new_block` saf-fn: cap+1-adversarial (cap-th YENİ-blok reddedilir) + monotonluk + cap==0-sınırsız |

**Bounding-disiplini (SR-AUDIT-001):** harness rx `cold_block_cap>0` → yeni-blok-slot cap'te REDDEDİLİR
(bounds-before-alloc; typed-diag `cs_rxcap_diag`; mevcut-bloklar EVICT-EDİLMEZ → sessiz-eviction-YOK).
**Önerilen cap:** `RX_COLD_BLOCK_CAP=256` (= kama-engine `MAX_PREPARAM_SHARDS` precedent), byte-cap
`256×K(8)×UNIT(1200)=2,457,600 B`.

**Lifecycle/close:** cold-state anahtarları TÜMÜ blok-indeksli (pencere-bounded); per-probe-anahtar-YOK.
Probe→echo yolu SIFIR-map-insert (udp_runner rx: fixed 43B-buf). `close_tombstones` bounded (tombstone_cap);
`seen` unbounded AMA bu DR-CS-1-eklemesi-DEĞİL (pre-existing Sprint-4-TTL-deferral).

---

## 10. MIXED-VERSION 5-SATIR MATRİS ✅ TAMAM

`g9_mixed_version_matrix` (codec-seviyesi; production-yol). Çift-sütun: wire_safety + feature_effectiveness.

| # | Yön | wire_safety | feature_effectiveness |
|---|---|---|---|
| 1 | new-ON → old-rx | eski-parser 0x19/0x1A = UnknownInnerKind → FRAME-drop, session-YAŞAR | feature-etkisiz (eski-rx anlamaz) |
| 2 | new-OFF → old-rx | 0x19/0x1A wire'a-ÇIKMAZ → mevcut-frame'ler byte-özdeş | N/A (OFF) |
| 3 | old → new-rx | eski-frame'ler (RECOVERED 0x12) new-rx'te decode | N/A |
| 4 | new-ON → new-rx | 0x19/0x1A tam-roundtrip + classify-Ok | feature EFFECTIVE |
| 5 | new-OFF → new-rx | mevcut-frame'ler byte-özdeş (RECOVERED-golden korunur) | N/A (OFF) |

0x12=RECOVERED KİLİDİ korundu (g1/g8). Registry {0x01,0x02}∪{0x10..=0x1A} FAIL-CLOSED (g2).

---

## 11. T_W1_BOUND — CALIBRATION + FREEZE RİTÜELİ ✅

Ayrı-artefakt: `reports/d1-exit/T_W1_BOUND-calibration-freeze.md`. Birinci-ilke bileşen-envelope'u →
**T_W1_BOUND = 30_000 ms**, temiz-W-1-verisi GÖRÜLMEDEN commit edildi ("sonuç-görüp-bound-seçme" disiplini).
Prior-gözlem (5.5s@RTT100/1200B) tavana FIT-EDİLMEDİ; 30s ölçümden-bağımsız generous-liveness-envelope.

---

## 12. ÖLÇÜLEN-ŞEY DİSİPLİNİ

wire-emission (INV-CS-1) / enqueue / completion (INV-CS-6) AYRI raporlanır. DI-2B'de `cs_probe_emitted`
(emisyon) ≠ block-completion; TimedOut bir SENDER-terminal'idir, receiver-completion-iddiası-DEĞİL.
"QUIC'i yendik" türü kanıtsız-iddia YOK; tüm sayılar lab-verifiable marker-dosyalarından.

---

## 13. INVARIANT KONTROL (INV-CS-1..7 + flag-OFF)

| INV | İçerik | Kanıt |
|---|---|---|
| INV-CS-5 | flag-OFF byte-özdeş | cold_start_liveness.rs (4-pin) + tüm-eklemeler cfg-gated |
| INV-CS-7 | probe_seq monoton | cstx cs_probe_seq_wrapped=0 |
| INV-CS-1 | wire-emission ayrı-sayaç | cs_probe_emitted/tx_bytes |
| INV-CS-3 | tek-terminal/epoch | take_cs_terminal drain (F3 A/B/C/D pin) |

---

## 14. KALINTI / AÇIK HÜCRELER

- W-1/W-2/DI-2B/W-3 faithful-tablolar: temiz-rerun-verisiyle doldurulacak (BUG-H1/H2-sonrası VALID).
- Ürün-alıcı patience-contract: UYGULANMADI (kapsam-dışı; hüküm §15 yansıtır).
- Faithful-70s-restore PostTimeoutRecovery: opsiyonel (in-process F3-D zaten-pinli).

---

## 15. HÜKÜM

> **DATA-PENDING:** nihai-hüküm temiz-wire-verisi (W-1/W-2/DI-2B/W-3) doldurulunca §talimat-verdict-kurallarına
> göre yazılır. Ürün-receiver-patience UYGULANMADIĞINDAN, izinli-hükümler:
> - Tüm-cell-PASS + ürün-receiver-yok → "SENDER-SIDE COLD-START PROGRESS MECHANISM VALIDATED — FULL DEFINED
>   ACCEPTANCE MATRIX PASSED — PRODUCT RECEIVER PATIENCE CONTRACT UNIMPLEMENTED."
> - Kısmi (W-2/W-3/DI-2B/mixed partial) → "SENDER-SIDE COLD-START BOOTSTRAP CORE VALIDATED — ACCEPTANCE
>   PARTIAL — [EXACT OPEN CELLS] — PRODUCT RECEIVER PATIENCE CONTRACT UNIMPLEMENTED."
> **YASAK-hükümler** (ürün-patience-yokken): "COLD-START PROGRESS FIX FULLY VALIDATED" · "END-TO-END PRODUCT
> FIX VALIDATED" · "PRODUCTION DEFAULT-FLIP READY".

---

## 16. DUR-SON

> **DATA-PENDING:** temiz-rerun + tablolar + hüküm sonrası talimat-mandated DUR-SON formatıyla kapatılır.
