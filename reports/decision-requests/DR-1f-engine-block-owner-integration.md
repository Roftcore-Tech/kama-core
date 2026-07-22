# DR-1f — Engine block-owner PRODUCTION-entegrasyonu [TASLAK — DEFERRED; NO-IMPL; ONAY-BEKLER]

> Kaynak: DR-1a authority-continuity-kapanışı ("DR-1a MECHANISM CLOSED — PRODUCT INTEGRATION
> DEFERRED"). Bu-iş 1a-MEKANİZMA-implementasyonuyla-AYNI-ŞEY-DEĞİLDİR ve hiçbir-faza-sessiz-gömülmez;
> kendi-onay-kapısı-vardır. Mevcut-durum: aktüasyon-yüzeyinin (mode-setter/witness/latch/manuel-close)
> ÜRETİM-çağıranı-yok; kama-engine-üretim-lib'i adaptive-transport'u-yalnız-dev-dep-tutar.

## Kapsam (kapanış-kriterleri)

1. **Gerçek-block-open'da-authority:** engine-block-owner blok-açarken `open_fec_block_authority()`
   çağırır; witness blok-state'ine-bağlanır (block_id ↔ witness 1:1; generation-binding-anahtarı
   burada-doğrulanır-hale-gelir — bilinen-sınırın-kapanışı).
2. **Block-close'da by-value-latch:** `fec_controller_latch(witness, gerçek-snapshot, k_actual,
   r_code_max, block_id, parity_size)` / manuel-yol `close_fec_block_manual` — tek-tüketim.
3. **Planned-parity-taşıma:** staging→emisyon aynı-block-authority-altında (üyelik=izin-invariantı
   üretim-yolunda-doğrulanır); `enqueue_fec_parity`-legacy-yolu-ÜRETİMDE-KULLANILMAZ (deprecated).
4. **Mode-transition-etkisi:** yalnız-sonraki-blok (block-stable; emission_continuity-matrisi-üretim-
   çağıranıyla-yeniden-koşulur).
5. **Runtime-config/admin Controller-seçimi:** mode-seçiminin-ürün-yüzeyi (admin-plane/config) —
   default-Manual{false}-korunur; rollback-yolu-tanımlı.
6. **Production-telemetry:** FecActuationDiag-alanlarının-ürün-telemetrisine-bağlanması (semantik-
   tablo dr-1a-review-hardening.md §6 + Commit-K-supersession-notu).
7. **Rollback:** Controller→Manual{false}-geri-dönüşü-tek-adım; açık-blok-block-stable-tamamlanır.

## Ön-koşullar / bağlı-işler

- SR-AUDIT-001-bound'ları: `pending_fec`-cap + `manual_r`-clamp (Commit-K-deferred-minor'ı) BU-DR'da.
- pending_lost'taki-emitted-ama-lost-parity'nin decoded-blok-cancel'i (doğal-takip) değerlendirilir.
- block_id-aliasing: generation-binding-tüketim-doğrulaması (witness↔block-registry) BU-DR'da.
- 1b (token/budget-politikası) ve 1d (dup↔parity) AYRI — bu-DR-emisyon-politikası-DEĞİŞTİRMEZ.

## Kapı

Onay + implementasyon + üretim-yolu-testleri (emission_continuity-üretim-varyantı + N8 + {A+B+C})
yeşil-olmadan "adaptif-FEC-ürün-yolunda-etkin" SINIFI-DEĞİŞMEZ.
