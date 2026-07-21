# FUZZY-CONTROLLER ENVANTERİ (madde-5; kod-yazılmadan) — M-serisi-hizalama-raporu

> **TESPİT (iki-yol-sorusuna cevap):** Controller **MOTOR-OLARAK GERÇEK-FUZZY, AKTÜASYON-OLARAK YARIM.**
> Karar-matematiği tam ve parametrik (kalibrasyon-yüzeyi HAZIR: `FuzzyConfig`); ama **FEC-ekseni uçtan-uca
> KOPUK** (fec-kararı ölü-çıktı), ARQ-ekseni dejenere-binary, senkron-mod SPEC-NİYETİ. Sonuç: M-serisi verisi
> bugün doğrudan-kalibrasyona BAĞLANAMAZ — **önce KABLOLAMA-fazı (DR'lı), paralelinde karar-yüzeyi-taraması
> TASARIM+KALİBRASYON-verisi üretir.** M1-kök-2/kök-3 bulguları kod-yapısıyla birebir-doğrulandı.

## 1. ÇEKİRDEK — VAR ve gerçek (`fusion.rs`, 410-satır; fuzzy.rs-değil)

Trapezoidal-üyelik (LOW/MED/HIGH, per-mille tam-sayı) + Mamdani-AND(min) + **4-kurallı-tablo** + ağırlıklı-ortalama-defuzz.
ADR-17-TAM (float-yok, RNG-yok; golden `tv_fusion.rs` 10-test — yalnız-motor-matematiği pinli, eşik-iddiası-yok).
Port: `FusionController`-trait / `DeterministicFuzzyController`. `Sender::fuzzy_evaluate` (sender.rs:923-955) her-olayda çağırır.
Kural-tablosu (fusion.rs:334): R1 loss.LOW→fec0/arq_light · R2 loss.HIGH∧bw.HIGH∧jitter.LOW→fec_high/arq_moderate ·
R3 loss.MED∧bw.MED→fec_med/arq_med · R4 loss.LOW∧jitter.HIGH→fec0/arq_light. INV-RATE-1 yapısal-gömülü
(send_rate_gain yalnız bw+qd; loss rate-girdisi-DEĞİL). Zero-weight-fallback + `no_rule`-bayrağı ([TUR-4]).

## 2. GİRDİ-LİSTESİ (kritik-hizalama-tablosu)

| # | Girdi (FusionInputs) | Kaynak | Durum |
|---|---|---|---|
| 1 | `loss_windowed_ppm` | forward-loss ORANI (ledger-pencere; loss.rs:243) | VAR |
| 2 | `bandwidth_bps` | BtlBw windowed-max (log2_fp-domain) | VAR — **MUTLAK** (headroom-değil) |
| 3 | `jitter_ppm` | rttvar/srtt (RFC6298) | **YARIM** — spec `max(jitter, reordering_score)`; reordering KABLOSUZ |
| 4 | `qd_ratio_permille` | queue_delay/rtprop | VAR |
| 5 | `ack_confidence_permille` | ACK-SESSİZLİK-süresi | VAR — reverse-loss'un yalnız-PROXY'si |

**OKUMADIKLARI (deney-tasarımını-belirleyen eksikler):** loss-PATERNİ (IID-vs-burst ayrıştıran istatistik SIFIR —
fuzzy patern-farkını AYIRT-EDEMEZ) · reverse-loss-DOĞRUDAN (seyrek-ACK-kaybı sessizlik-eşiğini-aşmadıkça GÖRÜNMEZ;
return-bw sabit-önsel [CAL]) · feedback-gecikmesi (EAI yalnız-saat, girdi-değil) · **deadline** (fuzzy'ye-girmiyor;
arq'ta salt-sıralama-anahtarı) · headroom/utilization (C−offered hesaplanmıyor) · PMTU (sabit) · inflight.

## 3. ÇIKTI/AKTÜASYON-DURUMU (en-kritik-bulgu)

| Çıktı | Fiili-durum |
|---|---|
| `send_rate_gain` | **CANLI** — compute_send_rate→pacing (INV-RATE-1-sınırlı; tek-tam-kablolu-çıktı) |
| `mode` (Emergency) | **CANLI** — RecoverFwd'a-zorlar (tek-fuzzy-güdümlü-state-geçişi) |
| `arq_aggressiveness` | **DEJENERE** — plan() yalnız ==0-gate; ==0 yapısal-erişilmez → fiilen-hep-açık; seviye-büyüklüğü kullanılmıyor |
| `fec_parity_ratio_permille` | **ÖLÜ-ÇIKTI** — repo-genelinde-SIFIR-tüketici. fec_enabled=manuel-toggle; parity-miktar sabit (harness K8/R4); §10.2 `r=round(ratio×k)` YOK; emisyon=token-ARTIĞI |
| `compress` / `no_rule` | ölü / salt-teşhis |

**Sonuç:** "FEC-ağırlıklı-mod" controller'ın **NİYET-alanında** (R2-kuralı fec_high-üretir) ama **AKTÜASYON-alanının
DIŞINDA** — fuzzy'nin-FEC-niyeti hiçbir-kabloyla FEC-emisyonuna-bağlı-değil. **M1'in-ölçtüğü "fuzzy'nin-FEC-kararı"
DEĞİL, "sabit-oranlı-FEC'in artık-bütçe-altındaki-davranışı"ydı** — çerçeve-belgesinin-M1-kaydıyla tutarlı.

## 4. SENKRON-MOD-DURUMU: SPEC-NİYETİ (kodda 4-dar-nokta)

**VAR:** (i) karar-katmanı ortak-arbitrasyon (tek-tablo fec+arq üretir — ama fec-icrasız) · (ii) ortak-pacer =
örtük-bütçe (ama pay-değil **FIFO-ARTIĞI**: parity schedule'da EN-SON — M1-kök-3'ün-kodu) · (iii) RecoverFwd-önceliği
ARQ'yu-gölgeler (M1-kök-2'nin-kodu) · (iv) tek-GERÇEK-koordinasyon: DR-S5-4 RECOVERED-iptali (REAKTİF/post-hoc).
**YOK:** `plan_forward_duplication`↔`plan_fec_parity` çapraz-okuma SIFIR (aynı-unit'e dup+parity çift-harcama-mümkün) ·
§8.5 `parity_final/budget_cap` YAZILMIŞ-KABLOSUZ (budget.rs:45, yalnız-test) · §10.2 r-seçimi yok · §11 MinimalParity
ÖLÜ (Framing::Tiny-hardcode) · §9 fec-dominant-ARQ-baskılama YAPISAL-ERİŞİLMEZ (fallback arq≥arq_light) ·
parity-zamanlama-garantisi (rezervasyon/pay/preemption) yok. `K_ARQ_SHARE` = ARQ↔DATA-payı, FEC'le-ilgisiz.

## 5. EŞİK-DURUMU: parametrik-yüzey-HAZIR, değerler-KALİBRE-DEĞİL, injection-YOK

- `FuzzyConfig` 5×3-trapezoid + 9-consequent **parametrik** → kalibrasyon-verisinin bağlanacağı-yüzey BU.
- Tüm-değerler `fixture()` = **açıkça "KALİBRE-DEĞİL"** ([CAL:netem]; spec-disiplini: "UYDURULMAZ").
- **`Sender::new()` fixture'ı hard-wire — config-INJECTION-YOLU-YOK** (kalibrasyon=şimdilik-kod-değişikliği).
- ~22 sender-[CAL]-const compile-time; kural-tablosu + emergency-tetiği (min3>500‰) + compress FuzzyConfig-DIŞI-hardcoded.
- Ölü-kalibrasyon-yüzeyi: `sr_med`, ackc.med/high, qd.med hiçbir-kuralda — beslenecek-kural-yok.

## 6. ÖNERİLEN-TARAMA-EKSENLERİ (girdilere-hizalı; madde-6-taslağı)

| Eksen | Hizalandığı-girdi | Not |
|---|---|---|
| forward-loss-oranı | loss_ppm | ana-eksen; crossover-birincil-kaynağı |
| loss-paterni IID/GE-burst | **GİRDİ-YOK** | fuzzy-ayırt-edemez → tarama "patern-girdisi-gerekli-mi" TASARIM-verisi (kök-1) |
| bandwidth + headroom | bandwidth_bps | mutlak-bw-taraması + headroom-eksikliğinin-etkisi (M1-starving-tekrarı-farklı-C'lerde) |
| RTT/feedback-gecikmesi | jitter+qd (dolaylı) | RTT 100ms→5s; EAI-girdi-adaylığı-verisi |
| reverse-loss + sessizlik-epizotları | ack_confidence (proxy) | sürekli-oran-VS-epizodik-blackout ayrı-taranmalı (proxy'nin-kör-bölgesi; M1-kök-1) |
| deadline | **GİRDİ-YOK** | M2-ekseni; "deadline→FEC-kayması"-kuralı-tasarım-verisi (M1-kök-4) |

## 7. YOL-HARİTASI-TESPİTİ (madde-5b; bağlayıcı-öneri)

**MELEZ-YOL:** motor-fuzzy-gerçek → tarama-verisi-KALİBRASYONA-yarar; ama FEC/ARQ-aktüasyonu-kopuk → veri bugün
motora-BAĞLANAMAZ. Sıralama-önerisi:
1. **KABLOLAMA-fazı (DR'lı; mühürlü-sıralamaya-dokunuşlar-tartışılarak):** (a) `fec_parity_ratio`→r-seçimi (§10.2)
   + `fec_enabled`-fuzzy-bağlama, (b) §8.5-budget_cap-kablolama (parity-bütçe-kavramı; starving-politikası),
   (c) `arq_aggressiveness`→gerçek-aktüatör (tempo/cap/pay), (d) dup↔parity-çapraz-okuma (senkron-çekirdeği;
   RECOVER_FWD↔FEC-fusion — M1-kök-2), (e) `FuzzyConfig`-injection (kalibrasyon=veri-bağlama-olsun, kod-değil).
2. **Karar-yüzeyi-taraması (madde-6; kablolamayla-paralel-başlayabilir):** sabit-FEC/sabit-ARQ-armlarıyla crossover'lar
   (ÇIKTI-1/2) kablolama-beklemez; **senkron-arm (ÇIKTI-3) ancak-1d-sonrası** koşulabilir.
3. Eşik-işleme: tarama→FuzzyConfig-değerleri→golden-yeniden-türetme (spec-§16-adımı).

**M1-verisinin-bu-çerçevedeki-değeri:** crossover-değil (fuzzy-devre-dışıydı) ama iki-mekanik-bulgu kalıcı:
parity-starving-politikasızlığı (1b'nin-gerekçesi) + RECOVER_FWD-gölgesi (1d'nin-gerekçesi) + evicted-guard/miss-
semantiği (ölçüm-altyapısı-hazır).
