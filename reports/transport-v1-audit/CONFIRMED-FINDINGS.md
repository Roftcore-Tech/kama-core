# TRANSPORT v1 DENETİMİ — BİZZAT DOĞRULANMIŞ BULGULAR (grep kanıtlı)

Bunlar ajan çıktısı değil; her biri kaynak koddan doğrudan doğrulandı. Ajan bulguları `phase1-raw/`,
adversaryal doğrulama sonrası sınıflandırma `COVERAGE-MATRIX.md`'de.

## B-1 (BLOCKER adayı) — Ürün sevk profilinde FEC KAPALI; her "Kama" kıyası FEC'siz koştu

`RuntimeConfig::default()` → `fec: None` ([runtime.rs:199](crates/client-core/src/runtime.rs#L199)).
Doküman: "`None` = KAPALI (sevk default'u; flip **S1-GATE kanıtına** bağlı)" (runtime.rs:133-136).
Tüm FEC aktüasyonu `fec.as_ref()`'e kapılı (runtime.rs:978, 1244, 1263, 2180, 2220, 2348) → profil yokken
blok açılmaz, parity üretilmez, alıcı decode etmez.

**Sonuç:** bugünkü "Kama v1 ürün profili" = ARQ + tail-recovery + cold-start, **FEC YOK**. FEC v1 kapanışının
(bounded_v1 spekülatif kol, `fec_loss_risk`) ürün default'unda ölçülebilir hiçbir etkisi yoktur — oran
hesaplanır, aktüasyon olmaz. Kapanış raporundaki "enabled with bounded C4-safe policy" ifadesi **kolun**
açık olduğunu söyler, FEC'in kendisinin değil.

**Benchmark etkisi:** `scripts/netem/product_vs_quinn.sh:76` `kp-sevk` kolu = `kama-product` (FEC bayrağı YOK)
→ Quinn kıyasında "sevk davranışı" diye ölçülen kol FEC'siz. `kp-iw-bs` kolunun üstündeki yorum
"# İkisi birlikte + FEC: tam ürün." yanıltıcı: o kolda da `--fec-profile` yok (satır 84). FEC'li iki kol
(`kp-static-r4`, `kp-iw-bs-static`) **statik r=4** kullanır; adaptif Controller + spekülatif kol hiçbir
Quinn kıyasında koşmamıştır.

**Kanıt durumu:** S1-GATE (`reports/s1-gate/S1-GATE-RESULT.md`, 2026-08-26) birincil kapıyı GEÇMİŞ:
"NO-HARM (birincil gate — PASS): ürün FEC hiçbir hücrede OFF'a göre gerilemedi", burst h3b'de FEC-static
%100 vs ARQ-only %99. Yani flip'i bloke eden kanıt eksikliği **artık yok**; flip yapılmamış.

## B-2 (BLOCKER adayı) — Kısmi/son blok kodlanmıyor: kuyruk birimleri FEC görmüyor

Codec Model-A'yı destekliyor (`kama-fec-codec/src/profile.rs:15` "K_actual ≤ K_shape; zero-slot pad",
`error.rs:22-34` known_zero yolları), ama ürün runtime'ı **yalnız tam blok** kapatıyor:
`runtime.rs:1277` `if cur_block.len() == p.k_shape_usize()` ve `runtime.rs:999`/`:1294`
`k_actual = k_shape` (known_zero hep 0). Doküman bunu kabul ediyor: runtime.rs:172-173 "S1: tam-blok
k_actual = k_shape, known_zero = 0".

**Sonuç:** K=8'e ulaşmayan mesaj kuyrukları ve kısa akışlar FEC AÇIKKEN bile korumasız. Hostile
rejimin birincil vakası (kısa akış + kayıp) FEC'in yapısal olarak ulaşamadığı yer.

## B-3 (WIRE adayı) — ACK_FRAME_V2 `seen_count` telde taşınıyor, hiçbir tüketicisi yok

Üretim: `ack.rs:74/146/226` (her V2 SACK'te 6 B ölçüm eki). Tüketim: YOK — `sender.rs:107` ve `:3217`
açıkça "seen_count/arrival_gap SENDER'A-TAŞINMAZ (ledger-only; karar-girdisi-değil)".
Spec §2.2 alanı "K3 cold-start yön-ayrımı" için tanımlar.

**Sonuç:** ürünün **ileri-yön kaybı ile dönüş-yolu kaybını ayırt edecek hiçbir sinyali yok**; asimetrik
yol ve bozulmuş dönüş yolu (T5/T6) birincil hedef rejimler olmasına rağmen. Her ACK'te ödenen 6 B tel
maliyeti karşılıksız.

## B-4 (DELETE/WIRE ayrımı) — `kama_engine::admit_and_decode` ürün yolundan çağrılmıyor

Tip-kapılı decode köprüsü (`kama-engine/src/lib.rs:32`) yalnız kendi testlerinden ve
`client-core/tests/api_surface_v0.rs` pininden çağrılıyor. Ürün alıcısı `runtime.rs` içindeki
`fec_ingest`'i kullanıyor → aynı işi yapan **iki ayrı alıcı decode hattı**.

## B-5 (BLOCKER — benchmark geçerliliği) — Bench ürün profilini KOŞMUYOR; "sevk" etiketi yanlış

`crates/bench/src/kama_product.rs:140-215` `runtime_cfg()` `RuntimeConfig`'i **sıfırdan struct-literal**
olarak kurar; `RuntimeConfig::default()`'ı hiç okumaz. Ürün default'ları değiştiğinde bench sessizce eski
değerlerde kalır. Ölçülen sapmalar:

| alan | ÜRÜN default (runtime.rs:190-206) | BENCH `kama-product` default | yön |
|---|---|---|---|
| `initial_window_bytes` | `DEFAULT_BURST_CAP` = **2400** | CLI `default_value_t = 0` (main.rs:123) → **0** | bench DAHA KÖTÜ (ürünün "clamp hatası" diye düzelttiği eski davranış) |
| `fec_loss_risk` | `bounded_v1()` (**enabled**) | `--fec-risk` yoksa `FecLossRiskConfig::default()` (**disabled**) | bench spekülatif kolu HİÇ koşmaz |
| `structure_gate_level_permille` | `Some(250)` | `None` → motor `fec_med` = **500** | farklı parity seviyesi |
| `fec` | `None` | `None` (bayrak yoksa) | eşleşiyor (ikisi de FEC'siz — bkz. B-1) |

Bench yorumları bu değerleri hâlâ "sevk davranışı" diye etiketliyor (kama_product.rs:161 "0 = sevk
davranışı (ilk uçuş tek paket)", :186 "false = sevk davranışı (semantik özdeş)") — **ikisi de artık yanlış**.

**Ölçülü etki (`reports/product-vs-quinn-v3/`, n=6, kendi hesabım):**

| kol | iid50 completion (medyan) | burst50 | tel (medyan) |
|---|---|---|---|
| `kp-sevk` ("sevk" etiketli) | **0.500** [0.01, 1.00] | **0.495** [0.01, 1.00] | 25.6 / 27.7 kB |
| `kp-iw2400` (yalnız IW=2400 = ürün default'u) | 0.990 | 0.995 | 54.5 / 57.7 kB |
| `kp-bootstrap` | 0.990 | 0.985 | 52.9 / 53.8 kB |
| `kp-iw-bs` | 1.000 | 0.980 | 56.9 / 61.5 kB |
| `kp-static-r4` (FEC statik r=4) | 0.995 | 1.000 | 120.0 / 114.9 kB |

Yani Quinn kıyasında "Kama sevk davranışı" diye sunulan kol, ürünün **bugün sevk ettiği** yapılandırmadan
iki eksende geride ve tam ikiye bölünmüş (0.01 ↔ 1.00 bimodal); ürün default'unu koşan `kp-iw2400` aynı
hücrede 0.99. **Bu, "Kama Quinn'e kaybediyor" sonucunun geçerliliğini doğrudan çürütür** (sınıf F/G).

**Gerekli düzeltme (görev zaten şart koşuyor):** (i) bench `RuntimeConfig::default()`'tan başlayıp yalnız
açık override uygulasın (tek doğruluk kaynağı); (ii) koşu başında **mimari-aktif manifest** basılsın
(commit, profil, açık/kapalı mekanizmalar ve nedenleri), koşu sonunda **fiilen aktive olan** mekanizmalar.

## B-6 (SONUÇ TERSİNE DÖNÜYOR) — "Kama Quinn'e kaybediyor" bulgusu yanlış kolun eseri

Aynı v3 koşularından `out.jsonl` üzerinden completion (n=6, medyan [min,max]):

| hücre | quinn | `kp-sevk` (yanlış "sevk" etiketi) | `kp-iw2400` (= ürün default IW) |
|---|---|---|---|
| iid50 (%50 kayıp, RTT 400) | 0.995 [0.94, 1.00] | **0.500** [0.01, 1.00] | 0.990 [0.98, 1.00] |
| burst50 (%50 korelasyonlu) | 0.895 [0.57, 1.00] | **0.495** [0.01, 1.00] | **0.995** [0.99, 1.00] |

**Okuma:** ürünün bugün sevk ettiği ilk-uçuş penceresi (2400 B) kullanıldığında Kama, burst50'de Quinn'i
**yeniyor** (0.995 vs 0.895; ayrıca dağılım dar vs Quinn'in 0.57–1.00 yayılımı) ve iid50'de istatistiksel
olarak berabere (0.990 vs 0.995). "Kaybetme" tamamen `initial_window_bytes = 0` koşan yanlış-etiketli
koldan geliyor.

**Yine de bu hücreler GEÇERLİ TAM-MİMARİ KIYASI DEĞİLDİR** (B-1/B-5): `kp-iw2400` de FEC'siz
(`fec: None`), spekülatif kol kapalı, yapı-gate 500. Yani tam v1 profili Quinn'e karşı **hiç ölçülmedi**.
Bu hücrelerin hükmü: **INVALID COMPARISON (kol ≠ ürün profili)** — ama sinyal, tek bir ürün-default'unun
sonucu tersine çevirdiğini gösteriyor.
