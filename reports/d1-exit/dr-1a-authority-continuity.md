# DR-1a SON-SEMANTİK-KAPI RAPORU (Commit K+L) — block-authority-continuity + product-reachability

> **NİHAİ-HÜKÜM (Abkarada, 2026-07-22): "DR-1a MECHANISM CLOSED — PRODUCT INTEGRATION DEFERRED."**
> **BASELINE-FREEZE:** Commit-K=`9f3d463` + Commit-L=`6da77ae` DONDURULDU. Şu-alanlara-yeni-değişiklik
> YALNIZ-ayrı-açık-DR'la: FecBlockAuthority · FecActuationMode · ratio→r-formülü · same-event-guard ·
> block-stable-emission-semantics · emergency-suppression · authority-continuity · variable-r-wire-
> kontratı. Kritik-regresyon-dışında DR-1a-yeniden-AÇILMAZ. Hükmün-kapsamı/kapsam-dışı-listesi
> talimat-metnindedir (mekanizma-kapalı; ürün-runtime/1b/1d/kalibrasyon/Kesit-2-hükümleri/Sprint-6 AÇIK).
>
> Talimat-statüsü (önceki-tur): "DR-1a FUNCTIONAL REVIEW PASSED / FINAL CLOSURE BLOCKED — BLOCK AUTHORITY
> CONTINUITY + PRODUCT-REACHABILITY SCOPE". Bu-rapor 4-kapsam-maddesinin kanıt-belgesidir.

## 1. Tespit-edilen-uyuşmazlık (talimat-§1) — DOĞRULANDI ve KAPATILDI

Commit-I-sonrası-gerçek: planlama-otoritesi witness'tan (block-stable) AMA emisyon-izni drain-anı-
current-mode'dan → "açıldığı-mode'la-kapanır" garantisi EKSİKTİ (talimat-örnek-riski birebir-gerçekti:
Manual{true}-blok parity-staged → Manual{false}-geçişi → drain-suprese). Ayrıca witness `Copy` idi —
tek-kullanım-engellenmiyordu (aynı-witness-N-latch-derlenirdi).

**Seçim: SEÇENEK-1 (block-stable-authority)** — varsayılan-tercihle-uyumlu; önceki-onaylı "current-blok-
eski-mode'la-tamamlanır" semantiğinin tamamlanmasıdır. Immediate-global-kill BİLİNÇLİ-reddedildi:
`set_fec_actuation_mode` normal-transition'dır; acil-safety-disable AYRI-DR-gerektirir (API-doc'da-açık).

**Mekanizma (cerrahi-sınır-içinde; scheduler-veri-modeli-değişmedi):** izin-kapısı KUYRUK-GİRİŞİNE-
taşındı — **kuyruk-üyeliği = kalıcı-emisyon-izni** ("membership-as-permission"). İzinsiz-item-hiç-
girmez (girişte-red+sayaç); girmiş-item'ı hiçbir-sonraki-mode-değişimi-bastıramaz (drain'de-mode-
kontrolü-YOK). Item-üstü-tag-gereksizleşti (taşınan-bilgi=üyelik); iptal (RECOVERED) izinden-ayrı-
kavramdır (iş-yok-oldu) ve çalışır.

## 2. FecBlockAuthority yaşam-döngüsü-envanteri (talimat-§2; kod-gerçeği)

