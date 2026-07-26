# TUR A1 — OTURUM BÜTÜN RAPORU

**Roadmap yerleşimi · pre-registration · harness · W-3-65 · iki DUR-B · bulgular · analizler**

> **BAĞLAYICI ÇERÇEVE:** Bu deney ARQ ile FEC'i karşılaştırmaz. ARQ bütün deney kollarında aktiftir; aynı
> ACK/NACK/PTO/retransmit semantiğini kullanır. Ölçülen tek şey, ARQ substratının üzerine eklenen yapının
> marjinal faydasıdır. DR-CS-1 bir **cold-start LIVENESS/bootstrap** araştırma-kararıdır. "ARQ FEC'i yendi /
> FEC ARQ'dan iyi" türü ifadeler YASAKTIR.

**Tarih:** 2026-07-26 · **Branch:** `d1-sprint5c` · **PR:** #44 (DRAFT) · **Commit aralığı:** `a635efa..a8b50e7`
**Üretim-etkisi: SIFIR** (tüm değişiklikler harness/test-only + doc; `crates/*/src` mekanizma-yolu DEĞİŞMEDİ).
**Girdi belgeleri:** resmî roadmap + TUR-A1 talimatı v2 (Abkarada) — repoya yerleştirildi.

---

## 0. YÖNETİCİ ÖZETİ

Bu tur, Abkarada'nın verdiği **roadmap + TUR-A1 talimatını** resmen repoya yerleştirmekle başladı ve roadmap'e
göre **A1'e (kalan faithful cold-start hücreleri)** başladı. İki gerçek pre-registration/gerçeklik çelişkisi
faithful-ölçümle ortaya çıktı; her ikisi de **dürüstçe DUR-B'ye taşındı** (biri Abkarada tarafından karara
bağlandı, biri bekliyor). Bir hücre (**W-3-65**) tam kapandı.

