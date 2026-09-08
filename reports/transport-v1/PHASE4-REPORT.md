# FAZ 4 — Benchmark Metodoloji Temizliği / Adillik / Kanonik Ürün Profili

Tarih: 2026-09-08 · Dal: `dev/product-transport-fusion` · Kapsam: yalnız `crates/bench`,
gecelik iş akışı ve netem betiği. **Transport karar mantığı DEĞİŞMEDİ** (kanıt: aşağıda §D).

> Faz-4 kuralı gereği bu belge **hiçbir performans sonucu içermez**. Aşağıdaki her sayı ya bir
> enstrüman ölçümüdür (kurulum süresi, sayaç) ya da bir kanıt kontrolüdür.

Sözleşme belgesi: [`BENCHMARK-METHODOLOGY.md`](BENCHMARK-METHODOLOGY.md) (20 bölüm).

---

## A. Metodoloji temizliğinin bulduğu GERÇEK kusurlar

Faz-4 sırasında üç ölçüm kusuru ortaya çıktı. Üçü de **ölçüm katmanındadır**; hiçbiri
transport davranışına dokunmadan düzeltildi.

**A1 — Kurulum sınıfı sessizce karışıyordu.** Üç kol (Kama, quinn, qdgram) ölçüm saatini
bağlantı kurulumundan SONRA başlatıyordu; Faz-4'te eklenen TCP kolu ise kurulumu ölçüm
penceresinin İÇİNDE ödüyordu. `--setup-class` bayrağı ise denetlenmeyen serbest bir iddiaydı:
operatör `cold-flow` yazıp `warm` bir koşu raporlayabilirdi. Düzeltme: kural dört kolda
birleştirildi (kurulum pencere DIŞINDA, `setup_ns` ayrı ölçülür), sınıf kolun **kodundan**
türetilir (`RunMeta.setup_in_window`), operatör iddiası yalnız denetlenir ve çelişirse koşu
**manifest yazmadan** hata verir. Uçtan-uca görünüm `setup_ns` üzerinden türetilir.

**A2 — Kurulum maliyeti üç kolda hiç ölçülmüyordu.** `setup_ns` yalnız TCP'de doluydu;
diğerlerinde sabit `0` yazıyordu. Artık dördü de ölçüyor (el sıkışma başarısız olsa bile
harcanan süre raporlanır). Ölçülen tipik değerler: Kama ≈ 37–61 µs, TCP ≈ 0.15 ms,
quinn ≈ 2.3 ms, qdgram ≈ 1.6 ms (loopback; kurulum maliyetidir, performans değil).

**A3 — Kıyas denetimi tek düzeydeydi.** İlk yazımda anlam farkı (güvenilirlik/sıra) ile deney
koşulu farkı (yük/ağ/deadline) aynı listede toplanıyordu → §4.4'ün ana kıyası (Kama ↔ quinn)
her zaman "GEÇERSİZ" çıkıyordu, araç gürültüye dönüşüyordu. Üç düzeye ayrıldı: `BLOCKING`
(koşul farkı, çıkış 2) · `NON-NORMALIZED` (anlam farkı, çıkış 1, damga zorunlu) ·
`NORMALIZED` (çıkış 0).

**A4 — resmî kıyas yolunun FEC profili ürün sabitine BAĞLI DEĞİLDİ.** `--fec-profile`
bayrağının varsayılanı literal `5` idi. Bugün `KamaV1Profile::FEC_PROFILE_ID` de 0x05 olduğu
için manifest doğru şekilde `kama-v1-full` diyordu — yani kusur ÖLÇÜMDE görünmüyordu. Ama ürün
profili sabiti değişseydi bayraksız kıyas yolu sessizce ürün-DIŞI bir yapılandırma ölçmeye
devam eder, manifest ise (delta boş olduğu için değil, sabit eşleştiği için) yine `kama-v1-full`
derdi. Varsayılan artık doğrudan `KamaV1Profile::FEC_PROFILE_ID`'ye bağlı. Bu kusuru Faz-4'ün
kendi birim testi (`bayraksiz_kiyas_yolu_kanonik_profildir`) ortaya çıkardı — duman testleri
DEĞİL, çünkü duman testinde iki değer tesadüfen eşitti.

Ayrıca **kendi test setim iki hatamı yakaladı** ve ikisi de düzeltildi: `binom_cdf` bir fazla
terim ekliyordu (0/10 Clopper–Pearson üst sınırı 0.445 çıktı, doğrusu 0.308) ve `p = 1`
sınırında `p/(1−p)` NaN üretip bisection'ın yön tespitini bozuyordu (10/10 alt sınırı 4e-22).

**Metodoloji temizliği hiçbir transport doğruluk/kablolama hatası ortaya çıkarmadı** —
dolayısıyla Faz-4'ün "bulursan DUR ve blocker olarak raporla" kuralı tetiklenmedi.