| soru | cevap |
|---|---|
| Type-visibility | `pub struct` (sender.rs) |
| Field-visibility | 3-alan-HEPSİ-private (`instance_id`, `generation`, `mode`) |
| Constructor-visibility | Public-raw-ctor-YOK; yalnız `Sender::open_fec_block_authority(&mut self)` |
| Clone | **YOK** (compile_fail-E0599-doctest) |
| Copy | **YOK** (compile_fail-E0382-doctest — Copy-olsaydı-derlenirdi) |
| Default | YOK |
| Serialize/Deserialize | YOK (serde-impl-yok; private-alan → dış-inşa-imkansız) |
| Latch by-value/ref | **BY-VALUE** (`authority: FecBlockAuthority`) — tüketim |
| Aynı-authority-2×-latch | **COMPILE-TIME-İMKANSIZ** (E0382 use-after-move; doctest + pozitif-kontrol) |
| Aynı-authority-2-blok | Aynı-imkansızlık (1-tüketim=1-kapanış); NOT: tek-kullanım **per-WITNESS**tir — aynı-`block_id`'ye-iki-AYRI-witness-açıp-kapamak-mümkün (sender-blok-kaydı-tutmaz; block_id↔witness-1:1-bağı = engine-block-owner-entegrasyonunda, DEFERRED §7) |
| 1-blok-N-authority | API-engellemez (yukarıdaki-satır); BlockOwner-deseni-1:1-gösterir |
| block_id/generation/nonce | `generation`: sender-başına-monoton (saturating), witness'ta-taşınır; `block_id`-witness'ta-YOK (close-anında-caller-verir; binding-DEFERRED) |
| Sender-aidiyeti-kanıtı | `instance_id` (process-yerel-AtomicU64) tüketimde-karşılaştırılır |
| Yabancı-Sender'da-kullanım | **TYPED-RED** — latch: `ForeignAuthority`-suppression+sayaç+planned=0+kuyruk-dokunulmaz; manuel-close: `Err(ForeignAuthority)` (testli) |
| Blok-bitmeden-drop | Blok-TERK — sender-tarafı-state-YOK → sızıntı-imkansız (t210-testli) |
| Latch'siz-drop-gözlemi | GÖZLENMEZ (bilinçli): Drop-impl normal-tüketimde-de-ateşlenirdi (gürültü); stateless-witness'ta-terk-izsizdir |
| Consume-sonrası-reuse-engeli | COMPILE-TIME (Rust-ownership; runtime-consumed-bit-GEREKSİZ — talimat-tercihiyle-uyumlu) |
| Generation-overflow | `saturating_add` — u64::MAX'ta (2^64-açılış, ulaşılamaz) uniqueness-durur, tek-kullanım-ownership'i-ETKİLENMEZ (belgeli). instance-sayacı-wrap = 2^64-Sender-inşası (ulaşılamaz; belgeli) |

## 3. Blok-kimlik-modeli (talimat-§4): **B+C-hibrit** — sender-generation-token + identity-taşıyan-witness

Sahiplik-diyagramı:
```
 Engine/BlockOwner (blok-gerçeğinin-sahibi)          Sender (blok-bilmez; otorite+kuyruk-sahibi)
 ┌───────────────────────────────┐                   ┌─────────────────────────────────────────┐
 │ open-blok:                    │  open_fec_block_  │ generation++ (saturating)               │
 │   authority ◄─────────────────┼───authority()─────┤ witness{instance,gen,mode} ÜRET         │
 │   (blok-state'inde-saklanır)  │                   │ (pending-state-YOK)                     │
 │ ... source'lar eklenir ...    │                   │                                         │
 │ close-blok (TEK-SEFER):       │  by-value-consume │ kimlik-guard → witness-mode-dalı →      │
 │   latch(authority, snap, ...) ├──────────────────►│ plan → STAGING (üyelik=izin) → kuyruk   │
 │   VEYA close_manual(authority)│                   │ drain: mode-KONTROLSÜZ (block-stable)   │
 └───────────────────────────────┘                   └─────────────────────────────────────────┘
```
Saf-C (identity-free) YETMEZDİ (yabancı-Sender-reddi-istenmişti); saf-A (engine-owned-block-registry)
üretim-block-owner-yokken-inşa-edilemez → witness kimlik+generation-taşır (B), blok-1:1-bağı engine-
entegrasyonunda-tamamlanır (C-koşulları — non-Clone/Copy + by-value + her-blok-yeni-witness-API-zorunlu —
BUGÜN-sağlanır). `generation`ın-tüketimde-doğrulanmaması = bilinen-sınır (envanter-tablosu); gelecek-
engine-binding-anahtarı-olarak-alanda-hazır.

