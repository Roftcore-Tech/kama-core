# ADR — Transport v1 mimari/profil DONDURMASI

**Status:** ACCEPTED (Faz-6, 2026-09-11).

## Context

Faz 1–5 kapandı: dönüş-yolu gözlenebilirliği (Faz-1), FEC spec-tamlığı (Faz-2 `ab597ab1`),
aktivasyon telemetrisi (Faz-3 `7e0a9261`), kıyas metodolojisi (Faz-4 `b1049e39`), T0–T7 hostile
doğrulama (Faz-5, ölçüm SHA `e3c1e8bb`, 990 koşu, N=30). Hiçbir hücrede materyal TCP/QUIC
üstünlüğü yok; T6 birincil ürün iddiasını örtüşmeyen CI ile doğruladı.

## Decision

`KamaV1Profile` + `adaptive-transport` karar mantığı + engine/TDD spec'leri (AMENDMENT-1..3 ile)
+ Faz-4 metodoloji sözleşmesi **DONDURULMUŞTUR.** Dondurma commit'i:
`reports/transport-v1/TRANSPORT-V1-CLOSURE.md` §2'de beyan edilen SHA (bu ADR'nin bulunduğu
kapanış commit'i). Kimlikler tek kaynaktan: `client_core::runtime::TRANSPORT_V1_PROFILE_ID`
(`kama-v1-full`), `TRANSPORT_V1_ARCHITECTURE` (`transport-v1`), bench şeması
`kama-bench-manifest/v1`.

**"Donmuş" şu demektir:**
1. Transport runtime kararını değiştiren HİÇBİR değişiklik `kama-v1-full` kimliğini koruyamaz.
2. Böyle bir değişiklik EVOLUTION §9.5 / §22 politikasındaki altı beyanı yapmak ve etkilenen
   T-hücrelerini ADR-hostile-validation-contract ile yeniden koşmak ZORUNDADIR; kimlik
   `transport-v1.x` ya da `transport-v2` olur.
3. İzin verilen değişiklik sınıfları: dokümantasyon, normatif spec netleştirme
   (`[AMENDMENT-N]` deseni, kod-spec tutarlılığı için), salt-gözlem tanı, sürüm/release metadata,
   dondurma/invariant testleri, CI tesisatı.
4. Freeze testleri (`crates/client-core/tests/transport_v1_freeze.rs`, `product_profile_v1.rs`,
   bench kimlik testleri) profil alanlarını pinler; birini değiştirmek = sevk kararı.

## Alternatives considered

* "Faz-6'da küçük iyileştirmeler" → 990 koşuluk kanıtı geçersiz kılar; reddedildi.
* Dondurmayı `main`'e merge'e ertelemek → kanıt ile kod arasındaki bağ zayıflar; dondurma
  branch commit'inde beyan edilir, merge sahip onayıyla ayrıca yapılır.

## Consequences

* Kama Transport v1 artık bir **belgelenmiş mimari sözleşmedir**: SPEC = MİMARİ = PROFİL =
  UYGULAMA = INVARYANTLAR = KANIT = SINIRLAR (docs/transport-v1/).
* Gelecek kripto/güvenlik katmanı `TRANSPORT-CRYPTO-HANDOFF.md`'yi alt-katman sözleşmesi olarak
  kullanır; transport bu katmanın MÜŞTERİSİ olarak tasarlanmıştır (INV-ENGINE-SOC).
* Transport çalışması DURUR. Kripto uygulaması bu görevde BAŞLAMAZ.

## Evidence

`reports/transport-v1/TRANSPORT-V1-CLOSURE.md`, `PHASE5-T0-T7-REPORT.md`, `PHASE4-REPORT.md`,
`reports/transport-v1-audit/`, `reports/phase{1,2,3}-*/`.

## Revisit conditions

Yalnız yeni bir sürüm kimliği altında (v1.x/v2), EVOLUTION.md'deki tetikleyicilerle.