---

## B. Yapılan işler (madde madde)

| iş | dosya |
|---|---|
| Kanonik profil kimliği koddan türetilir (`kama-v1-full` / `kama-custom`) | `bench/src/kama_product.rs` |
| Yapılandırılmış koşu manifesti (JSON) + anlam sözleşmesi | `bench/src/manifest.rs` (yeni) |
| Üç düzeyli kıyas denetimi + `bench fairness` alt komutu | `bench/src/manifest.rs`, `main.rs` |
| Belirsiz kol adı yasağı (`quinn` → `quinn-reliable-stream` / `quinn-datagram-app-retry`) | `bench/src/manifest.rs` |
| TCP taban kolu (normal yığın, minimum simetrik çerçeveleme) | `bench/src/tcp.rs` (yeni) |
| Kurulum maliyeti: dört kolda ölçüm + türetilmiş sınıf + iki görünüm | `bench/src/{tcp,quic,qdgram,kama_product,kama}.rs`, `outcome.rs` |
| Sonuç sınıfları · blackout · katmanlı istatistik (N≥30, CI, stall) | `bench/src/outcome.rs` (yeni) |
| Okunamayan koşu artık sessizce silinmiyor (harness/infra sınıfı) | `bench/src/main.rs` |
| Gecelik ADR-18: manifest kapısı, sapmada `BENCHMARK_FAILURE` | `.github/workflows/nightly.yml` |
| netem kanıt betiği: manifest + ayrı ileri/dönüş bozulma + fairness | `scripts/netem/v1_activation_proof.sh` |
| Metodoloji sözleşmesi (20 bölüm) | `reports/transport-v1/BENCHMARK-METHODOLOGY.md` |

---

## C. Metodoloji duman testleri (M0–M5) — kanıt

Hepsi sınırlı; **hiçbiri performans iddiası üretmez**.

**M0 — profil kimliği.** Bayraksız kanonik koşu → `arm=kama-v1-full`, `profile_delta=[]`.
Tek alan mutasyonu (`--initial-window-bytes 1200`) → `arm=kama-custom`,
`profile_delta=["initial_window_bytes 1200 != 2400"]`. Deterministik.

**M1 — manifest tamlığı.** Tek Kama koşusu hem yapılandırma hem aktivasyon bölümünü üretti
(19 aktivasyon alanı; `tail_configured=true fec_configured=true`, kestirim alanları dolu).

**M2 — quinn kolu.** Bağlandı, koştu, anlamını bildirdi (`quinn-reliable-stream`,
`reliable-ordered-stream`, teslim = "stream write acknowledged by peer"), uygulama düzeyi
teslim metriği üretti.

**M3 — TCP kolu.** Bağlandı, mesajı çerçeveledi, koştu, uygulama teslimini ve kurulum
kategorisini bildirdi (`tcp-default`, 120/120 teslim, `setup=152602ns`).

**M4 — kurulum muhasebesi.** `--setup-class cold-flow` iddiası ile harness gerçeği
(`setup-excluded`) çeliştiğinde koşu hata verdi ve **manifest yazılmadı** (doğrulandı: dosya
yok). Sessiz karışım yapısal olarak imkânsız.

**M5 — asimetrik harness (gerçek netem, üç netns).** İleri ve dönüş bozulmaları ayrı verildi,
manifeste ayrı kaydedildi ve **fiilen uygulandığı bağımsız olarak doğrulandı**: runtime'ın
kendi dönüş-yolu gözlemcisi hücre başına farklı ileri/dönüş bozulması ölçtü.

| hücre | ileri netem | dönüş netem | ölçülen `fwd_loss` | ölçülen `ret_ack_loss` |
|---|---|---|---|---|
| clean | `delay 100ms rate 1mbit` | aynı | 4‰ | 0‰ |
| loss20 | `loss 20%` | `loss 2%` | 192‰ | 53‰ |
| burst50 | `loss 50% 50%` | `loss 10%` | 548‰ | 448‰ |

Aynı koşularda aktivasyon sayaçları da boş değil (burst50: 7 tail probe, 19 parity,
215 retransmit, 30 dup) — manifest vakum sayaç taşımıyor. `fairness` üç manifesti
`BLOCKING` işaretledi (farklı ağlar), yani araç farkı görüyor.

---

## D. Çıkış ölçütleri

