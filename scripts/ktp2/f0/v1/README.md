# Senaryo v1 — netem ölçüm düzeneği (kovalı hat) ve birleşik kabul koşusu

Ürün kodu değildir; yalnız ölçüm betikleri. Senaryo: #175 yorum 7096381261. Kabul hesabı ve satır adları (1, 2, 3a–3c, 4a–4c, 5): #175 yorum 7171842711 ve ekleri. Hüküm bu betiklerle YAZILMAZ; hükmü değerlendirici (#252 `scripts/ktp2/f0/kabul/degerlendir.py`) verir. Buradaki `ozet.py` ve `ack_tablo.py` döküm üretir.

Bu belgedeki sayıların hiçbiri bu dalda koşulmuş bir hat ölçümünden gelmez. Dayanak kayıtlar: `yerel/ham-izler-netem-v1` @ `f955a7a0`, `netem-v1/` (PTT'siz senaryo v1 koşuları).

## 1. Dosyalar

| Yol | İçerik |
|---|---|
| `scripts/netem/kapi3_harness.sh` `[DÜZENEK v1]` | İki aşamalı ileri hat: `ma0` girişi → `ifb0` darboğaz → `mb0` netem (gecikme + kayıp; kayıp darboğazdan SONRA) |
| `scripts/netem/kapi3_ornekleyici.py`, `kapi3_basamak.py`, `kapi3_kesinti.py`, `kapi3_kopyaci.sh` | Yan süreçler: kuyruk örnekleyici, t0'a bağlı hız basamağı, zamanlı kesinti, düğüm kaydı kopyası |
| `scripts/netem/pkt_capture.py` | Yakalama; `pid` (UDP gövdesi özeti) ile düzlemler arası eşleme |
| `scripts/ktp2/f0/f0_cell.sh` | Hücre koşucusu; `FWD="v1 <hız>"` ise darboğaz bariyeri |
| `scripts/ktp2/f0/cells/v1/*.cell` | Hücreler (bölüm 2) |
| `scripts/ktp2/f0/v1/ack_tablo.py`, `ozet.py` | ACK başına tablo ve koşu özeti (bölüm 6) |

## 2. Hücreler (`scripts/ktp2/f0/cells/v1/`)

Ortak: ileri 100 kbit (12 500 B/s), 300 ms, %20 kayıp (netem `seed $V1_SEED`); dönüş 496 bit/s (62 B/s), 300 ms; sıra 64 paket; 49 metin, 2 s'de bir, 64 B; `KAMA_TRANSPORT_ENGINE=cc-port KAMA_ACK_FORMAT=bitmap`, `KAMA_LINK_RATES` aynalı; tohum ortamdan (`V1_SEED=1|2|3`); ikili `BIN_F0A_ADAY`.

| Hücre | Darboğaz | Hız planı | Dosya | PTT | Rol |
|---|---|---|---|---|---|
| `v1_birlesik.cell` | kovalı | 20 → 50, 60 → 100, 85,0 → 50, 85,5 → 100 kbit | 1 200 000 B | var (sabit çizelge) | **Birleşik kabul koşusu** |
| `v1_birlesik_dipsiz.cell` | kovalı | 20 → 50, 60 → 100 kbit (85,0–85,5 s dibi YOK) | 1 200 000 B | var (aynı çizelge) | 3c satırının karşılaştırma kolu |
| `v1_butunluk.cell` | kovalı | birleşikle aynı | 165 000 B | var (aynı çizelge) | Dosya bütünlüğü — AYRI kayıt (bölüm 5) |
| `v1_taban.cell` | kovalı | birleşikle aynı | 1 200 000 B | yok | PTT'siz taban |
| `v1_kesmesiz.cell` | kovalı | dipsiz | 1 200 000 B | yok | PTT'siz tabanın dipsiz kolu; birleşik koşunun kolu DEĞİL |
| `v1_taban_teshis_seri.cell` | tam serileştiren | birleşikle aynı | 1 200 000 B | yok | Teşhis; kabul düzeneği değil |
| `v1_birlesik_teshis_seri.cell` | tam serileştiren | birleşikle aynı | 1 200 000 B | var (çizelge ortamdan) | Teşhis; kabul düzeneği değil |

`v1_birlesik_dipsiz.cell` ve `v1_butunluk.cell`, `v1_birlesik.cell`'i `source` eder; yük, basış çizelgesi, tohum düzeni, hat ve motor ayarları oradan gelir. Her birinin değiştirdiği şey hücre başlığında yazılıdır (dipsiz: `V1_PLAN` ve `CELL`; bütünlük: `--file-bytes` ve `CELL`). `cell.env`'de `spec_sha256` türetilmiş dosyanın, `spec_taban_sha256` `v1_birlesik.cell`'in özetidir.

Kullanım:

```
V1_SEED=1 BIN_F0A_ADAY=<f0_flows> bash scripts/ktp2/f0/f0_cell.sh scripts/ktp2/f0/cells/v1/v1_birlesik.cell <çıktı kökü>
```

İkili önkoşulu (PR #260 Bulut-C incelemesi): `ack`, `ack_rx`, `lg`, `rcpt_tx`, `rcpt_wire` satırları yalnız ölçüm kancalarını taşıyan SHA'dan (`cd15a3ae` ve üstüne kurulanlar) derlenen `f0_flows`'ta vardır. Hücre ikilinin SHA'sını sabitlemez; SHA ve sha256 koşu kaydına ayrıca yazılır.

## 3. PTT basış çizelgesi (birleşik, dipsiz, bütünlük)

Koordinatör kararı, #175 yorum 7172332337. `v1_birlesik.cell` içinde sabit:

```
V1_PTT_AT_MS=5000,16000,40000,56000,72000,82000
V1_PTT_BUNDLES=28
```

- Altı basış × 8,4 s (28 demet × 300 ms). Açılıştan kapanışa 8,7 s; son basış 90,7 s'de biter.
- Ortamda bu adlarla başka bir değer varsa hücre koşmaz (çıkış 2). Çizelge `cmd.txt`'ye `--ptt-at-ms` / `--ptt-bundles` olarak da yazılır.
- `v1_birlesik_teshis_seri.cell` çizelgeyi hâlâ ortamdan alır (varsayılanı yok); birleşikle aynı yük ancak aynı iki değer verilirse kurulur.

## 4. Yineleme kuralı (koşudan ÖNCE yazıldı)

Dayanak: aynı ikili + hücre + tohum aynı koşuyu vermez; iki koşunun ACK 16'da ayrıştığı ölçüldü (PR #260 Web incelemesi madde 4; `netem-v1/k258-1` `1` ↔ `1-yineleme`).

- Kabul hükmü üç tohumun üçünde de geçmeyi ister.
- Yineleme HÜKMÜ DEĞİŞTİRMEZ.
- Bir tohum 4a, 3a ya da 3c satırında kalırsa o tohum aynı SHA ile iki kez daha koşulur. Amaç yalnız nedeni etiketlemektir:
  - üç koşunun üçünde de kaldıysa (3/3): **yinelenebilir**;
  - değilse: **koşu değişkenliği**.
- Her iki durumda satır **KALDI** yazılır.

Bu kuralın söylemedikleri (karar yok; bölüm 7): 3c yinelemesinde dipsiz kolun da yeniden koşulup koşulmayacağı; öbür satırlarda (1, 2, 3b, 4b, 4c, 5) kalan tohum için yineleme.

## 5. Dosya bütünlüğü — ayrı kayıt, birleşik kabulün kendisi değil

- `v1_birlesik.cell`'in 1 200 000 B'lık dosyası 100 s'de bitmez: sekiz yayımlanmış kaydın hiçbirinde Bob'da `rx c:"F"` yok, `bob.json` `file.status` = `none`. Birleşik koşuda "dosya bütünlüğü" satırı **SINANMADI** kalır.
- `v1_butunluk.cell` aynı hat, aynı hız planı, aynı metin + PTT yükü ve aynı motor ayarlarıyla 165 000 B'lık dosya gönderir. Bütünlük bu kayıttan okunur: `bob.json` `file.status`, `file.size`, `file.match`; `bob.jsonl` `rx c:"F"` (`match` = alınan dosyanın gönderilen desenle karşılaştırması).
- Bu kayıt **birleşik kabul koşusu değildir** ve onun yerine geçmez: dosya koşu ortasında biter, sonrasında hat dosya yüküyle dolu değildir. Bu kayıttan hız sınırlama, hızlanma ya da kısa dip satırı okunmaz. Birleşik koşunun kendi "dosya bütünlüğü" satırı SINANMADI olarak kalır; bütünlük hükmü hangi kayıttan alınırsa o kayıt adıyla yazılır.

### Dosya boyunun türetilmesi

Kaynak: `netem-v1/` altındaki sekiz PTT'siz kayıt (`bob.jsonl` `tr` satırları, `D` = alıcıda birimin ilk teslimi). Dosya parçası birimi 1 013 B; içindeki dosya yükü 990 B (1 013 − 5 B kayıt başlığı − 18 B parça öneki; `crates/kama-app/src/codec/limits.rs`. Aynı kayıtlarda metin birimi 75 B = 5 + 6 + 64 ile tutarlı).

| Kayıt | İkili | 100 s'de Bob'a teslim edilen 1 013 B birim | Dosya yükü (× 990 B) | İlk 167 parçanın tamamı Bob'da (τ) |
|---|---|---|---|---|
| `1` (tohum 1) | `73715edb` | 459 | 454 410 B | 37,7 s |
| `2` (tohum 2) | `73715edb` | 339 | 335 610 B | 41,6 s |
| `3` (tohum 3) | `73715edb` | 471 | 466 290 B | 33,1 s |
| `kesmesiz-1` | `73715edb` | 360 | 356 400 B | 37,3 s |
| `kancali-1/1` | `e1012cf2` | 351 | 347 490 B | 37,9 s |
| `k258-1/1` | `911f2c6d` | 463 | 458 370 B | 37,7 s |
| `k258-1/1-yineleme` | `911f2c6d` | 452 | 447 480 B | 37,7 s |
| `k258-2/1` | `b0659064` | 368 | 364 320 B | 37,7 s |

- En küçük ilerleme 335 610 B; yarısı 167 805 B; aşağı yuvarlanmış **165 000 B** (167 parça: 166 × 990 + 660).
- Sınır: kayıtların hepsi PTT'siz. Birleşik yükte (altı basış) dosya ilerlemesi ÖLÇÜLMEDİ; payın yeterli olduğu varsayımdır. Kabul SHA'sı bu sekiz kaydın ikililerinden farklıdır.
- Dosya koşu içinde bitmezse (`file.status` = `none`) o kayıt bütünlüğü sınamamıştır.

## 6. `ozet.py` — darboğaz beklemesinin tanımı

Sınır `wait_bound = 2 × (MTU + 28) ÷ 6 250` = 393 ms, paketin ÖNÜNDEKİ bekleme içindir (kendi serileşmesi hariç). `ozet.py` değerlendiricinin `packet_waits` tanımını kullanır:

- giriş = `cap/MID_ma0` `in`, çıkış = `cap/MID_ifb0` `out`; eşleme `pid` ile (aynı kimlikten ilk giriş);
- bekleme = (çıkış − giriş) − IP boyu ÷ çıkış anındaki hat hızı; alt sınır 0;
- hat hızı uygulanan çizelgeden (`basamak.jsonl` `komut_once_unix_ns` + `hiz`; ilk hız `kuyruk.jsonl` ilk örneğindeki `ifb0` tbf hızı);
- pencereye paket darboğaza GİRİŞ anıyla girer.

393 ms kıyası yalnız bu tanımla yazılır. Özet ayrıca iki bilgi satırı verir, ikisi de 393 ms ile kıyaslanmaz: "giriş→çıkış, serileşme DAHİL" (aynı paketler) ve "serileşme DAHİL, pencere ÇIKIŞ anına göre" (betiğin önceki tanımı; eski `ozet.txt` dosyalarıyla karşılaştırma için).

Fark: değerlendirici çizelgeyi okuyamazsa senaryo v1 çizelgesine düşer; `ozet.py` o durumda durur.

### Araç doğrulaması (yayımlanmış üç kayıt, [35, 58) s)

| Kayıt | Önceki `ozet.py` (serileşme dahil, çıkış penceresi) | Serileşme dahil, giriş penceresi | Yeni tanım (serileşme hariç, giriş penceresi) | Değerlendirici (#252 @ `ccbf4295`) |
|---|---|---|---|---|
| `kancali-1/1` | 437,8 ms; 14/188 > 393 | 558,6 ms; 15/188 | 386,7 ms; 0/188 | 386,7 ms; 0/188 |
| `k258-1/1` | 535,5 ms; 29/188 | 535,5 ms; 32/189 | 430,7 ms; 1/189 | 430,7 ms; 1/189 |
| `k258-1/1-yineleme` | 527,3 ms; 28/188 | 527,3 ms; 31/189 | 451,9 ms; 1/189 | 451,9 ms; 1/189 |

- Yeni tanım ile değerlendiricinin `packet_waits` çıktısı paket düzeyinde karşılaştırıldı (946 / 1 152 / 1 130 eşleşen paket): en büyük fark 0,5 µs'den küçük (değerlendirici saniyeyi kayan noktayla, `ozet.py` nanosaniyeyi tam sayıyla çıkarır).
- İkinci ve üçüncü sütunun farkı yalnız penceredir: çıkış penceresi 35 s'den önce girip sonra çıkan paketleri alır, 58 s'den önce girip sonra çıkanları almaz.
- `ozet.py`'nin öbür satırları üç kayıtta değişmedi (kesme anı ve [20, 60) s satırlarına serileşme hariç değer eklendi; uygulanan çizelge satırı eklendi).

## 7. Kapsam, sınırlar, açık noktalar

- **Koşulmadı.** Bu dalda hat kurulmadı, hücre koşulmadı. Yapılan denetimler: `bash -n`, `python3 -m py_compile`, hücrelerin `source` edilip değişkenlerinin dökülmesi (koşusuz), `ozet.py`'nin yayımlanmış üç kayıtta çalıştırılması. `f0_cell.sh`'ın `spec_taban_sha256` satırı ve türetilmiş hücrelerin uçtan uca koşusu sınanmadı.
- **`pkt_capture.py` seçeneğe bağlı değildir.** `CAPTURE=1` ile koşan her hücrede (v1 dışındakiler dahil) yakalama satırına `ipid`, UDP satırına `pid` eklenir ve UDP paketi başına bir SHA-1 hesaplanır. Gösterilen: var olan alanların adı ve hesabı aynı (metin karşılaştırması). Gösterilmeyen: (a) eski okuyucuların yeni alanlarla çalıştığı — depoda bulunan okuyucular alanı adıyla seçiyor (`kapi3_selftest.sh` jq süzgeci, `ack_tablo.py`, `ozet.py`), depo dışındakiler okunmadı; (b) özet hesabının yakalama zaman damgalarına etkisi — ölçülmedi.
- **3c yinelemesi ve dipsiz kol.** Yineleme kuralı kalan tohumun birleşik koşusunu yineler; dipsiz kolun da yinelenip yinelenmeyeceği yazılı değil. Aynı tohum aynı koşuyu vermediği için dipsiz kolun tek koşusu da değişkendir.
- **Dipsiz kolda `ack_tablo.py`.** Betik 85,0 s basamağı olmayan planda τ ≥ 80 s ACK'i görünce durur; `ozet.py` onun çıktısını ister. 3c için değerlendiricinin dipsiz koldan okuduğu (`--baseline`): `cap/MID_ifb0.jsonl`, `alice.jsonl` `ts` satırları, `basamak.jsonl` ve `kuyruk.jsonl` (uygulanan çizelge).
- **Tam serileştiren kol hiç koşulmadı**; hız basamağı o kolda sıradaki paketlere uygulanmaz (hücre başlığı).
- **Zamanlı kesinti (`KES_PLAN`) iki aşamalı hatta koşulmadı.**
