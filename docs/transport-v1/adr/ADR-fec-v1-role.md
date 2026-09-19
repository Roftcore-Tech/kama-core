# ADR — FEC'in v1'deki rolü: doğrulanmış profil AYNEN sevk edilir, nedensellik iddiası YOK

**Status:** ACCEPTED (Faz-6, 2026-09-11) — SHIP + DOCUMENTED BEHAVIOR.

## Context

Faz-5 (990 koşu, N=30) Kama'nın hostile üstünlüklerini ölçtü (T1a, T6 DOMINATES) ama
aktivasyon telemetrisi şunu gösterdi:

| hücre | clamp (§8.5 kısıtladı) | parity_emitted | denied | hüküm |
|---|---|---|---|---|
| T1a %50 iid | 10 428 | **0** | 0 | KAMA DOMINATES |
| T1c %50+RTT | 12 230 | **0** | 0 | PARITY |
| T3a 8 kbit | 7 486 | **0** | 0 | PARITY |
| T2a gemodel | 5 525 | 19 | 1 560 | PARITY |
| T2b korelasyon | 12 982 | 1 | 141 | PARITY |
| T6 dönüş %80 | — | 3 | 81 | KAMA DOMINATES |

Altı hostile hücrenin beşinde arbiter parity'yi binlerce kez ÖNERDİ; §8.5 bant kısıtı küçülttü;
T1a/T1c/T3a'da hiç parity çerçevesi maddeleşmedi (ne emisyon ne ret). Desteklenen çıkarım:
kısıtlanmış oran K=8'de tek onarım sembolüne yetmedi (`compute_requested_r` → r=0).
Kama'nın kazandığı iki hücrede FEC katkısı sıfır (T1a) ya da ihmal edilebilir (T6, 3 sembol).

## Decision

1. **`KamaV1Profile` FEC ayarı AYNEN sevk edilir:** `fec = Some{profile 0x05 (K=8, R_max=4,
   E=1024), manual_r=None}` (adaptif), yapı-gate 250‰, spekülatif kol `bounded_v1`, §8.5 kısıtı
   sıfırlama-reddiyle (ADR-minimal-parity-exception). Hiçbir alan değişmez.
2. **Belgelenen ifade (normatif, dokümantasyon geneli):**
   > FEC Kama v1'de AÇIKTIR ve bazı rejimlerde katılır; ancak Faz-5 kanıtı FEC'i gözlenen
   > hostile üstünlüklerin NEDENSEL kaynağı olarak KURMAZ.
3. **Ek belgelenen davranış:** bazı FEC önerileri mevcut bant otoritesi (§8.5) tarafından
   maddeleşebilir onarım geometrisinin altına indirilir. Bu bir mimari/evrim notudur, Faz-6
   optimizasyon hedefi değildir.
4. Yasak ifadeler: "FEC avantajı sağlıyor", "FEC işe yaramıyor". İkisi de kanıtsız.

## Neden ayarlanmıyor / kapatılmıyor

* Ayarlama: Faz-5 sonrası kalibrasyon = ölçülen sistemi ölçülmemiş sisteme çevirir (INV-008).
* Kapatma: FEC'in katıldığı hücrelerde (T2a: üç kolun en düşük blackout insidansı 4/30,
  örtüşmeyen CI) zarar kanıtı yok; T3a'da aynı mekanizma KORUYUCU çıktı (dar banda israf yok,
  quinn'den %43 az tel). Aynı davranış iki rejimde zıt işaretli → tek etiketle karar verilemez.

## Alternatives considered

* §8.5 `budget_ref_bps` kalibrasyonu → yeni araştırma ekseni, yasak.
* K=8 yerine küçük K → profil değişikliği, tüm T-hücreleri geçersiz olur.
* FEC'siz "kama-v1-lite" profili → ikinci ürün kimliği; kanıtsız, kapsam dışı.

## Consequences

* Kama'nın üstünlüğü belgelerde **ARQ + mesaj-granülü teslim + veri-öncelikli kuruluş + yönlü
  geri-besleme** olarak atfedilir; FEC "katılan, nedensel olmayan" bileşen olarak kalır.
* Öneri→maddeleşme boşluğu Faz-6'da salt-gözlem sayacıyla kapatıldı
  (`FecActuationDiag::rounded_to_zero_count`); kontrol yolu okumaz (INV-007).

## Evidence

`reports/transport-v1/phase5/RESULTS-RUNNING.md` § "DÜZELTME + BULGU FEC-1";
`PHASE5-T0-T7-REPORT.md` §1, §19, §22 madde 2.

## Revisit conditions

Yalnız **nedenselliği karara bağlayabilecek** sınırlı/karşı-olgusal bir testle: "onarım
maddeleşmeseydi zamanında teslim zarar görürdü" gösterilirse. "Sayaç büyük görünüyor" gerekçe
DEĞİLDİR. Bkz. EVOLUTION §9.2, §9.4.
