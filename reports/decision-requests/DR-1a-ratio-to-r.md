# DR-1a — fec_parity_ratio → r KABLOSU: SEMANTİK-TABLO (yapısal-yarı) [TASLAK — ONAY BEKLİYOR; NO-IMPLEMENTATION]

- **Amaç:** dead-output `fec_parity_ratio_permille`'i FEC-emisyonuna bağlayan kablonun TAM-semantiği.
  Spec-§10.2 `r = round(ratio × k)` tek-satır-kablo-DEĞİL — 28-semantik aşağıda. Feature-flag-arkası;
  **mevcut-manuel-davranış VARSAYILAN ve davranışsal-özdeş-kalır.**
- **ÖN-KOŞUL:** DR-1e (config-injection) onay+impl. **1a↔1b-SINIRI:** 1a = "KAÇ parity" (r-sayısı);
  1b = "parity NE-ZAMAN/HANGİ-bütçeyle" (token-pay/rezervasyon) — 1a emisyon-zamanlamasına DOKUNMAZ.

## 0. FEC-AÇIKLIĞININ NİHAİ-SAHİBİ — TEK-CEVAP

**NİHAİ-SAHİP = `FecActuationMode` (tek-otorite-enum; `fec_enabled`-bool ile `fec_parity_ratio` bağımsız-iki-
otorite olarak BIRAKILMAZ):**

```
enum FecActuationMode { Manual /* DEFAULT */, Controller }
```

| Mode | FEC-açık-mı kararı | `set_fec_enabled(b)` | `fec_parity_ratio` |
|---|---|---|---|
| **Manual** (default) | `fec_enabled`-bool (mevcut-DR-S5-1 davranışı AYNEN — **COMMIT-K-CORRIGENDUM:** enqueue-while-disabled artık-RED+sayaç ve drain-mode-kapısı-kalktı; pause/resume-semantiği-bilinçli-son, bkz. dr-1a-authority-continuity.md) | ETKİLİ (tek-yetkili) | OKUNMAZ (dead-output-kalır; telemetri-görünür) |
| **Controller** | `latched_ratio > 0` (blok-başına; §9-latch) | **ETKİSİZ** — çağrı no-op + `diag_manual_toggle_ignored`-sayaç (hata-değil; geçiş-dönemi-gürültüsüz) | TEK-YETKİLİ (r-türetimi §1-§8) |

Mode-geçişi: açık-API `set_fec_actuation(mode)`; geçiş **yalnız block-boundary'de-etkinleşir** (mid-block-
değişim-yasağı §9 ile aynı-latch-mekanizması). Precedence-belirsizliği YOK: her-an tek-otorite = aktif-mode.

## 1-28 SEMANTİK-TABLOSU

