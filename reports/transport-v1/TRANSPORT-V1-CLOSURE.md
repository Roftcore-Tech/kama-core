# Kama Transport v1 Closure

## 1. Nihai hüküm

**TRANSPORT V1 READY** — architecture, product profile, invariants, limitations, and validation
evidence frozen.

## 2. Donmuş sürüm

| | |
|---|---|
| dondurma commit'i | `3a1ca5cf06c2213194832e1e904f06bb876bf7a6` (bu rapor onu izleyen commit'tedir) |
| dal | `audit/transport-v1-closure` (origin/main `75d8990a`'nın 38+ commit ilerisinde; PR ile merge sahip onayına bağlı) |
| Faz-5 ölçüm ağacı | `e3c1e8bb` — Faz-6 transport karar mantığına DOKUNMADI (yalnız salt-gözlem sayacı + kimlik sabitleri) |
| kanonik profil kimliği | `KamaV1Profile` → benchmark `kama-v1-full` (`client_core::runtime::TRANSPORT_V1_PROFILE_ID`) |
| mimari kimliği | `transport-v1` (`TRANSPORT_V1_ARCHITECTURE`) |
| spec | `specs/adaptive-transport-engine-spec.md` + `specs/transport-layer-tdd.md`, AMENDMENT-1..3 ile (hash yok — repo spec'leri hash'lemiyor) |
| benchmark sözleşmesi | `reports/transport-v1/BENCHMARK-METHODOLOGY.md` (Faz-4, donmuş) · manifest şeması `kama-bench-manifest/v1` |
| etiket | OLUŞTURULMADI (yetki verilmedi). Dondurma için amaçlanan commit: `3a1ca5cf06c2213194832e1e904f06bb876bf7a6`; commit edilmemiş ağaç "donmuş sürüm" sayılmaz. |

## 3. Ne doğrulandı (Faz 1–5)

Faz-1 dönüş-yolu gözlenebilirliği (INV-RETURN-1, R0–R5) · Faz-2 FEC spec-tamlığı (`ab597ab1`:
kısmi blok Model-A, §8.5, repair fate, MinimalParity kablolama) · Faz-3 aktivasyon telemetrisi
(`7e0a9261`: configured≠reachable≠fired) · Faz-4 metodoloji (`b1049e39`: kanonik profil,
manifest, TCP kolu, kurulum muhasebesi, blackout/sonuç/istatistik, 4 ölçüm kusuru) · Faz-5
T0–T7 (990 koşu, N=30, ERRATUM-1 sonrası sıfırdan). Raporlar: `PHASE5-T0-T7-REPORT.md`,
`PHASE4-REPORT.md`, `../transport-v1-audit/`, `../phase{1,2,3}-*/`.

## 4. Faz-5 nihai matris

| hücre | Kama | quinn | TCP | hüküm |
|---|---|---|---|---|
| T0 temiz | 1.000 | 1.000 | 1.000 | PARITY (regresyon yok) |
| T1a %50 iid | **1.000** [1,1] | 0.900 [0.53,1] | 0.120 | **KAMA DOMINATES** |
| T1b %80 iid | 0.010 | 0.000 | 0.000 | KAMA WINS (göreli; üçü de başarısız) |
| T1c %50 + 200 ms | 1.000 | 1.000 | 0.830 | PARITY |
| T2a gemodel burst | 1.000 | 1.000 | 0.770 | PARITY (süreklilikte Kama: blackout 4/30 ↔ 25/30) |
| T2b korelasyon burst | 1.000 | 1.000 | 0.430 | PARITY (eski n=6 iddiası GERİ ÇEKİLDİ) |
| T3a 8 kbit | 1.000 | 1.000 | 1.000 | PARITY |
| T3b 0.5 kbit sonda | — | — | — | SPEC CONFLICT NOT REACHED |
| T4 200 ms RTT | 1.000 | 1.000 | 1.000 | PARITY |
| T5 asimetrik bant | 1.000 | 1.000 | 1.000 | PARITY (zayıf kanıt) |
| T6 dönüş %80 kayıp | **1.000** [1,1] | 0.740 [0.21,0.99] | 0.060 | **KAMA DOMINATES** |
| T7 kayıp + reorder | 1.000 | 1.000 | 1.000 | PARITY |

## 5. Doğrulanmış birincil iddialar (temkinli, kanıta bağlı)

* Sağlıklı yolda patolojik regresyon yok (T0/T4/T5: parite, blackout 0/30).
* Yüksek rastgele kayıpta güçlü teslim (T1a: en kötü koşu 0.970 > quinn medyanı 0.920; felaket 0/30).
* Bozuk dönüş yolunda güçlü teslim (T6: örtüşmeyen CI; quinn 9/30 hiç bağlanamadı).
* Yönlü ileri/dönüş bozulma ayrımı çalışıyor (T6: enjekte %5/%80 → ölçülen 45.5‰/793‰).
* Kayıp/reorder ayrımı çalışıyor (T7: `spurious=1`, fwd_loss 200‰ = enjekte).
* İlan edilmiş zarfta materyal TCP/QUIC üstünlüğü GÖZLENMEDİ (11/11 hücre).
* Yakınsama hızı: dar deadline'da Kama önde (T1c/T2b/T3a D=5 s) — davranış farkı, üstünlük iddiası değil.

## 6. Açıkça YAPILMAYAN iddialar

* "Kama QUIC'i her zaman yener" — HAYIR (8 hücre parite; T2a'da P=0.438).
* "Kama TCP'yi her zaman yener" — HAYIR (T0/T4/T5/T7 parite).
* "FEC gözlenen kazançların nedeni" — HAYIR (T1a parity=0; LIM-002, ADR-fec-v1-role).
* "Sıfır-geri-besleme kuruluşu garanti" — HAYIR (T1b 18/30; LIM-001).
* "Doygun dönüş-bant asimetrisi tam doğrulanmış" — HAYIR (T5 %3.5 kullanım; LIM-004).
* "TCP'ye karşı tam-tel maliyet kıyası" — HAYIR (çekirdek trafiği görünmez; LIM-005).
* "§11 acil yolu sahada çalışıyor" — HAYIR (330/330 koşuda ateşlemedi; LIM-006).

## 7–10. Donmuş profil · invaryantlar · sınırlar · evrim

`docs/transport-v1/PRODUCT-PROFILE.md` · `INVARIANTS.md` (INV-001..016) ·
`KNOWN-LIMITATIONS.md` (LIM-001..007) · `EVOLUTION.md` (§9.1–9.7, §22 politikası).
Mekanizma denetimi (SHIP/KEEP-EXP/DELETE, WIRE defteri kapanışı, sahip karar tablosu):
`FINAL-MECHANISM-AUDIT.md` — hiçbir kalem TBD değil.

## 11. Transport → kripto hazırlığı

Transport sözleşmesi gelecek güvenlik katmanı için **kararlıdır**: birim geometrisi (≤1 013 B,
E 1 024, prefix 11, outer 26), sırasız mesaj-granülü güvenilir teslim, veri-öncelikli ilk-veri,
opak eş, metadata sınıflandırması, replay/dedup ayrımı ve arıza yayılımı
`docs/transport-v1/TRANSPORT-CRYPTO-HANDOFF.md`'de belgelidir. Kripto tasarımı/uygulaması
BU GÖREVDE YAPILMADI ve bu belge onu tanımlamaz.

## 12. Nihai kapı (dondurma ağacı 3a1ca5cf06c2213194832e1e904f06bb876bf7a6)

```
cargo fmt --all --check                                              OK
cargo clippy --workspace --all-targets --all-features -- -D warnings   0 hata
cargo test  --workspace --all-features         1264 geçti / 0 başarısız / 18 ignored (168 ikili)
cargo test  -p bench --all-features              58 geçti / 0 başarısız
gecelik netem kapısı                             INFRA WAIVER (NIGHTLY-INFRA-WAIVER.md)
```
18 ignored = gecelik-only sim testleri (Faz-6 kaynaklı değil). Faz-5 kapısına göre +7 test:
6 freeze + 1 diag-contract.

## 13. Hüküm kuralı kontrolü (Faz-6 §20, 14 madde)

Faz 1–5 kapalı ✔ · KamaV1Profile donmuş ✔ (pin testleri) · v1-gerekli açık WIRE yok ✔ ·
açıklanmamış default-OFF yok ✔ (her biri ADR/LIM) · §8.5/§11 normatif çözüldü ✔ (AMENDMENT-3) ·
sıfır-ACK sınıflandırıldı ✔ · FEC rolü nedensel iddiasız ✔ · sınırlar açık ✔ · zarf = Faz-5
kanıtı ✔ · invaryantlar ✔ · transport→kripto sınırı ✔ · metodoloji donmuş ✔ · kapı yeşil +
gecelik için feragat ✔ · yeni araştırma ekseni yok ✔.

**TRANSPORT V1 READY — architecture, product profile, invariants, limitations, and validation evidence frozen.**
