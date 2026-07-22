# DR-1a REVIEW-HARDENING RAPORU (Commit I+J) — nihai-kapanış-kanıtları

> Talimat: "DR-1a IMPLEMENTATION FUNCTIONALLY PASS — FINAL CLOSURE PENDING REVIEW-HARDENING".
> Bu-rapor 5-kapanış-maddesinin kanıt-belgesidir: (1) mode-transition-blok-sınırı-YAPISAL-garantisi,
> (2) gerçek-fuzzy-E2E, (3) G/H-sahiplik-corrigendum'u, (4) wire/profil-hüküm-daraltması,
> (5) dead-output-kapanışı + diag-semantik-tablosu.

## 1. Mode-transition blok-sınırı — sahiplik-envanteri + model-seçimi (Commit-I, `fb19f29`)

**Sahiplik-envanteri (kod-gerçeği):** Blok-kavramı (block_id, k_actual, açık/kapalı-durumu) SENDER'DA
YOK — yalnız engine/harness-tarafında yaşar (`controller_actuation.rs`/`mode_boundary.rs` BlockOwner;
üretimde gelecek-engine-blok-sahibi). Sender'ın gördüğü: latch-çağrısı + enqueue-çağrıları. Mode-setter
ve latch'in ÜRETİM-çağıranı yok (test/harness-only — grep-teyit).

**Model-değerlendirmesi:**
| model | tanım | karar |
|---|---|---|
| A | sender-içi "blok-kapanana-kadar-pending-mode" | İMKANSIZ — sender-blok-durumu-bilmez |
| B | transition'ı-blok-kapanış-event'ine-kuyrukla | İMKANSIZ — sender'da-blok-kapanış-event'i-yok |
| C | caller-contract (disiplin-varsayımı) | RED — talimat-gereği-yapısal-değil |
| **D** | **witness: açılış-anı-mode-yakalama** | **SEÇİLDİ** — tip-düzeyi, pending-state'siz |

**Model-D (dar-varyant):** `FecBlockAuthority` — private-alan, YALNIZ `Sender::open_fec_block_authority()`
kurar (dışarıdan-kurulamaz); blok-AÇILIŞINDA aktif-mode'u-yakalar; `fec_controller_latch(authority, …)`
dalı `authority.mode()`'dan-seçer (self.fec_mode'dan-DEĞİL) → **"blok-açıldığı-mode'la-kapanır"
tip-düzeyi-garantidir**, caller-disiplini-değil. Pending-state-yok → idle-transition-HEMEN (2.5),
no-future-block-leak-yok (2.10), repeated-transition aktif-mode'da-last-write-wins + açık-witness-
etkilenmez (2.9).

**Test-matrisi (2.1–2.10):** `kama-engine/tests/mode_boundary.rs` 9-test HEPSİ-PASS (t21, t22,
t23_t24-birleşik, t25, t26, t27, t28, t29, t210). 2.8-nüansı-belgelendi: blok'un parity-KARARI
geriye-dönük-değişmez (diag-kaydı-sabit); zaten-enqueue-edilmiş-parity'nin EMİSYONU mevcut-DR-S5-1-
semantiğiyle current-authority-emisyon-kapısına-tabidir (karar-kaydı ≠ akış-kapısı).

**Transition-telemetri:** `last_transition_event_index` (gauge) + `transition_applied_count`
(saturating-counter) — `FecActuationDiag`'da.

## 2. Gerçek `FusionInputs → decide → parity-emission` E2E (Commit-J)

`kama-engine/tests/fuzzy_e2e.rs` — 5-test PASS. Yasak-uyum: snapshot-alanı-YAZILMAZ; ratio-hook-YOK;
latch fuzzy-katmanını-ATLAMAZ (snapshot = gerçek `controller_decision_snapshot()`); gerçek-`on_event`-
canonical-yolu; config = `try_with_fuzzy_config(fixture)` validated-yolu. Rejimler batch-ACK/kayıp-ilan
desenleriyle SÜRÜLÜR (girdi-enjeksiyonu-değil: motor-ölçümleri gerçek-event-akışından türer).

| test | sürülen-gerçek-inputs (ölçülen) | fuzzy-çıktı | zincir-sonucu |
|---|---|---|---|
| 4.1 non-zero (R2) | loss=592145 (H=921) · bw=800000 (H=1000) · jitter=1312 (L=1000) | **ratio=800** (R2=921) | computed=6→clamp **planned=4 → emitted=4 → decode-RECOVERED** (1-source-düşürüldü) |
| 4.2 zero (R1) | loss≈0 (temiz-sürüş) | ratio=0 | planned=0, emitted=0, source-only-decode ✓ |
| 4.3 no-rule | loss=659090 (≥600k-plateau: med=0) · bw=80000 (high=0) | **no_rule=true** | LITERAL-0; suppression=`NoRule` ✓ |
| 4.5 partial k=5 | 4.1-rejimi | ratio=800 | planned=(800×5+500)/1000=4=clamp ✓ emitted=4, decode ✓ |
| 4.4 emergency | loss=787878 · bw=26666 · ackconf=393 | **mode=EmergencyForwardRedundancy (GERÇEK-YOL)** | latch-interlock planned=0 ✓ |