| # | Semantik | Karar-taslağı |
|---|---|---|
| 1 | Ratio-domain | permille `u32 ∈ [0,1000]` (fuzzy-çıktı-domain'i aynen; >1000 config-validate'te-reddedilir — DR-1e-§9) |
| 2 | Yuvarlama | tam-sayı: `r = (ratio·k + 500) / 1000` (round-half-up; float-yok, ADR-17) |
| 3 | Tie-breaking | half-up SABİT (yukarı) — deterministik, platformdan-bağımsız |
| 4 | `r=0` semantiği | MEŞRU: "bu-blokta-parity-yok" (FEC-fiilen-o-blok-kapalı). Zorla-r≥1 YOK |
| 5 | Min-parity | `r_min = 0` (emergency-MinimalParity AYRI-yol, bu-DR-dışı; deferred-6-senkron-kapsamı) |
| 6 | Max-parity | `r_max = min(profile.r_code_max, R_ENQUEUE_CAP)`; `R_ENQUEUE_CAP`=blok-başına-emniyet-tavanı (bounded-alloc; başlangıç=profile.r_code_max — ek-tavan-politika-fazında) |
| 7 | `r > profile_max` | **CLAMP** (reject-değil) + `diag_ratio_clamped`-sayaç (sessiz-değil) |
| 8 | Desteklenmeyen-K/R | Negotiated-profil-DIŞINA-ÇIKILMAZ: r her-zaman [0, profile.r_code_max]-içine-clamp; profil-değiştirme-YOK (PARAM-yeniden-müzakere ayrı-faz) |
| 9 | Mid-block-değişim | **YASAK.** `r` bloğun-parity-enqueue-ANINDA (blok-tamamlanınca) son-decision'dan **LATCH-edilir**; sonraki-decision'lar o-bloğu-etkilemez |
| 10 | Yeni-r hangi-boundary'de | SONRAKİ-blok: latch blok-tamam-anında-okunur → değişim doğal-olarak-bir-sonraki-blokta |
| 11 | `no_rule`-fallback | fallback-çıktı `fec_zero`-consequent → ratio=fec_zero-değeri → tipik-r=0 (fallback-değeri-config'ten; ek-özel-yol-YOK — tek-formül) |
| 12 | Invalid-config-fallback | DR-1e-validate tip-düzeyi-engeller (ValidatedConfig). Runtime-invalid-durumu-kalmaz; savunma: latch-okuma-hatası-imkansız (u32-permille) |
| 13 | Emergency-etkileşimi | `mode==EmergencyForwardRedundancy` iken: dup-yolu-mevcut-davranış AYNEN; **yeni-blok-parity-latch = 0 (bootstrap-muhafazakâr: emergency'de dup↔parity-çift-harcama-açılmaz; 1b/1d-arbitrasyonu-gelince-revize)**. Ürün-politikası-DEĞİL — deney-açıcı-bootstrap |
| 14 | `fec_enabled`↔ratio tek-kaynak | §0-tablosu: mode-enum TEK-otorite; Controller-mode'da `fec_enabled`-alanı İÇSEL-türetilmiş (`latched_ratio>0`) — dış-yazma-etkisiz |
| 15 | Manuel-toggle geçiş-dönemi | Manual→Controller: mevcut-`fec_enabled`-değeri ANLAMSIZLAŞIR (mode-tablosu-devralır); Controller→Manual: son-manuel-değere-DÖNÜLMEZ, `fec_enabled=false`'a-reset (bilinen-temiz-durum; sürpriz-emisyon-yok) — state-machine-notu impl-DR'ında test-edilir |
| 16 | Flag-OFF davranışı | OFF = `Manual`-default: **davranışsal-özdeşlik** {output-bytes · scheduling-order · pacing-decisions · counters · traces} baseline-özdeş (N8-spot-kalıbıyla-testli); ratio-latch-kodu Manual'da-HİÇ-çağrılmaz |
| 17 | ON-ama-controller-disabled | Tanım: Controller-mode + fuzzy-`no_rule`-sürekli → §11 (r=0-doğal). Ayrı-"disabled"-durumu-YOK (fuzzy-her-event-koşar; ADR-17) |
| 18 | Fixed-FEC-default-korunması | Manual-mode `enqueue_fec_parity`-manuel-akışı AYNEN (harness/M-serisi sabit-arm'ları bozulmaz) |
| 19 | Starvation-davranışı | 1a EMİSYON-YOLUNU-DEĞİŞTİRMEZ: plan_fec_parity token-artığı-mekanik AYNEN (starving-karakteristiği-değişmez; **yeni-starvation-açılmaz**, mevcut-da-çözülmez — o-1b'nin-işi) |
| 20 | 1a↔1b-sınırı | 1a=r-SAYISI (enqueue-adedi); 1b=bütçe/zamanlama (pay/rezervasyon/preemption). 1a-impl'i pacer/schedule-sıralamasına TEK-SATIR-dokunmaz |
| 21 | Retx/recovered-cancel-etkileşimi | Değişmez: parity-adedi-artarsa RECOVERED-olasılığı-artar → DR-S5-4-cancel-yolu-mevcut-mekanizmayla; yeni-etkileşim-kablosu-YOK |
| 22 | Sender/receiver-profil-uyumu | r ≤ negotiated-profile.r_code_max (§6-§8) → receiver-decode-uyumu-yapısal (profil-zaten-PARAM-bound; DR-S5-6/5d-1) |
| 23 | Wire-visible-profil-değişimi | GEREKMEZ: r-varyasyonu profil-İÇİ (R_max-altı); shard-header zaten (block_id, esi, k_actual)-taşır; receiver esi≥K'yı-repair-bilir. Profil-DEĞİŞİMİ (K/E) = PARAM-yeniden-müzakere (deferred) |
| 24 | Determinizm | Latch = decide()-çıktısından (deterministik-motor) + blok-tamam-anı (event-akışından-deterministik) → aynı-input→aynı-r (ADR-17; RNG-yok) |
| 25 | Telemetri/counter | `diag_fec_actuation() -> (mode, last_latched_ratio, last_r, ratio_clamped_count, manual_toggle_ignored_count, blocks_latched)` — M-serisi-erişilebilir |
| 26 | Rollback | Runtime: mode=Manual (anında; block-boundary-latch'le). Kod: feature-dalı-ayrı-commit; revert=Manual-yol-dokunulmamış-olduğundan-güvenli |
| 27 | Property-test-planı | ∀ratio∈[0,1000], k∈[1,255]: (i) r==round-half-up-formülü (ii) r≤r_max (iii) ratio-monoton→r-monoton (iv) ratio=0→r=0 (v) clamp-sayacı-doğru. proptest; ADR-17-golden-ek |
| 28 | Golden/non-regression | Manual-yol: {A+B+C}-tam + N8-bit-özdeşlik (Manual-vs-baseline decisions-dizisi-özdeş). Controller-yol: yeni-birim+property-testler; golden-yeniden-türetme-GEREKMEZ (fixture-değerleri-değişmiyor) |

## KAPSAM-SINIRI + KANIT-PLANI
Impl-DR'ı (onay-sonrası): dokunuş-listesi = sender.rs {mode-enum-alanı, latch, plan_fec_parity-çağıran-blok-tamam-
noktasında-r-okuma — DİKKAT: engine-tarafı-blok-tamam-bilgisi} + kama-engine {blok-tamam→enqueue×r}. Mühürlü-yasak-
6-blok + pacer/schedule-sıralaması dokunulmaz-beyanı + N8-bit-özdeşlik (Manual-yol) AYNI-commit. **Bu-belge-taslak;
onaysız-impl-YOK.**
