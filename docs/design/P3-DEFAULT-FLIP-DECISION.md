# P3 — Varsayılan-Açma (Default-Flip) Karar Belgesi

**Statü:** **G2 ONAYLANDI + UYGULANDI (Abkarada, 2026-08-24).** G1 kapısı yeşil (7de0a205);
G2 flip'i `ClientSession::new()` → `set_tail_recovery_config(validated_v1())` olarak uygulandı
(client-core; çift-yönlü pin: ClientSession tail-ON probe eder, `Sender::new()` motor-default'u
OFF kalır — R1). G3 kabul (L10-med tuning ürün-v2). G4 onaylı: startup-pacing/bootstrap/FEC OFF
kalır — her biri TEK somut blokör arkasında, mekanizmalar bankada (§3-§5); "boşuna" DEĞİL,
tail'in bugün izlediği build→gate→flip hattının sırada bekleyen üyeleri. Otorite: yol haritası
`BITIRME-YOL-HARITASI-2026-08-18.md` §"REVİZYON 2026-08-24" P3. Bağlam: tüm ürün-varsayılanları
şu an **OFF** (tail · startup-pacing · bootstrap-RTT · FEC); flip'ler baştan beri "§4.4-benchmark-
sonrası"na bağlıydı — o kanıt artık var (kapi-eval-v1 + P0).

## 0. İlke (product-grade)

