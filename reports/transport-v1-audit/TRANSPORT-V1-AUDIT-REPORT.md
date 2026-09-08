# TRANSPORT v1 KAPANIŞ DENETİMİ — KARAR RAPORU

**Tarih:** 2026-09-08 · **Dal:** `audit/transport-v1-closure` (main `75d8990a` + FEC kapanışı + denetim düzeltmeleri)
**Yöntem:** 33 ajanlık iki yönlü denetim (12 küme SPEC→CODE + 12 adversaryal doğrulama + 8 modül CODE→SPEC +
bayrak sicili) + kendi grep doğrulamalarım + netem aktivasyon koşuları.
**Ham veri:** `phase1-raw/` · **Siciller:** `COVERAGE-MATRIX.md`, `DEFAULT-OFF-REGISTER.md`, `UNWIRED-REGISTER.md`
· **Profil:** `PRODUCT-PROFILE-v1.md` · **Kanıtlar:** `CONFIRMED-FINDINGS.md`, `ACTIVATION-PROOF.md`

---

## 1. YÖNETİCİ HÜKMÜ

> **TRANSPORT V1 BLOKE — 4 blocker.** Üçü bu oturumda düzeltildi; kalan blocker sınıfı ölçüm-öncesi
> mimari eksikliktir. Kıyas kampanyası (T0–T7) **başlatılmadı**: benchmark durdurma kuralı gereği
> açıklanmamış WIRE kalemi varken koşulmaz.

Ve en önemli düzeltilmiş bulgu:

> **Önceki "Kama, Quinn'e kaybediyor" sonucu GEÇERSİZDİR.** Ölçülen kol ürünün sevk etmediği bir
> yapılandırmaydı. Ürünün kendi default'uyla aynı hücrede Kama burst'te Quinn'i **yeniyor**
> (0.995 vs 0.895), iid'de berabere (0.990 vs 0.995) — üstelik FEC hâlâ kapalıyken.

---

## 2. MİMARİ KAPSAMA (özet)

| ölçü | değer |
|---|---|
| SPEC→CODE mekanizması (doğrulama sonrası) | **444** |
| SHIP | 243 |
| WIRE — açık | **93** (spec-"required" olan **62**) |
| WIRE — bu oturumda kapatılan | 20 |
| KEEP-EXPERIMENTAL | 64 |
| DELETE | 27 |
| CODE→SPEC taranan sembol/knob | **839** (94 DELETE · 54 hiçbir yerden çağrılmıyor · 143 test-only · 100 bench-only · 57 feature-gated · 31 diagnostic-only) |
| Adversaryal doğrulama | 292 teyit · 71 düzeltme · **29 çürütme** · 3 doğrulanamaz · 67 sınıf düzeltmesi · 72 atlanmış mekanizma |

Açık WIRE'ın tema dağılımı: **A. dönüş-yolu/asimetri**, **B. FEC tamamlanmamış parçalar**,
**C. telemetri borcu**, **D. kıyas metodolojisi**, **E. post-v1 kripto (S2)**, **F. pacing/CC**.

---

## 3. BLOCKER'LAR

### B-1 ✅ DÜZELTİLDİ — Ürün profilinde FEC yoktu (`fec: None`)

`RuntimeConfig::default().fec = None` → FEC alt-sistemi ürün yolunda hiç kurulmuyordu. Spekülatif kol
(`bounded_v1`) açık olmasına rağmen yönettiği alt-sistem yoktu: oran hesaplanıyor, aktüasyon olmuyordu
(*configuration zombie*). Sevk kapısı **S1-GATE 2026-08-26'da geçilmişti** (NO-HARM PASS; burst h3b
FEC %100 vs ARQ-only %99) — eksik olan kanıt değil, kararın default'a uygulanmasıydı.
**Sınıf 5 (gerekli özellik default OFF).** → `c9664187`: `KamaV1Profile` + `fec: Some(0x05, adaptif)`.