## 4. Planning-authority vs emission-authority — KESİN-TANIMLAR

- **Planning-authority:** bloğun parity-KARARINI (ratio→r-zinciri / manuel-r) veren-otorite =
  **witness-mode** (blok-açılış-yakalaması). Commit-I'dan-beri-yapısal; Commit-K tüketimi-tek-sefere-
  bağladı.
- **Emission-authority:** kuyruklu-parity'nin wire'a-çıkma-izni = **kuyruk-üyeliği** (girişte, kaynak-
  otoriteden-karara-bağlanır: latch-staging=witness; manuel-close=witness; legacy-enqueue=çağrı-anı-
  aktif-mode-snapshot'ı). Girişten-sonra HİÇBİR-global-mode-değişimi-bastıramaz.
- Sonuç-garanti: **"Blok, planlaması-ve-emisyonu-dahil, açıldığı-otoriteyle-tamamlanır; mode-değişimi
  yalnız-SONRAKİ-open'ı-etkiler."** (İptal ayrı: RECOVERED = işin-yok-olması, otorite-bastırması-değil.)

## 5. Emisyon-süreklilik test-matrisi (talimat-§6 → test-eşlemesi; HEPSİ-PASS)

| § | test | kanıt-notu |
|---|---|---|
| 6.1 | t61 | Manual-blok-parity Controller-geçişine-rağmen-2/2-emit; next=Controller |
| — | **t61b MUTASYON-PİNİ** | staging-izni-WITNESS'tan: aktif=Manual{false}-iken manuel-close-Ok(2)+2-emit VE Controller-witness-latch-planned-4+4-emit. Staging'i-current-mode'a-bağlayan-mutasyon (Seçenek-2) FİİLEN-uygulandı → t61b+t67-DÜŞTÜ, geri-alındı (adversarial-MAJOR-bulgu-kapanışı) |
| 6.2 | t62 | Controller-blok-4-parity Manual{false}-geçişine-rağmen-emit; next-blok-0 |
| 6.3 | t63 | eski-blok-kendi-r'si(2); next-manual-fixed(1) |
| 6.4 | t64 | emergency-suppression-latch-kararı transition-sonrası-AYNEN + vakum-pini (enqueued_r_total==0) |
| 6.5 | t65 | 4×1200B > burst-cap → çok-tur; ilk-emisyon-sonrası-transition; 4/4-tamamlanır; diag-enqueued==emitted |
| 6.6 | t66 | RECOVERED transition-SONRASI 3-staged-parity-cancel; ÇİFT-SAYIM-PİNİ (2-unit-aynı-blok → 3, 6-değil); emisyon-0 |
| 6.7 | t67 | interleaved-açık-2-witness (farklı-mode, farklı-generation); ÜÇÜNCÜ-geçiş-İKİ-CLOSE-ARASINDA (mutasyon-pini); blok-11=2, blok-12=4, contamination-yok |
| 6.8 | compile_fail-E0382 (+pozitif-kontrol-doctest) | reuse/2-blok-tek-witness DERLENMEZ; stable-rustdoc-hata-kodu-doğrulamadığından pozitif-kontrol-bloğu-vakumlaşmayı-CI'da-yakalar |
| 6.9 | commit_k_foreign_authority_and_manual_close | latch-suppression+sayaç+kuyruk-boş; manuel-close-Err; Controller-witness-manuel-close-Err(NotInManualMode) |

## 6. Adversarial-doğrulama (workflow: 6-çürütücü + critic) — refuted 0/6

MAJOR (staging-pin-eksikliği) → t61b+t67-reorder ile KAPANDI (mutasyonla-fiilen-doğrulandı). Kapanan-
minor'lar: stale-drain-guard-yorumu; ForeignAuthority-doc-daraltması; pozitif-kontrol-doctest; t64-
vakum-pini; t66-çift-sayım-pini; 6.9-kuyruk-boş-assert'i; corrigendum'lar (DR-S5-4-addendum, DR-1a-
ratio-to-r-satırı, review-hardening-supersession-notu); 5-davranış-değişikliği-tam-listesi-commit-
mesajında. **DEFERRED-minor/note-kaydı:** `manual_r`-clamp'sız + `pending_fec`-cap'siz (SR-AUDIT-001-
gerilimi; production-block-owner-entegrasyonunda-bound) · emitted-ama-lost-parity'nin decoded-blok-
iptali pending_lost'ta-YOK (doğal-takip-cancel'i; ayrı-küçük-iş) · block_id-aliasing (id-reuse →
yanlış-blok-cancel'i; generation-binding-gelecek-anahtar) · N8-harness'ları-non-empty-parity-queue-
arm'ıyla-koşmuyor (kod-inceleme-güvenli) · sprint5c-fec_off-yorumu-artık-giriş-kapısını-anlatmalı ·
M1-ignored-sweep'leri-eski-semantik-sayılarıyla (P5-GATE-re-run'da-tazelenir).