**4.4-GERÇEK-BULGU (fixture-geometrisi):** emergency-tetiği (bw.low>500 ⇔ bw<75kB/s) R2'nin
bw.high-öncülüyle AYRIK; loss-HIGH-plateau'da (≥600k) loss.med=0 → R3-de-sıfır → gerçek-sürülen-
emergency-noktasında `no_rule ∧ emergency` BİRLİKTE-doğru. Guard-önceliği (Commit-G-mühürlü-sıra)
`NoRule`'u-önce-raporlar; her-iki-dal-da planned=0-interlock. `EmergencyPendingArbitration`-dalı
enjeksiyonlu-unit'lerde-kanıtlı (dr_1a_latch_guards, t27). Bu KALİBRASYON-bulgusudur (kesit-2-sonrası
1b/1d-arbitrasyon-girdisi) — bu-turda-eşik-değişikliği-YAPILMADI (yasak).

**Harness-zaman-ölçeği-bulguları (araç-doğrulama; motor-bulgusu-değil):**
- Ledger-saatleri EAI-return-önseli-tabanlı (65B/31B/s ≈ 2.1s) → confirm_age≈3.15s, W_loss≈16.8s.
  Kısa-sürüş YAPISAL ppm=0 okur (pay=yaşlanmış-Confirmed, payda=pencere-içi-TÜM-gönderim);
  gözlenen-ppm ≈ kayıp-frac × (W−confirm)/W → pump-süresi ≥ W_loss şart.
- btlbw-MAX-filtresi (W≈16.8s): uzun-pump'ta warm-örneği-expire-olur → periyodik clean-burst
  (ardışık-round batch-ACK, Δt≈9ms → ~1MB/s-örneği) max'ı-tazeler; loss-oranını-dilüe-etmez.
- ACK'siz-PTO-drenajı re-loss/re-queue-döngüsüne-girer (burst-cap≤2×MTU) → parity'ye-sıra-gelmez;
  emisyon-fazında emitted-DATA'nın batch-ACK'lenmesi (gerçek-drenaj) şart.

## 3. Commit G/H sahiplik-CORRIGENDUM'u (git-diff-gerçeği)

`dr-1a-implementation.md` üst-notuna-işlendi: **G (`9a30240`) = üretim-actuation-yüzeyi** (sender.rs
+439-satır: authority-enum, latch, compute_requested_r, diag, plan_fec_parity-guard; + 6-dosya-tek-satır-
API-migrasyonu). **H (`5097136`) = engine-path-entegrasyon-testleri+kanıt** (yalnız `controller_
actuation.rs` + 2-rapor; SIFIR-üretim-satırı). Sunumdaki "G=test+docs-only" ifadesi-YANLIŞTI; rapor-
başlığındaki "H=engine-actuation-wiring" YANILTICIYDI. Tarih-yeniden-yazılmadı; düzeltme-not-olarak.

## 4. Wire/profil hükmü — DARALTILMIŞ-NİHAİ-İFADE

Mevcut codec/schema ve negotiated profile içinde, `0 ≤ emitted_r ≤ r_code_max` değişimleri için ek
wire alanı veya profile-ID değişimi gerekmediği preflight ve engine-path testleriyle doğrulandı.

