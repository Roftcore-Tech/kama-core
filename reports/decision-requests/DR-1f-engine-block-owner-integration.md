# DR-1f — Engine block-owner PRODUCTION-entegrasyonu [DESIGN REGISTERED — IMPLEMENTATION NOT AUTHORIZED]

> Resmî-durum (Abkarada, 2026-07-22): **"DR-1f DESIGN REGISTERED — IMPLEMENTATION NOT AUTHORIZED."**
> Kaynak: "DR-1a MECHANISM CLOSED — PRODUCT INTEGRATION DEFERRED". Bu-iş 1a-MEKANİZMA-implementasyonuyla-
> AYNI-ŞEY-DEĞİLDİR ve hiçbir-faza-sessiz-gömülmez; kendi-onay-kapısı-vardır. Mevcut-durum: aktüasyon-
> yüzeyinin ÜRETİM-çağıranı-yok; kama-engine-üretim-lib'i adaptive-transport'u-yalnız-dev-dep-tutar.

## Kapanış-kriterleri (güçlendirilmiş-liste; talimat-§2-12-madde)

1. **`block_id ↔ witness-generation` GERÇEK-1:1-binding:** engine-block-registry witness'ı-blok-
   state'ine-bağlar; aynı-block_id'ye-ikinci-witness yapısal/typed-reddedilir.
2. **Generation-tüketim-doğrulaması:** latch/close witness-generation'ını-registry'yle-karşılaştırır;
   eşleşmeyen-generation typed-red (bugün: generation-taşınır-ama-tüketimde-doğrulanmaz — bilinen-sınır).
3. **Gerçek-production-block-registry/owner:** blok-yaşam-döngüsünün (open/add-source/close) üretim-
   sahibi kama-engine'de; adaptive-transport üretim-dep-ilişkisi-bu-DR'da-kararlaştırılır.
4. **`pending_fec` KESİN-cap:** bounded-VecDeque (SR-AUDIT-001); cap-değeri+overflow-politikası
   (red/evict+sayaç) tanımlı+testli.
5. **`manual_r` PROFILE-BOUND-clamp:** manuel-r ≤ profil-r_code_max (veya-açık-üst-sınır); clamp-
   görünür (diag).
6. **Deprecated-`enqueue_fec_parity` üretim-görünürlüğünden-ÇIKARILMASI:** `pub(crate)`/test-only
   (cfg-gate) VEYA tam-silme (harness'lar otorite-bağlı-close'a-migre). "pub+deprecated" SON-durum-
   DEĞİL — üretim-kapanış-kriteri.
7. **Product-runtime Controller-mode-seçimi:** admin/config-yüzeyi; default-Manual{false}-korunur.
8. **Production-telemetry:** FecActuationDiag+token-denial-snapshot ürün-telemetrisine-bağlanır
   (semantik-tablolar-otorite).
9. **Rollback:** Controller→Manual{false} tek-adım; açık-blok-block-stable-tamamlanır; runbook.
10. **Emitted-fakat-lost-parity cancellation-takibi:** decode-edilmiş-bloğun pending_lost'taki-moot-
    parity-retransmit'lerinin-iptali (Commit-K-adversarial-bulgusu m8) bu-DR'da-karara-bağlanır.
11. **Block-ID-aliasing-koruması:** id-reuse/wrap'te yanlış-blok-cancel'i (RECOVERED-cancel block_id-
    bazlı) generation-binding'le-kapanır.
12. **Gerçek-production N8/emission-continuity-matrisi:** emission_continuity-6.1-6.9 + N8-bit-özdeşlik
    ÜRETİM-block-owner-yoluyla-yeniden-koşulur (harness-değil-ürün-yolu-kanıtı).

## Generation-overflow DÜZELTMESİ (talimat-hükmü)

Bugünkü `saturating_add` YALNIZ generation-kullanılmadığı-için kabul-edilmiş-deferred-sınırdır.
Generation kimlik/binding-anahtarı-olarak-kullanılmadan-ÖNCE: **checked-increment + typed-exhaustion-
error + fail-closed** (yeni-witness-üretimi-reddedilir; mevcut-akış-bozulmaz) tanımlanacak.
**`u64::MAX`-üzerinde-sessizce-aynı-generation'ı-yeniden-üretmek KABUL-EDİLMEZ.**

## Ön-koşullar / bağlı-işler

- 1b (token/budget-politikası) ve 1d (dup↔parity) AYRI — bu-DR-emisyon-politikası-DEĞİŞTİRMEZ.
- Bu-turda (DR-1b-evidence-fazı) DR-1f-üretim-koduna-DOKUNULMAZ.

## Kapı

Onay + implementasyon + üretim-yolu-testleri (madde-12 + {A+B+C}) yeşil-olmadan "adaptif-FEC-ürün-
yolunda-etkin" SINIFI-DEĞİŞMEZ.