### B-5 ✅ DÜZELTİLDİ — Benchmark ürün profilini koşmuyordu

Bench `RuntimeConfig`'i elle kuruyordu; ürün default'ları (IW 2400, `bounded_v1`, gate 250) benchmark'a
hiç ulaşmıyordu. "kp-sevk" etiketli kol IW=0 (ürünün *clamp hatası* diye düzelttiği davranış), risk kolu
kapalı, gate 500 ile koşuyordu. **Sınıf F/G (test tasarımı / semantik uyumsuzluk).**
→ `c9664187`: bench profilden başlar + **zorunlu mimari-aktif manifest** (`KP-MANIFEST` profil kimliği ve
v1'den sapma listesi, `KP-ACTIVATED` fiilen ateşleyenler). Manifest ilk koşusunda kendi işini gördü:
bench hâlâ FEC'i kapatıyordu, sapma satırı bunu yakaladı.

### B-7 ✅ DÜZELTİLDİ — KURULUŞ-GUARD spekülatif kolu kapsamıyordu

`structure_gate_fec` PreConverge'de `BlockedPreConverge` döndürüyor ama `speculative_propose` hiçbir
`SenderState` kontrolü içermiyordu. FEC default açıldığı anda bu canlı bir yol oldu: Karn-dışı bir ACK
RTT örneği vermeden kayıp ilan edebilir → kuruluş fazında parity, kıt token bütçesinde orijinallerle
yarışır. **Sınıf D (mekanizmalar birlikte yanlış çalışıyor).** → `b07d8286`: guard arbiter çıkışına
taşındı (üç kolu kapsar) + `preconv_parity_blocked` sayacı. Aktivasyon koşusunda 0 → açık **latent**ti,
ölçümleri etkilememişti.

### B-8 ⛔ AÇIK — Dönüş yolu ölçülmüyor; asimetri/bozulmuş-dönüş rejimi mimari olarak eksik

Bu, hedef zarfın **birincil** iki rejimini (T5 asimetrik yol, T6 bozulmuş dönüş yolu) doğrudan vuran
tek blocker. Spec-normatif, kod tarafı yok:

| eksik | spec | durum |
|---|---|---|
| Ölçülen `return_bw` (31 B/s prior yerine) | engine §3.3-E FULL, §12 | tüm ağaçta sıfır kod; prior kanaldan bağımsız → confirm ufku ölü zamanı |
| ACK decimation (dönüş bozulunca ACK_FREQ↑ / ACK bütçesi↓) | §2.1/§11/§12, TDD §5.6 | `ACK_FREQ_N=2` sabit; `emergency::ack_decimation` helper'ı hiç tüketilmiyor |
| `ACK_FRAME_V2.seen_count` tüketicisi | §2.2 (K3 yön-ayrımı) | her ACK'te 6 B taşınıyor, `sender.rs:107/3217` "SENDER'A-TAŞINMAZ" |
| `estimator_confidence` / `ack_compression_score` / `reverse_path_impaired` | §7.3 | sıfır implementasyon |
| PATH_DEAD / REVERSE_PATH / CHANNEL_LOSS ayrımı | TDD §9, §12 R4 | yok |

Sonuç: ürünün **ileri-yön kaybı ile dönüş-yolu kaybını ayırt edecek hiçbir sinyali yok**. Spec bu işin
gövdesini ADIM-6'ya erteliyor; dolayısıyla iki seçenek var ve **karar owner'ındır**: (a) T5/T6 v1
kapsamından AÇIKÇA çıkarılır ve hostile-dominance iddiası bu iki rejim için yapılmaz; (b) ADIM-6'nın
asgari parçası (ölçülen `return_bw` + ACK decimation) v1'e alınır. Denetim (a) veya (b) olmadan
"v1 READY" diyemez — çünkü v1 kriteri "mimari-gerekli mekanizmalar uygulanmış **ya da açıkça v1 dışı
ilan edilmiş** olmalı"dır.

---

## 4. KALAN AÇIK WIRE — tema özeti (93 kalem, ayrıntı `COVERAGE-MATRIX.md`)

| tema | kalem | v1 etkisi |
|---|---:|---|
| A. Dönüş-yolu / asimetri | ~14 | **B-8 blocker** |
| B. FEC tamamlanmamış parçalar | ~18 | kısmi/son blok kodlanmıyor · §8.5 bant-türevli parity clamp'i bağlı değil · kayıp-parity ARQ'ya DATA olarak giriyor (ESI namespace) · emergency MinimalParity spec'in tersini yapıyor |
| C. Telemetri/gözlem borcu | ~9 | tail-probe ateşlemesi ürün derlemesinde sayaçsız; bw/srtt/pacing ürün istatistiklerine export edilmiyor → "aktive oldu" kanıtı üretilemiyor |
| D. Kıyas metodolojisi | ~23 | setup-cost muhasebesi, servis-semantiği normalizasyon tablosu, blackout-survival ikili metriği, operasyonel taban, N≥30/CI/provenans — **T0–T7 öncesi kapatılmalı** |
| E. Post-v1 (S2/kripto) | ~8 | `IdentitySeam` (kripto yok), `ack_epoch`, ikinci ACK kodek ailesi, `admit_and_decode` köprüsü — Sprint-6 kapsamı, v1 blocker DEĞİL ama v1'in "güvenli transport" diye adlandırılmasını engeller |
| F. Pacing/CC + diğer | ~21 | `inflight_bytes ≤ cwnd` bayt kapısı yok, ProbeBW down-leg yok, `reordering_score` yok, `arq_aggressiveness` fiilen sabit |

---

## 5. DEFAULT-OFF SİCİLİ (63 giriş — `DEFAULT-OFF-REGISTER.md`)

Açıklanmamış tek bir OFF kalmadı: her giriş bir nedene bağlandı (kalibre-edilmemiş / opt-in ölçüm kapısı /
deneysel / gözlem-only / uyumluluk). Bu oturumda **6 default değişti** (FEC açıldı, birim tavanı symbol'e
hizalandı, bench 4 default'u ürüne bağlandı).

## 6. KABLOSUZ / ÖLÜ KOD (839 sembol taraması — `UNWIRED-REGISTER.md`)

94 DELETE adayı + 54 hiçbir yerden çağrılmayan sembol. Öne çıkanlar: **üç ayrı alıcı decode hattı**
(client-core `fec_ingest` · `kama_engine::admit_and_decode` · kama-integration `ReceiverHost`),
legacy `0x20 FEC_SHARD` kodeği, `FusionPolicy::calibration_v1`, `rate::evaluate`/`SrcDecision`,
`emergency::ack_decimation`, `Framing::SegmentBlock` + `RecoveryAction::MinimalParity` (spec-gerekli ama
hiçbir yerden çağrılmıyor), `fec_loss_risk_ppm` (RiskSplit ile duplicate).

## 7. ÜRÜN RUNTIME YOLU / 8. BENCHMARK YOLU

Artık **tek kaynak**: `KamaV1Profile::runtime_config()` → `RuntimeConfig::default()` → `ClientSession`
(tail validated_v1) → `Sender` → arbiter → pacer → wire. Bench aynı yapıcıdan başlar; her koşu
`KP-MANIFEST profile=kama-v1-full|kama-custom` + sapma listesi basar. Ayrıntı: `PRODUCT-PROFILE-v1.md`.

## 9. HEDEF-REJİM HAZIRLIĞI

**T0–T7 KOŞULMADI** (durdurma kuralı). Yerine mekanizma-aktivasyonu kanıtlandı (`ACTIVATION-PROOF.md`):
temiz hücrede parity 0; %20 kayıpta 46 repair tele çıktı (23 blok, arbiter `SpecPersistent` 4129 tick);
%50 burst'te ek olarak yapı-gate 1236 tick. ARQ + duplication + FEC birlikte koşuyor.

## 10. QUINN/TCP ADİLLİK DENETİMİ

- Önceki tüm Quinn kıyasları **INVALID COMPARISON** (kol ≠ ürün profili — B-5).
- Bench'te **TCP kolu YOK** — hedef zarf iddiası TCP'yi kapsıyorsa bu bir eksik.
- Açık metodoloji borçları (tema D): quinn handshake maliyeti ölçüm penceresi dışında; servis-semantiği
  normalizasyon tablosu `kama-product` kolunu kapsamıyor; blackout-survival ikili metriği ve operasyonel
  taban metrikleri yok; N≥30/p95-p99/%95 CI/commit provenansı yok.
- ADR-18 nightly netem kalite kapısı **stub kolunu** ölçüyor (`--transport kama`) → düzeltilmeli.

## 11. SINIFLANDIRMA KARARLARI

SHIP 243 · WIRE 93 (açık) · KEEP-EXPERIMENTAL 64 · DELETE 27 (mekanizma düzeyi);
kod düzeyinde ayrıca 94 DELETE + 54 çağrısız sembol. "Belki sonra" kalemi yok.

## 12. KAPI DURUMU

Yerel CI-eşdeğeri (fmt · clippy all/no-default · test --locked/--all-features/--no-default + rakam-duyarlı
test-adı bütünlüğü) bu dalda koşuldu. `client_survives_server_death_mid_run` yarışı **testte** düzeltildi
(`ac38fffd`; ürün semantiği değişmedi, tam paket ×8 yeşil). Hosted CI **faturalandırma nedeniyle
çalışmıyor** (PR #51'de 13 job 2 s'de "recent account payments have failed") — owner aksiyonu.

## 13. TRANSPORT v1 KAPANIŞ HÜKMÜ

```
TRANSPORT V1 BLOCKED BY:
  1. B-8 — dönüş-yolu ölçümü ve ACK decimation mimari olarak yok; asimetrik yol (T5) ve
     bozulmuş dönüş yolu (T6) hedef rejimleri için v1 ya bu asgari parçayı almalı ya da
     bu iki rejimi AÇIKÇA v1 dışı ilan etmelidir. (owner kararı gerektirir)
  2. FEC tamamlanmamış parçalar: kısmi/son blok kodlanmıyor (K<8 kuyruklar korumasız);
     §8.5 bant-türevli parity clamp'i bağlı değil; kayıp parity ESI namespace'iyle ARQ'ya
     DATA olarak giriyor; emergency MinimalParity spec'in tersini yapıyor.
  3. Telemetri: tail-probe ateşlemesi ve tahminci değerleri ürün derlemesinde gözlenemiyor —
     "mekanizma aktive oldu" kanıtı üretilemiyor (kapanış kriteri 8).
  4. Kıyas metodolojisi: setup-cost, servis-semantiği normalizasyonu, blackout-survival ikili
     metriği, N≥30/CI/provenans ve TCP kolu yok; ADR-18 nightly kapısı stub kolunu ölçüyor.
     T0–T7 bunlar kapatılmadan koşulmamalıdır.
```

**Bloke OLMAYAN, kapanmış kalemler:** ürün profili tanımlandı ve pinlendi · FEC ürün yolunda açık ve
aktivasyonu kanıtlandı · benchmark ürün profilini koşuyor ve manifest zorunlu · kuruluş guard'ı üç kolu
kapsıyor · kapı yarışı düzeltildi.

**Bu denetim yeni bir FEC/transport araştırma programı AÇMAZ.** Kalan blocker'lar ya küçük bağlama
işleridir (tema B/C), ya metodoloji işidir (tema D), ya da owner'ın kapsam kararıdır (B-8).