## 7. PRODUCT-REACHABILITY sınıflandırması (talimat-§7) + deferred (§8)

**Mekanizma-statüsü:** Controller-mode actuation mechanism implemented and integration-tested.
**Ürün-runtime-statüsü:** Product runtime block-owner integration not yet present.

**Grep-kanıtı (test-dışı-üretim-çağıran):** `set_fec_actuation_mode` / `open_fec_block_authority` /
`fec_controller_latch` / `close_fec_block_manual` / `enqueue_fec_parity` → sender.rs-dışında SIFIR
üretim-çağıran; kama-engine üretim-lib'i adaptive-transport'u YALNIZ-dev-dep-tutar (Cargo.toml).
Kod-düzeyi-enforcement: `enqueue_fec_parity` **#[deprecated]** (legacy-test-sınıfı; Controller-aktifte-
kararsız-battaniye-kabul-uyarısıyla).

**Dead-output DARALTILMIŞ-HÜKÜM:** "Controller-mode mekanizma yolunda fuzzy ratio çıktısı artık
dead-output değildir. Mevcut product runtime'da Controller mode'u ve block latch'i kullanan production
block owner henüz bulunmadığından adaptif FEC ürün yolunda etkin değildir." (Yasak-dil-kullanılmadı:
"production'da-tamamlandı" / "ürün-otomatik-fuzzy-FEC" / "product-dead-output-tamamen-kapandı" /
"Controller-gerçek-uygulamada-aktif" İDDİA-EDİLMEZ.)

**Deferred-kaydı (ayrı-DR-taslağı — sessiz-gömme-yok):**
[DR-1f-engine-block-owner-integration](../decision-requests/DR-1f-engine-block-owner-integration.md).

## 8. Token-denial attribution — 1b-ÖN-KOŞULU (talimat-§10)

`token_denied_r_gauge` mevcut-tanımıyla (son-red-anı-pending-adedi) block/event-attribution-TAŞIMAZ.
**KAYIT:** "Token denial değeri block/event attribution olmadan calibration veya budget policy verisi
olarak kullanılmaz." Gerekli-asgari-bağlam: denial-event-index + (mümkünse)-block-id/generation + mode +
pending-count. Bu-turda-üretim-değişikliği-YAPILMADI (talimat-izni); checklist:
[DR-1b-onkosullar](../decision-requests/DR-1b-onkosullar.md).

## 9. Emergency/no-rule bulgu-kaydı (talimat-§9)

"Fixture-geometrisinde gerçek-emergency-bölgesi, non-zero-FEC-kurallarından-ayrık ve no-rule-bölgesiyle-
çakışmaktadır." Sonuçlar: gerçek-emergency-yolunda-requested-ratio-sıfır-olabilir; `EmergencyPending
Arbitration`-non-zero-dalı-yalnız-injected-unit'te; emergency-FEC-politikası-bugün-ampirik-anlamsız;
1b/1d+kalibrasyon-öncesi-ürün-davranışı-olarak-yorumlanmaz. Kesit-2-grid+rule-coverage-girdisi:
kesit-2-plan-v1.md §G. Eşik/kural-DEĞİŞTİRİLMEDİ.