### Durum tablosu
| Kalem | Durum |
|---|---|
| Roadmap + talimat repoya yerleşimi | ✅ `docs/roadmap/` + `docs/tasks/` |
| Pre-registration freeze (orijinal + R2) | ✅ anchor `a635efa` / `fe119b4` |
| F-A1-1 harness (W-2B + W-3-65 knob'ları) | ✅ `9cba9cf` → refactor `baea738` (production-code SIFIR) |
| **DUR-B(1) recovery-path** | ✅ ÇÖZÜLDÜ (Abkarada A3+B1) |
| F-A1-3 H1-desen denetimi | ✅ CLEAN |
| **W-3-65 (B1)** | ✅ **3/3 PASS** |
| **W-2B-R2** | ⛔ BLOKLU (DUR-B(2) bounds-kararı bekliyor) |
| **DUR-B(2) [CAL]-bounds** | 🛑 Abkarada A/B/C kararı BEKLİYOR |

### İki temel bilimsel bulgu
1. **W-2B recovery-path bulgusu:** Full ACK-starvation altında §9-ARQ retransmit **mekanistik ateşleyemez**
   (SACK-reaktif; feedback-bastırılmış). Motor doğal-recovery'yi §11 forward-recovery (ACK-bağımsız) ile yapar.
2. **Full-starvation recovery karakterizasyonu (PİLOT — nihai-değil):** Doğal-recovery **yavaş floor-rate
   byte-tabanlı dispatch** (65B~14s, 300B~61s, 1200B NoRecovery). BtlBw hiç ilklenmez (ACK-gerektirir; cold-start
   yalnız srtt-kurar). **Pilot evidence suggests ON≈OFF under full ACK starvation at RTT100/w8. Confirmatory
   54-run matrix remains pending** (Abkarada-düzeltmesi; genel "FULL-STARVATION'DA COLD-START NÖTRDÜR" hükmü
   tam-matris-bitmeden TAŞINMAZ).

---

## 1. F-A1-0 — BELGE YERLEŞİMİ + PRE-REGISTRATION FREEZE

**Yerleştirilen resmî belgeler** (mojibake-temizlendi; teknik-içerik bire-bir korundu):
- `docs/roadmap/kama-transport-roadmap-status-board.md` — iki-program (cold-start A1→A2→B, FEC C0→D) +
  yaşayan status-board.
- `docs/tasks/dr-cs-1-tur-a1-talimati-v2.md` — bağlayıcı TUR-A1 icra talimatı (W-3-65 + W-2B + H1; F-A1-0..5).

**Pre-registration DONDU** (`reports/d1-exit/DR-CS-1-A1-pre-registration.md`; anchor **`a635efa` @ 16:58:48**,
sha256 `b436803…`): tüm [CAL] (OBSERVATION_HORIZON=120s · RX_TEST_PATIENCE=130s · T_MAX=30s · SUPPRESSED_ACK_CAP=64 ·
RTT-tol |p50−cfg|≤max(5ms,%5) / p95≤cfg+max(20ms,%10)) + beklenen-zincirler + 25-alan-ledger + §5-eşikler,
**herhangi bir koşu YAPILMADAN ÖNCE** commit edildi (§5: sonuca-bakıp-beklenti-seçmek YASAK).

---

## 2. F-A1-1 — HARNESS (test-only knob'lar; PRODUCTION-CODE SIFIR)

`crates/adaptive-transport/src/udp_runner.rs` + example + smoke-test (`9cba9cf`):
- `--no-settle-exit` — tx settle-exit'i TAMAMEN kapat (W-3-65: post-blackout heal 120s-ufka-kadar gözlenir).
- `--suppress-data-acks` — rx TÜM matching DATA-ACK/SACK'i bastır; probe-echo (0x1A) HER ZAMAN geçer;
  **arrival-triggered release** (ilk transport-duplicate DATA arrival'ında, unit-id-eşleşmeli).
- 25-alan W-2B ledger (rx + tx tarafı; **ölçülen** retransmit_path — hardcode-yok) + sayaç-ayrımı
  (transport_duplicate vs application_delivery vs application_duplicate=0).

**Fizibilite kararı (production-code-SIFIR):** 25-alanın TÜMÜ mevcut sender-diag'larından (`diag_cs_bootstrap`,
`take_cs_terminal`, `diag_cold_start`, `next_wakeup_ns`) + harness wire-emit/recv gözleminden türetilebildi →
`sender.rs` diff GEREKMEDİ → DUR-B-yok.

---

## 3. DUR-B(1) — RECOVERY-PATH PRE-REGISTRATION ÇELİŞKİSİ

F-A1-1 izole-doğrulaması iki pre-registered zinciri çürüttü:

### 3.1 W-2B: retransmit_path = ForwardRecoveryDup (ExistingPtoArq DEĞİL)
```
w2b_pto_wire        = none                (§9-ARQ Retransmit HİÇ ateşlemedi)
w2b_dup_wire        = 16195ms#unit0       (§11 Duplicate)
w2b_retransmit_path = ForwardRecoveryDup  ← ÖLÇÜLEN
```
**Mekanistik gerekçe (deterministik):** §9-ARQ retransmit SACK-REAKTİFTİR; full-suppression'da ateşleyemez.
Geriye tek re-send yolu §11 forward-recovery (ACK-bağımsız). Pre-registered `ExistingPtoArq` MEKANİSTİK-İMKÂNSIZ.

### 3.2 W-3-65: heal oluyor ama PostTimeoutRecovery ateşlemiyor
`--no-settle-exit` ile blok HEAL OLDU (finalized=1; F4-UNOBSERVED çözüldü) ama terminal=TimedOut (PostTimeoutRecovery
yok). Gerekçe: 65B ACK-bağımsız forward-recovery ile heal → DATA-ACK-tetikli terminal ateşlemez (300/1200B'de ateşler).

### 3.3 Ortak kök
Küçük-workload/suppressed-feedback rejiminde motor kurtarımı **§11 forward-recovery + RECOVERED-control
(ACK-bağımsız)** ile yapar; pre-registered varsaydığı **§9-ARQ + DATA-ACK** yolunu DEĞİL. §5 gereği
pre-registration tek-taraflı değiştirilemez → **DUR-B** (`b519d31`).

---

## 4. ABKARADA DUR-B(1) KARARI: A3 + B1 (A2/B2 RED)

**W-2B → W-2B-R2** "FULL ACK-STARVATION NATURAL RECOVERY-PATH CHARACTERIZATION AND COLD-START CAUSALITY":
- Eski pre-reg: `REJECTED AS MECHANISTICALLY INAPPLICABLE — SUPERSEDED BY W-2B-R2`.
- ON/OFF matched-kollar (aynı suppression/workload/RTT/arg); recovery_path GÖZLENİR (hardcode-yok; izinli
  sınıflar: ForwardRecoveryDup/ExistingPtoArq/OtherExistingProductionPath/NoRecovery; yeni-path → DUR-B).
- Matris: RTT{100,250,1000}×size{65,300,1200}×feature{ON,OFF}×3 = **54 koşu**. Per-cell verdict:
  DECISIVE/BENEFICIAL/NEUTRAL/NO-RECOVERY. **ON≈OFF → cold-start-lehine-PASS-YAZILMAZ** (natural-self-heal).

**W-3-65 → B1** (forward-recovery KAPATILMAZ): 65B beklenen sınıf terminal=TimedOut +
recovery_observation=RecoveredByForwardRecoveryControl + PostTimeoutRecovery-yok. PostTimeoutRecovery
DATA-ACK-kaynaklı 300/1200B için KORUNUR. A2 açık-madde: **POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT**.

---

## 5. F-A1-3 — H1-DESEN DENETİMİ: CLEAN

14 script `rg` + semantik-inceleme. Tek gerçek-BUG-H1 (`$(runrx)` substitution-block) `a6d199b`'de fixed-SAFE.
Diğer tüm rx-backgrounding **doğrudan** `cmd & VAR=$!` (`$()`-içinde-DEĞİL → ikame-borusu-yok → blok-yok).
ADAY-1 `$(...)` eşleşmeleri `2>&1`-redirect / awk-`&&`. **Hüküm: H1 current-evidence audit CLEAN** (`45e47e7`).

---

## 6. R2 İCRA — ARŞİV + FREEZE + HARNESS REFACTOR

1. **Eski pre-reg arşivlendi** (SUPERSEDED-header; sonuç değişmedi; arşiv-deseni).
2. **W-2B-R2 pre-registration DONDU** — anchor **`fe119b4` @ 18:06:00**, sha256 `570d752…` (koşulardan ÖNCE;
   ON/OFF-matched, 54-koşu, izinli-path, per-cell-verdict, ON≈OFF→PASS-yazılmaz).
3. **Harness refactor** (`baea738`; production-code SIFIR): W-2B suppression + recovery-path ölçümü GENEL'e
   taşındı (`suppress_data_acks` runtime-flag; her-iki-build → OFF-arm ölçülebilir); cold-start-özel-alanlar
   (probe/echo/bootstrap/terminal) ON-only kaldı. `recovery_path` = block-0'ı **TAMAMLAYAN shard'ın wire-kind'ı**
   (completion-mekanizması; 0x00=FirstFlightNoRecovery/0x01=ExistingPtoArq/0x03=ForwardRecoveryDup/none=NoRecovery).
   W-3-65 `recovery_observation` (repair_kind[0]). flag-OFF pin + clippy(-D) yeşil; acceptance 22.

---

## 7. W-3-65 (B1) — SONUÇ: 3/3 PASS

Faithful; 65B×16; restore=61s; --no-settle-exit; HORIZON=120s; RX_PATIENCE=130s; 3 rep; git `1b88336`.
Enstrüman-zinciri: acceptance_rtt100=100 · per-cell-RTT p50=p95=100 (tol içinde) · overlap-gate 3/3 · provenance.

| rep | finalized | pending | terminal | recovery_observation |
|---|---|---|---|---|
| 1 | **1** | 0 | TimedOut@59997ms | **RecoveredByForwardRecoveryControl** |
| 2 | **1** | 0 | TimedOut@60000ms | **RecoveredByForwardRecoveryControl** |
| 3 | **1** | 0 | TimedOut@59999ms | **RecoveredByForwardRecoveryControl** |

**B1 kabul: 3/3** (TimedOut-tek-sefer · finalized=1 · pending=0 · app-correct · app_dup=0 · no-busy-loop ·
no-second-timeout · recovery_observation=beklenen · PostTimeoutRecovery-yok). **W-3-65 (B1) PASS; kesindir**
(bounds-DUR-B'den bağımsız). F4'ün UNOBSERVED'i çözüldü (`a8b50e7`).

---

## 8. YENİ BULGU — FULL-STARVATION DOĞAL-RECOVERY KARAKTERİZASYONU

R2-harness ON/OFF izole-doğrulaması (RTT100, w=8, suppress_data_acks):

| size | completion_time | recovery_path | ON vs OFF |
|---|---|---|---|
| 65B | ~14.1s (ON) / ~14.7s (OFF) | FirstFlightNoRecovery (yavaş-dispatch) | ON≈OFF |
| 300B | ~61.2s | FirstFlightNoRecovery | ON≈OFF |
| 1200B | NoRecovery (90s-ufuk) | NoRecovery | ON≈OFF |

**Mekanizma:** Full-ACK-starvation'da BtlBw hiç-ilklenmez (delivered-rate ACK-gerektirir) → pacing floor-rate'e
düşer → dispatch **yavaş + byte-tabanlı** (65B<300B<1200B). Cold-start-bootstrap YALNIZ srtt-kurar (BtlBw-DEĞİL;
M3-invariant). **PİLOT-HÜKMÜ (nihai-DEĞİL):** *Pilot evidence suggests ON≈OFF under full ACK starvation at
RTT100/w8; confirmatory 54-run matrix pending.* Genel "cold-start-marjinal-faydası-SIFIR" hükmü tam-matris-
bitmeden yazılmaz. Sınır-hipotezi: cold-start'ın W-1'deki-faydası (2.7–5.6s) SOME-ACK-feedback gerektirir —
R3-matriste doğrulanacak. Spurious-dup(§11) cadence-floor'da (~16s/65B); app_dup=0 (dedup temiz).

> **NOT (Abkarada-düzeltme-2):** Bu pilotta `recovery_path=block-0-tamamlayan-shard'ın-wire-kind'ı` tanımı
> AŞIRI-GENELLEŞTİRME idi (FirstFlightNoRecovery bir recovery-yolu-değil). R3-harness'ta alan-AYRIMI (completion_
> trigger_kind + first_recovery_event + recovery_events_before_completion) yapıldı; `recovery_path` yalnız-
> gerçekten-recovery-gözlendiyse. Ayrıca `workload/cwnd-oranı` R3-ledger'da birinci-sınıf (Abkarada-düzeltme-3).

---

## 9. DUR-B(2) — W-2B-R2 [CAL]-BOUNDS ÇELİŞKİSİ (BEKLİYOR)

R2 pre-reg'e devralınan eski-W-2B bound'ları R2'nin full-starvation-karakterizasyonuyla **mekanistik çelişiyor**:
- **CAP=64:** 64×50ms = **~3.2s**'te suppression'ı-bırakır → ACK-akar → BtlBw-ilklenir → hızlı-tamamlanır →
  **full-starvation-DEĞİL** (~3.2s-partial ≈ W-2C). CAP doğal-recovery'yi KESER.
- **T_MAX=30s** < 300B-completion(61s).
- Bu bound'lar çürüyen eski-hipotez (probe→echo→PTO≤13s) için tasarlanmıştı.

**Karar seçenekleri (Abkarada):** **A** (bound'ları R2'de release+FAIL uygulama; boyut-ölçekli observation-horizon
[65B:25s/300B:80s/1200B:45s]; suppressed_ack/completion metrik — ÖNERİ) · **B** (CAP tut → W-2C) · **C** (CAP
saf-güvenlik-bound). §5 gereği tek-taraflı-değiştiremem → DUR-B (`1b88336`). Runtime: 54-koşu ~46 dk (kabul-edilirse arka-plan).

---

## 10. COMMIT HARİTASI (bu tur; `d1-sprint5c` → PR#44)

| Commit | İçerik |
|---|---|
| `a635efa` | F-A1-0: roadmap + talimat repoya + pre-registration FREEZE |
| `9cba9cf` | F-A1-1 harness (W-2B ACK-starvation + W-3-65 knob'ları; test-only) |
| `b519d31` | **DUR-B(1)** recovery-path çelişkisi |
| `45e47e7` | F-A1-3 H1-denetimi CLEAN |
| `fe119b4` | DUR-B(1) kapandı (A3+B1) — eski-pre-reg SUPERSEDED + R2-pre-reg FREEZE |
| `baea738` | R2 harness (ON/OFF matched ledger both-build; recovery_path=completing-shard-kind) |
| `1b88336` | **DUR-B(2)** [CAL]-bounds incompatibility |
| `a8b50e7` | **W-3-65 (B1) 3/3 PASS** |

Push: `2d2c179..a8b50e7 → origin/d1-sprint5c`.

---

## 11. DERSLER

1. **Faithful-ölçüm pre-registration'ı çürütebilir — dürüstçe DUR-B'ye taşı.** Bu tur İKİ gerçek
   pre-reg/gerçeklik çelişkisi ölçümle ortaya çıktı (recovery-path + [CAL]-bounds); ikisi de sonuca-bakıp-
   beklenti-değiştirmek yerine DUR-B'ye taşındı. → ölçüm-disiplini çalışıyor.
2. **Mekanistik-imkânsızlık deterministiktir.** §9-ARQ-SACK-reaktif ⇒ full-suppression'da ateşleyemez —
   boyuttan-bağımsız, tek-ölçümle kanıtlanır (54-koşu-beklemez).
3. **ON/OFF matched-arm cold-start'ın nedensel-marjinal-etkisini izole eder.** Full-starvation'da bu NÖTR
   (BtlBw-yok); W-1'deki-fayda SOME-feedback-koşullu. Sınır-koşulu bilimsel-olarak-değerli.
4. **Devralınan [CAL] otomatik-geçerli-değil.** Eski-hipotez-bound'ları (CAP/T_MAX) yeni-senaryoyla
   çelişebilir; körlemesine-taşımak ölçümü-bozar.
5. **Production-code-SIFIR sınırı korundu.** 25-alan-ledger tümüyle mevcut-diag+harness-gözleminden türedi;
   `sender.rs` diff gerekmedi. Harness-only + flag-OFF-byte-özdeş + cfg/runtime-gated.
6. **Bağımsız-işi bloke-beklerken ilerlet.** H1-denetimi + W-3-65 (bounds-DUR-B'den bağımsız) karar-beklerken
   tamamlandı — [[faithful-netem-harness]] disiplinini koruyarak.