| # | ölçüt | durum |
|---|---|---|
| 1 | Resmî yol kanonik ürün profilini kullanır | ✔ `KamaV1Profile::runtime_config()` |
| 2 | Elle sapma "Kama v1" etiketi alamaz | ✔ M0 |
| 3 | `kama-v1-full` / `kama-custom` kimliği deterministik | ✔ M0 + birim testi |
| 4 | Yapılandırma manifesti var | ✔ M1 |
| 5 | Faz-3 aktivasyon manifesti entegre | ✔ M1 (19 alan) |
| 6 | Uygulama düzeyi teslim anlamı normalize | ✔ §4 |
| 7 | Güvenilirlik anlamı açık | ✔ §5 |
| 8 | Sıra anlamı açık | ✔ §5 |
| 9 | Kurulum muhasebesi açık ve simetrik | ✔ §6, A1/A2, M4 |
| 10 | TCP tabanı var ve koşuyor | ✔ M3 |
| 11 | Quinn modu açıkça adlandırılmış | ✔ §15 politikası + birim testi |
| 12 | İleri/dönüş bozulmaları ayrı temsil ve UYGULANIR | ✔ M5 |
| 13 | Blackout metriği tanımlı | ✔ §11 + 3 birim testi |
| 14 | Arıza/timeout anlamı tanımlı | ✔ §12 + birim testi |
| 15 | Tel maliyeti muhasebesi belgeli (sınırlarıyla) | ✔ §10 |
| 16 | Rastgelelik/seed politikası belgeli | ✔ §13 |
| 17 | Nihai iddia kuralı N≥30 + CI | ✔ §14–15, otomatik `claim class` |
| 18 | Stall işlemesi tanımlı | ✔ §16, üç sayı birlikte |
| 19 | ADR-18 gecelik gerçek kanonik profili kullanır | ✔ manifest kapısı |
| 20 | Metodoloji duman testleri geçiyor | ✔ M0–M5 |
| 21 | Tam PR kapısı yeşil | ✔ §E |
| 22 | Transport karar mantığı değişmedi | ✔ `git status`: `crates/adaptive-transport`, `client-core`, `kama-*` DOKUNULMADI |

---

## E. Kapı çıktısı

```
cargo fmt --all --check                                            OK
cargo clippy --workspace --all-targets --all-features -- -D warnings   0 hata
cargo test  --workspace --all-features                             1256 geçti / 0 başarısız
                                                                   / 18 ignored (167 test ikilisi)
cargo test  -p bench --all-features                                  57 geçti / 0 başarısız
```

18 `ignored` FAZ-4'ten GELMEZ; gecelik-only işaretli mevcut sim testleridir (`--ignored` ile
gecelik iş akışında koşarlar). Sayı gizlenmiyor, olduğu gibi raporlanıyor.

Faz-4 metodoloji birim testleri — **20** (koşu çıktısından sayıldı, tahmin değil):

* `manifest` **6** — `m0_profil_kimligi_deterministik` · `anlam_farki_beyan_kosul_farki_engel` ·
  `ayni_kol_iki_kosu_farksiz` · `kol_adlari_belirsiz_degil` ·
  `tcp_tel_muhasebesi_siniri_beyan_edilir` · `tel_muhasebesi_farki_beyan_edilir`
* `outcome` **11** — blackout 3 (`akis_sonu_sessizligi_blackout_degil`,
  `ortadaki_kesinti_yakalanir_ve_kurtarma_olculur`, `kurtarma_yoksa_none`) · sonuç sınıfı 2
  (`sonuc_siniflari_ayrilir`, `sinif_onceligi_sabittir`) · istatistik 4 (`kucuk_n_smoke_kalir`,
  `medyan_ci_deterministik_ve_kapsayici`, `kesin_oran_araligi_sifirda_bile_bilgi_verir`,
  `stall_katmanlamasi_gizlemez`) · kurulum 2 (`kurulum_gorunumu_simetrik_turetilir`,
  `kurulum_sinifi_sessizce_karisamaz`)
* `kama_product` **2** — `bayraksiz_kiyas_yolu_kanonik_profildir` · `tek_alan_sapmasi_gorunur`
* `tcp` **1** — `cerceveleme_mesaj_anlamini_korur`

Ayrıca mevcut ürün-yolu testine (`full_arm_server_and_client_run_end_to_end_with_fec`)
aktivasyon-manifesti tüketimi ve canlı kimlik doğrulaması eklendi.

---

## F. Faz-4'te KASITLI OLARAK YAPILMAYANLAR

* T0–T7 hostile kampanyası koşulmadı (Faz-5).
* `parity_denied ≫ parity_emitted` gözlemine politika tepkisi verilmedi; sayaç yalnız manifeste
  taşındı. Soru Faz-5 T2'ye aittir.
* FEC araştırması yeniden açılmadı.
* Kripto işine başlanmadı.
* Kayıtlı iki borç (§8.5 alt-kbps clamp politika çelişkisi; RECOVERED kaynak-birim iptali
  kapsam eksiği) Faz-3/4 kapsamı dışında bırakıldı — güvenilmez assertion uydurulmadı.