Bir varsayılanı açmak = o konfigürasyonu **sevk etmek** (dışa-dönük, geri-alması zor). Kural
(tail_recovery.rs R1'de kurulan): **doğrulanmamış hiçbir konfigürasyon default olarak sevk edilmez.**
Kanıt tek başına yeterli değil; kanıtın **mevcut motorda** geçerli olması gerekir.

## 1. Karar tablosu (dört aday)

| aday | mevcut | ÖNERİ | gerekçe |
|---|---|---|---|
| **tail-recovery** | OFF | **ON (koşullu)** | §2 — kanıt lehte; P0-sonrası re-validation kapısına bağlı |
| startup-pacing | **ON (motor default'u)** | **ON — zaten açık** | §3 DÜZELTME — `925dea66` (2026-08-12) default-ON'u geri getirdi; fuzzy_e2e blokörü çözülmüş (bugün 5/5 yeşil). İlk taslaktaki "OFF" iddiası hatalıydı (BootstrapRtt'nin false'u yanlış atfedildi) |
| bootstrap-RTT | OFF | **OFF kalır** | §4 — kilitli panel 2/3 FAIL (rep14 GE-L10) |
| FEC (Controller) | Manual{false} | **OFF kalır** | §5 — Q18 kalibrasyon kusuru Controller'ı yasaklıyor |

## 2. Tail-recovery → ON — **G1 KAPISI GEÇTİ (2026-08-24)**

> **GÜNCELLEME:** §2.3 re-validation kapısı KOŞULDU ve YEŞİL çıktı
> (`reports/g1-tail-revalidation/G1-CLOSURE.md`, `7de0a205`): burst tail-ON ≥ tail-OFF (h3b/h3c
> medyan 1.000 vs 0.990), p2 no-harm (n=10), P0 primary-regresyon YOK (post-P0 0.523 ≈ banked 0.505).
> **G2 (default-ON) artık Abkarada onayına hazır.** Aşağıdaki kanıt/gerekçe kaydı korunur.

### 2.1 Lehte kanıt

1. **Banked ürün-doğrulaması:** DR-ARQ-PRODUCT-TAIL-1, faithful two-netns/netem, **2290/2290 satır,
   GO** (`validated_v1` = KAMA-TAIL-V1 sabiti; pin-testli). Terminal-tail OFF %2.2 → ON %91.4;
   inv_fail=0, spurious=0.
2. **Kampanya primary (kapi-eval-v1) — nötr:** p2 rastgele-kayıpta kama-arq ≈ kama-tail-v1
   (medyan 0.495 vs 0.505). Tail ON hiçbir zarar vermiyor.
3. **P0 netem (2026-08-24, YENİ motor) — kuyruk-körlüğü ölçüldü:** h3b (GE-burst %50) fix-sonrası
   medyan teslim: **kama-tail-v1 98/100, kama-arq 95/100** (q-dgram/q-stream 100). Aradaki −3pp
   TAM OLARAK kuyruktan: kama-arq rep1'de teslim-edilmeyen 15 birimin 10'u ardışık kuyruk bloğu
   (id 90-99); kama-tail-v1'de yalnız 2. Neden: kuyruk-sonrası yeni paket yok → packet-threshold
   çalışamaz; tail-probe (yalnız ON'da) kuyruğu yoklayıp kurtarıyor. **Bu, tail-recovery'nin neden
   gerektiğinin doğrudan ölçülmüş gerekçesidir.**

### 2.2 Aleyhte / dikkat (dürüstlük)

1. **Ön-kayıtlı yan-kol testi YOK (audit #15):** kampanya kama-tail-v1 vs kama-arq'ı ön-kayıtlı
   paired-test ETMEDİ (yalnız betimsel medyan). §2.1'deki sayılar betimsel/tanısaldır, hüküm değil.
2. **`validated_v1` P0-ÖNCESİ motorda doğrulandı.** P0 (`759d4aa8`) queue_losses forward-recovery
   yolunu değiştirdi. Tail-probe kod-yolu ayrı olsa da, default-ON'u sevk etmeden önce `validated_v1`
   **P0-sonrası motorda** yeniden doğrulanmalı — yoksa R1'de kapattığımız "doğrulanmamış default"
   sorununu ince biçimde geri getiririz.
3. **Bilinen tuning açığı:** GO verdicti "MECHANISM CORRECT, **POLICY/TUNING PENDING**" idi:
   terminal-tail L10-med 0/30 (sabit cap-3 tail-uzunluğuna uyarlanmıyor) + yüksek-kayıp cold-start
   token-açlığı. Default-ON, bu açıkları KABUL etmek demektir (mekanizma doğru, politika kaba).

### 2.3 KARAR: ON — şu KAPI geçilirse (aksi hâlde OFF kalır)

**Re-validation kapısı (P0-sonrası, bounded — YENİ kampanya DEĞİL):** `validated_v1` ile,
post-P0 binary, 3-netns netem, şu hücreler × n≥5:
- **terminal-tail** (OFF/ON): ON ≥ OFF (banked GO tekrarı; cap-3 hâlâ tutuyor, inv_fail=0).
- **h3b/h3c** (GE-burst): kama-tail-v1 ON ≥ kama-arq (P0 bulgusunun tekrarı).
- **p2 paced** (rastgele): ON ≈ OFF (zarar-yok; nötr korunur).
Kapı yeşilse: `ClientSession`/ürün-kuruluşu `set_tail_recovery_config(validated_v1())` ile açılır;
`TailRecoveryConfig::default().enabled` **`false` KALIR** (default-bounds hiç doğrulanmadı — R1 kuralı).
Yani "flip" = ürün-kuruluşunun validated_v1 çağırması, default-struct'ın değişmesi DEĞİL.

L10-med tuning açığı ürün-v2 backlog'unda kalır (adaptif cap/backoff); default-ON bunu bilerek kabul
eder çünkü açık = "recover edemez" değil "daha az agresif recover eder" (mekanizma güvenli, bounded).

## 3. Startup-pacing → ZATEN ON (DÜZELTME, 2026-08-26)

**Bu bölümün ilk hali hatalıydı.** Gerçek durum: `StartupPacingConfig::default()` = `enabled: true`
— `925dea66` (2026-08-12, "restore startup pacing default-ON") EK4-A2 fix'i ve fuzzy_e2e
floor-vs-parity blokörünün çözülmesi sonrası default'u geri açtı; satır-672 civarındaki OFF-revert
yorumu TARİHSELdir (kayıt sonradan düzeltilmiş: "value was right, record was stale"). Bugünkü kanıt:
`cargo test -p kama-engine --test fuzzy_e2e` 5/5 (r2_to_emission + partial_block dahil), EK4-A2
regresyonu mühürlü. İlk taslağın hatasının kökü: `enabled: false` grep'inin BootstrapRttConfig'e
ait satırı startup-pacing'e atfedilmesi + bayat hafıza kaydı. **Karar gerekmiyor; mekanizma sevk'te.**

## 4. Bootstrap-RTT → OFF kalır

DR-CS-3 bootstrap RTT (ladder [3,9,21,45]s) §1 preflight PASS ama **kilitli panel 2/3 FAIL**
(rep14 = reverse-echo GE-L10 ladder'ı aşıyor); mekanizma 4/4 doğru ama 16/16 baseline frozen-record'u
birebir üretiyor ⇒ default-OFF. Yeniden-açma için panel geçmeli. **OFF kalır.**

## 5. FEC (Controller) → OFF/Manual kalır

Q18 kalibrasyon kusuru (bankada, pinli): controller %20 kayıpta parity=0 planlıyor; diz 250k ppm'de.
Yapı-kapısı estimator'ü hazır ama hiçbir kural onu tüketmiyor (Prereg-A enstrümantasyon dalı).
Controller sevk edilemez. Manual{false} default kalır; static-r Manual yalnız FAZ-2 kampanya kolu.
**OFF kalır; blokör: structure-gate kalibrasyonu (ürün-v2).**

## 6. Onaylanacak

| # | karar | öneri |
|---|---|---|
| G1 | tail-recovery re-validation kapısını KOŞ (post-P0, bounded netem) | onay |
| G2 | Kapı yeşilse tail default-ON (ürün-kuruluşu validated_v1; default-struct değişmez) | onay |
| G3 | L10-med tuning açığını ürün-v2'ye ertele (default-ON bilerek kabul eder) | onay |
| G4 | startup-pacing / bootstrap / FEC OFF kalır | onay |