KAPSAM-DIŞI (iddia-edilmez): K/E-değişimi · codec-değişimi · yeni-profil-formatı · gelecekteki-receiver-
davranışları · PARAM-yeniden-müzakeresi · blok-ortası-profil-değişimi · `r > r_code_max` · farklı-wire-
schema-versiyonu. ([variable-r-engine-matrix](variable-r-engine-matrix.md)'e-de-işlendi.)

## 5. Dead-output kapanış-kanıtı

**Tek-cümle-cevap:** Controller-mode'da fuzzy'nin ratio-çıktısı aynı-event-snapshot'ı-üzerinden
latch→enqueue→plan→emisyon-zincirine-bağlanmıştır ve gerçek-E2E-testte enjeksiyonsuz-non-zero-ratio
gerçek-parity-emisyonuna-ve-decode-recovery'ye-ulaşmıştır — fuzzy-FEC-çıktısı-artık-ölü-değildir.

**8-halka-zincir (her-halka-testli):**
| # | halka | mekanizma | kanıt |
|---|---|---|---|
| 1 | üretim | `fuzzy_evaluate` → FusionDecision (index++) | fuzzy_e2e-4.1 (gerçek-inputs), ordering-testi |
| 2 | snapshot | `controller_decision_snapshot` (canonical-kopya) | fusion_snapshot_equals_controller_input |
| 3 | eligibility | same-event-causality-guard (index-eşitliği) | dr_1a_latch_guards (mismatch→LITERAL-0) |
| 4 | ratio | `requested_ratio_permille` latch-kaydı | dr_1a_last_latch_full_overwrite |
| 5 | r | `compute_requested_r` (×k_actual, half-up, clamp) | golden-K8 + exhaustive-property |
| 6 | enqueue | `enqueue_fec_parity` × planned_r (`enqueued_r_total`) | engine-matris (enqueued==planned) |
| 7 | plan | `plan_fec_parity` (token; emisyon-guard; `token_denied_r_gauge`) | engine-matris + N8 |
| 8 | emisyon→decode | FecParity-decision (`SrLogicalRef::Fec`) → seal → `admit_and_decode` | fuzzy_e2e-4.1/4.5 (emitted==planned; decode-RECOVERED) |

## 6. FecActuationDiag semantik-tablosu (§8)

| alan | tür | kapsam | reset/overwrite | saturating | Manual'de | Controller'da |
|---|---|---|---|---|---|---|
| authority_mode | state-gauge | anlık | setter-anında | — | `Manual{enabled}` | `Controller` |
| last_latch | gauge (tam-kayıt) | son-latch | **her-latch'te-TAM-overwrite; carryover-YOK (§8.3-test)** | — | `ManualDisabled`-suppressed-kayıt | gerçek-latch-kaydı |
| latch_event_index | gauge | son-latch | overwrite | — | aynı | aynı |
| no_eligible_decision_count | counter | kümülatif-ömür | ASLA (reset-yok) | ✓ | latch-çağrılırsa | latch-çağrılırsa |
| decision_event_mismatch_count | counter | kümülatif | ASLA | ✓ | — (guard-sırası) | mismatch'te |
| no_rule_zero_count | counter | kümülatif | ASLA | ✓ | — | no_rule-latch'te |
| manual_toggle_rejected_count | counter | kümülatif | ASLA | ✓ | — | `set_manual_fec_enabled`-reddi |
| enqueued_r_total | counter | kümülatif | ASLA | ✓ | manual-enqueue-sayar | planned_r-enqueue-sayar |
| emitted_r_total | counter | kümülatif | ASLA | ✓ | wire-FecParity | wire-FecParity |
| canceled_r_total | counter | kümülatif | ASLA | ✓ | RECOVERED-iptal | RECOVERED-iptal |
| token_denied_r_gauge | **GAUGE** | son-token-reddi | her-redde-overwrite | — | aynı | aynı |
| last_transition_event_index | gauge | son-transition | overwrite | — | aynı | aynı |
| transition_applied_count | counter | kümülatif | ASLA | ✓ | aynı | aynı |

**§8.1 token_denied_r_gauge EXACT-TANIM:** SON `plan_fec_parity` token-REDDİ-anında-kuyrukta-bekleyen-
parity-adedi. Kümülatif-DEĞİL; `requested−emitted`-DEĞİL; clamp/suppression/mode-disabled-farkları
token-denial-SAYILMAZ (yalnız-pacer-token-reddi). 1b-tasarım-girdisi. (Eski-ad `budget_denied` —
"budget" 1b-bütçe-kavramıyla-çakışıyordu; rename Commit-I.)

**§8.2 saturation:** tüm-counter'lar `saturating_add` — wrap-YOK; sınır-testi
`dr_1a_counter_saturation_boundary` (MAX−1→MAX→MAX; debug/release-özdeş).

**§8.3 snapshot-reset:** `last_latch` her-latch'te-alan-alan-tam-yeni-değer (önceki-suppression/ratio-
taşınmaz) — `dr_1a_last_latch_full_overwrite_no_carryover` (suppressed→temiz→suppressed-üçlüsü).

## 7. Kapsam-dışı (bu-turda-DOKUNULMADI; yasak-listesi-uygulandı)

Pacer/BtlBw/token-accrual/kuyruk-sırası/rezervasyon/preemption (1b) · dup↔parity-arbitrasyon (1d) ·
fuzzy-eşik/kural-değişikliği (kesit-2-sonrası-kalibrasyon; §2-emergency-bulgusu-girdi-olarak-kaydedildi) ·
Kesit-2-koşusu · Sprint-6. BtlBw-offered-kilitlenmesi DEĞİŞİKLİK-YASAĞI-korundu (1b-tasarım-girdisi).
