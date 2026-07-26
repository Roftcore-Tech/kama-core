# DR-CS-1 TUR-A1 — R2 PRE-REGISTRATION (Abkarada DUR-B kararı: A3 + B1)

> **BAĞLAYICI ÇERÇEVE:** ARQ tüm kollarda aktiftir; DR-CS-1 cold-start LIVENESS DR'ı. **PRODUCTION CODE
> DEĞİŞİKLİĞİ SIFIR** (yalnız harness/test-only). Bu belge W-2B-R2 + güncellenmiş W-3-65 kabulünü, HERHANGİ
> BİR R2-KOŞUSU YAPILMADAN ÖNCE dondurur. git-commit-hash + author-date = R2 anchor. §5: sonuca-bakıp
> beklenti/[CAL]/path-sınıfı değiştirmek YASAK.

**Dayanak:** Abkarada DUR-B kararı (2026-07-26; A3+B1 ONAY, A2/B2 RED) · DUR-B doc `reports/decision-requests/
DR-CS-1-A1-recovery-path-DUR-B.md` (b519d31) · Devralınan geçerli [CAL] tablosu: `DR-CS-1-A1-pre-registration.md`
§1 (HORIZON=120s · RX_TEST_PATIENCE=130s · T_MAX=30s · SUPPRESSED_ACK_CAP=64 · RTT-tol) DEĞİŞMEZ.

---

## 1. W-2B → HÜKÜMSÜZ (arşiv)
```
REJECTED AS MECHANISTICALLY INAPPLICABLE —
FULL DATA-ACK/SACK SUPPRESSION PREVENTS §9 ARQ FROM FIRING.
SUPERSEDED BY W-2B-R2.
```
Eski `full ACK-starvation → ProbeEcho → ExistingPtoArq` hipotezi §9-ARQ SACK-reaktif olduğu için mekanistik
geçersiz. Eski pre-reg arşivlendi; sonucu değiştirilmedi; PASS SAYILMAZ.

---

## 2. W-2B-R2 — FULL ACK-STARVATION NATURAL RECOVERY-PATH CHARACTERIZATION AND COLD-START CAUSALITY

### 2.1 Matched kollar (aynı suppression/workload/RTT/argümanlar)
```
ON:      experimental-cold-start ON  · full matching DATA-ACK/SACK suppression · probe-echo passes
CONTROL: experimental-cold-start OFF · AYNI suppression/workload/RTT/argümanlar · (probe-echo yok — feature OFF)
```

### 2.2 Amaç
- Doğal PRODUCTION recovery-path'ini ÖLÇMEK (hardcode YOK).
- Yeni/test-only recovery-path OLMADIĞINI kanıtlamak.
- ON/OFF recovery + completion zamanlarını KARŞILAŞTIRMAK.
- Cold-start'ın nedensel MARJİNAL etkisini HÜCRE-BAZINDA sınıflandırmak.
- Transport-duplicate + application-dedup maliyetlerini ölçmek.

### 2.3 İzin verilen recovery_path SINIFLARI (önceden-hardcode YASAK)
```
ForwardRecoveryDup
ExistingPtoArq
OtherExistingProductionPath [exact-kind]
NoRecovery
```
**Yeni/test-only recovery-path görülürse → DUR-B.** (Harness YALNIZ mevcut TransmitKind'leri emit-eder;
recovery_path bu emit'lerden GÖZLENİR, atanmaz.)

### 2.4 Matris (54 koşu)
```
RTT {100, 250, 1000} × size {65, 300, 1200} × feature {ON, OFF} × 3 tekrar = 54 koşu
```
Her satır §5-enstrüman-zinciri (acceptance-RTT-pin + per-cell RTT-realization + overlap-gate +
binary-provenance + arg-paritesi + clock-domain). ON=bin_cs, OFF=bin_off; arg-diff YALNIZ feature (rapora).

### 2.5 Zorunlu ek ledger (HER koşu; her-iki-build'de mevcut)
```
feature_state                    (ON|OFF)
forward_recovery_eligible_time
forward_recovery_wire_time
recovery_path                    (izin-verilen-sınıflardan)
bootstrap_srtt_time              (ON'da; OFF'ta none)
completion_time                  (block-0 ilk-completion; harness-block-timestamp)
on_minus_off_recovery_delta      (analiz-aşamasında hesaplanır)
on_minus_off_completion_delta    (analiz-aşamasında hesaplanır)
```
+ mevcut 25-alan W-2B ledger (transport_duplicate_rx / application_delivery / application_duplicate=0 /
suppression_start/release / released_ack_wire; ON'da ayrıca probe/echo/srtt_source/terminal_class).

### 2.6 Hüküm — HÜCRE BAZINDA
```
DECISIVE                      (OFF NoRecovery/geç ∧ ON recovery/erken; cold-start nedensel-belirleyici)
BENEFICIAL                    (ON anlamlı-daha-iyi ama OFF de sonunda recover)
NEUTRAL / NO MARGINAL BENEFIT (ON≈OFF)
NO RECOVERY                   (her-iki-kol da recover-etmez)
```
**ON≈OFF ise cold-start lehine PASS YAZILMAZ → "natural forward-recovery self-heal" olarak raporlanır.**

---

## 3. W-3-65 — B1 UYGULANIR (forward-recovery KAPATILMAZ)

### 3.1 Yeni faithful kabul (65B; restore=61; --no-settle-exit; HORIZON=120s; RX_PATIENCE=130s; 3 tekrar)
```
TimedOut exactly once
restore@61s
existing production recovery
finalized_blocks = 1
pending_blocks   = 0
application data correct
application_duplicate_delivery = 0
no busy-loop
no second timeout
```
### 3.2 65B rejiminde BEKLENEN sınıf
```
terminal_class      = TimedOut
recovery_observation = RecoveredByForwardRecoveryControl
PostTimeoutRecovery  = not expected
```
`PostTimeoutRecovery` DATA-ACK-kaynaklı **300/1200B rejimi için KORUNUR** (in-process F3-D + F4 kanıtları).

### 3.3 A2/DR-CS-1b açık maddesi (eklendi)
```
POST-TIMEOUT RECOVERED-CONTROL NOTIFICATION CONTRACT
```
Ürün-katmanının ACK-bağımsız post-timeout-recovery'yi NASIL öğrendiği A2'de karara bağlanmadan **end-to-end
product semantics FULL SAYILAMAZ.** (Bu tur SENDER-SIDE; ürün-alıcı A2.)

---

## 4. FAZ (Abkarada)
1. Eski pre-registration ARŞİVLENDİ (sonuç değişmez) — `DR-CS-1-A1-pre-registration.md` SUPERSEDED-header.
2. R2 pre-registration bu commit'te DONAR (yeni hash+timestamp).
3. W-3-65 3 tekrar koşulur.
4. W-2B-R2 54 matched koşu koşulur.
5. Adversarial-verify: ON/OFF arg-paritesi + path-sınıflandırması + nedensellik-hükmü çürütülmeye-çalışılır.
6. Dürüst closure hükmü.

**Yasaklar:** production-code-değişikliği YASAK · W-2C partial-suppression/PTO-dalı bu-tur OPSİYONEL + AYRI-hücre ·
sonuca-bakıp-path/eşik/[CAL] değiştirmek YASAK · merge/flip YASAK.

## FREEZE-BEYANI
Bu belge W-2B-R2 (54) ve W-3-65 (3) koşuları YAPILMADAN ÖNCE commit edilir. git-commit-hash + author-date =
R2 anchor. Değişiklik gerekirse → YENİ pre-registration + DUR-B.
