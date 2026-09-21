# `vectors/` — test-vektör dosya formatı

> **Otorite:** bu dizindeki [`SCHEMA.json`](SCHEMA.json) makine-okunur sözleşmedir. Bu README onu
> açıklar; **çelişkide `SCHEMA.json` kazanır.** Vektörlerin *içeriği* ise `specs/` altındaki donmuş
> spec'lere aittir (ccr §30 / §30.7, fec-core, noise-wire) — bu dizin yalnız **taşıma biçimini**
> sabitler.

## 1. Neden var

Beklenen byte çıktılarını üç ayrı araç okuyacak:

| tüketici | rol |
|---|---|
| Python üretici | vektör dosyalarını **yazar** |
| Rust testleri | dosyaları okur, motoru ona karşı **koşturur** |
| CI doğrulayıcı | dosyaların formata uyduğunu **kapıda** denetler |

Aynı formatı okumazlarsa her biri kendi yorumunu yapar ve testler birbirini doğrulayamaz — üretici
`"DEADBEEF"` yazar, Rust `0xdeadbeef` bekler, CI hiçbirini görmez. `SCHEMA.json` bu üç aracın tek
buluşma noktasıdır.

## 2. Yerleşim

```
vectors/
  SCHEMA.json                     JSON Schema (Draft 2020-12) — format otoritesi
  README.md                       bu dosya
  examples/
    example-positive.json         pozitif vaka örneği (encode + decode)
    example-negative.json         negatif vaka örneği (reject + fail)
  <suite>/...                     gerçek vektör dosyaları (T-00.1 kapsamında henüz yok)

scripts/vectors/
  validate_vectors.py             doğrulayıcı (stdlib-only, bağımlılıksız)
  selftest.sh                     doğrulayıcının kendi kapısı
  testdata/accept/                kabul edilmesi gereken fixture'lar
  testdata/reject/                reddedilmesi gereken fixture'lar
```

`examples/` altındaki iki dosya **canlı**dır: CI `vectors/` ağacını süpürürken onları da doğrular,
yani örnekler bozulursa kapı kırmızı olur.

> `examples/` dosyaları **kriptografik golden vector DEĞİLDİR.** Alan değerleri desenli dolgudur,
> `issuer_signature` hesaplanmış bir imza değildir (ccr §30 `TEST_VECTOR_PLACEHOLDER` disiplini).
> Format örneğidirler; doğruluk kanıtı olarak kullanılamazlar.

> **Adlandırma notu:** `tech-stack-adr.md` ADR-12 repo ağacında bu dizine `testvectors/` diyor.
> T-00.1 görev metni `vectors/` dedi ve burası öyle açıldı. İkisi aynı şeydir; ADR satırını
> güncellemek ayrı bir kalemdir (bkz. §9).

## 3. Dosya şekli

```json
{
  "schema_version": 1,
  "suite": "ccr-compact-device-cert-v1",
  "spec_version": "ccr-format-spec@v1.0+2026-09-18b",
  "cases": [
    {
      "id": "ccr-compact-device-cert-v1.encode.field-order",
      "kind": "encode",
      "input": { "not_after_hex": "000000006a1b2c00", "...": "..." },
      "expected_hex": "1111...4444"
    },
    {
      "id": "ccr-compact-device-cert-v1.invalid-8.device-x25519-all-zero",
      "kind": "decode",
      "input_hex": "1111...4444",
      "expected": { "result": "reject", "reason": "invalid-8-device-x25519-all-zero" }
    }
  ]
}
```

### Üst düzey

| alan | zorunlu | kural |
|---|---|---|
| `schema_version` | ✔ | tamsayı. **Yalnız `1` tanımlı.** |
| `suite` | ✔ | küçük harf token `^[a-z0-9]+(-[a-z0-9]+)*$`. **Dosya başına TEK suite.** |
| `spec_version` | ✔ | vektörlerin üretildiği donmuş spec sürümü. Önerilen biçim `<spec-slug>@<versiyon>[+<yama-tarihi>]`. |
| `description` | — | serbest metin; **araç davranışını etkilemez**. |
| `cases` | ✔ | en az 1 vaka. |

### Vaka

| alan | zorunlu | kural |
|---|---|---|
| `id` | ✔ | **repo genelinde tekil.** Önerilen biçim `<suite>.<yerel-ad>`. |
| `kind` | ✔ | aracın üzerinden dağıtım yaptığı anahtar (`encode`, `decode`, `verify`, …). Söz dağarcığı suite'e özel ve **açık**; biçim sabit (küçük harf token). |
| `input` **\|** `input_hex` | ✔ | **tam olarak biri.** `input` = yapısal girdi (alanlardan kodlama yönü), `input_hex` = ham wire byte'ları (çözümleme yönü). |
| `expected_hex` **\|** `expected` | ✔ | **tam olarak biri.** `expected_hex` = pozitif vaka, `expected` = negatif vaka. |
| `note`, `spec_ref` | — | serbest metin; **araç davranışını etkilemez**. |

Başka hiçbir alana izin yok: bilinmeyen alan **reddedilir** (fail-closed). Yeni alan eklemek
`schema_version` artırmayı gerektirir (§8).

## 4. Kurallar (NORMATİF)

1. **Hex küçük harf, `0x` YOK.** `^([0-9a-f]{2})*$`. `DEADBEEF` ✗ · `0xdead` ✗ · `abc` (tek uzunluk) ✗ · `deadbeef` ✓
2. **Byte dizileri hex dizgedir.** Tamsayı dizisi (`[222, 173]`) değil.
3. **Beklenen wire her zaman hex.** Pozitif vakada beklenen çıktı `expected_hex`'tir, başka bir gösterim yoktur.
4. **Boş hex geçerlidir.** `""` = sıfır byte (örn. ccr §35 `early_data` N=0), yokluk DEĞİLDİR.
5. **Negatif vakada `result` tam olarak şu üç değerden biri:** `drop` · `reject` · `fail`. Dördüncüsü yok.
6. **`id` repo genelinde tekildir** — dosya içinde değil, `vectors/` ağacının tamamında.
7. **Bilinmeyen `schema_version` gören araç HATA verir, sessizce geçmez.**
8. **`input` içindeki her byte alanının anahtarı `_hex` ile biter** (ya da `hex`'tir). Değeri hex
   dizge veya hex dizgelerden oluşan dizi olabilir; doğrulayıcı her derinlikte denetler. Bu kural
   JSON Schema ile ifade edilemez — doğrulayıcı zorlar (`SV-006`).

## 5. `result` üçlüsü

Üçü farklı **gözlemlenebilir** davranıştır; eş anlamlı değildirler ve test bunları ayırt eder.

| değer | anlamı | çağıran ne görür |
|---|---|---|
| `drop` | sessiz atım | hiçbir şey; yalnız bir sayaç artar. Örnek: PARAM_SET onaylanmadan gelen FEC shard, tampon cap'te → `fec_pre_param_dropped`. |
| `reject` | tanımlı ret | tipli bir hata. Girdi **gerçekten geçersizdir**. Örnek: ccr §30.7 INVALID-4 (CompactDeviceCertV1 uzunluk ≠ 152B). |
| `fail` | fail-closed / defer | işlem **tamamlanamaz**; girdi mutlaka geçersiz DEĞİLDİR. Örnek: ccr §30.7 INVALID-13 (cert-ref cache-miss + inline cert yok) — cert sonradan gelirse aynı girdi geçerli olabilir. |

`reason` kararlı, **tam-dizge karşılaştırılan** bir gerekçedir. Önerilen biçim küçük harf token,
kaynağını taşısın: `invalid-4-compact-device-cert-length`. Prose açıklama `note`'a yazılır.

## 6. Doğrulayıcı

```bash
python3 scripts/vectors/validate_vectors.py vectors/
```

```bash
scripts/vectors/selftest.sh
```

Bağımlılık yok (Python 3.9+ stdlib). Çıkış kodu: `0` temiz · `1` doğrulama hatası · `2` kullanım/ortam hatası.

### Hata kodları

| kod | ne |
|---|---|
| `SV-001` | JSON olarak ayrıştırılamadı / dosya yok |
| `SV-002` | aynı nesnede yinelenen JSON anahtarı |
| `SV-003` | `schema_version` yok ya da tamsayı değil |
| `SV-004` | **bilinmeyen `schema_version`** — sessiz geçilmez |
| `SV-005` | `SCHEMA.json` ihlali |
| `SV-006` | hex alanı kuralı (küçük harf, çift uzunluk, `0x` yok) |
| `SV-007` | `id` repo genelinde tekil değil |
| `SV-008` | dizgede kontrol karakteri |
| `SV-009` | `SCHEMA.json` okunamadı / **doğrulayıcının tanımadığı şema anahtar sözcüğü** |

### İki tasarım kararı

**Fail-closed değerlendirici.** `validate_vectors.py` içindeki JSON Schema motoru tam bir
Draft 2020-12 uygulaması değil, `SCHEMA.json`'un kullandığı alt kümedir. Tanımadığı bir anahtar
sözcükle karşılaşırsa **onu atlamaz, `SV-009` ile durur**. Yani şemaya yeni bir kural eklenip
doğrulayıcının onu sessizce zorlamadığı bir ara durum oluşamaz — kapı kırmızı olur, geliştirici
motoru genişletir. Üçüncü parti bir uygulamayla çapraz kontrol:

```bash
python3 scripts/vectors/validate_vectors.py vectors/ --cross-check   # `jsonschema` kuruluysa
```

(`jsonschema` **bağımlılık değildir**; kurulu değilse adım atlanır. `SCHEMA.json` standart
Draft 2020-12'dir, herhangi bir uygulama okuyabilir.)

**Yinelenen JSON anahtarı hatadır.** `json.load` yinelenen anahtarda sessizce sonuncuyu tutar;
iki araç aynı dosyadan farklı byte okuyabilir. Doğrulayıcı bunu `SV-002` ile reddeder.

## 7. Tüketici sözleşmesi

Üç aracın da uyması gereken asgari davranış:

1. `schema_version`'ı **her şeyden önce** oku. `1` değilse **dur ve hata ver** — dosyayı atlamak,
   uyarı basıp devam etmek ya da "elimden geleni okurum" davranışı YASAK.
2. `expected_hex` / `expected` ayrımını vakanın **beklenen sonucu** olarak ele al; `expected` varsa
   test başarısı = motorun **tam olarak o `result`+`reason` ile** durmasıdır.
3. Hex'i çözerken küçük-harf varsay; büyük harf gördüğünde **düzeltme, hata ver** (dosya zaten
   CI'dan geçmiş olmalıydı; büyük harf gören araç bozuk bir üreticiye bakıyordur).
4. Vakaları `id` ile eşle, sıra ile değil.

## 8. `schema_version` nasıl değişir

`schema_version: 1` **donmuştur.** Alan eklemek, silmek, bir alanı zorunlu yapmak ya da bir kuralı
gevşetmek → `2`. Aynı PR'da:

1. `SCHEMA.json` + bu README güncellenir,
2. `validate_vectors.py` içindeki `SUPPORTED_SCHEMA_VERSION` güncellenir (gerekirse iki sürüm birden
   desteklenir),
3. `scripts/vectors/testdata/reject/schema-version-2.json` **bir üst sürüme** taşınır — yoksa
   "bilinmeyen sürüm reddedilir" kapısı vakumda kalır,
4. Python üretici ve Rust okuyucu aynı PR'da taşınır.

Serbest metin alanı (`description`, `note`, `spec_ref`) eklemek de sürüm artırır: `additionalProperties: false`
eski araçta yeni dosyayı kırar, sessiz geçiş olmaz — istenen davranış budur.

## 9. T-00.1'de kilitli metnin ötesine geçen kararlar

Görev metni şemayı alan alan kilitledi. Aşağıdakiler o metinde **yoktu**, uygulanabilir olsun diye
eklendi; her biri tek satırlık bir düzeltmeyle geri alınabilir:

| karar | gerekçe | geri alma |
|---|---|---|
| `suite` / `kind` / `id` küçük harf token deseni | makine dağıtım anahtarları; hex küçük-harf disipliniyle aynı hat | `SCHEMA.json`'daki `pattern` |
| `note`, `spec_ref`, `description` opsiyonel alanları | vakanın spec atfı olmadan corpus denetlenemiyor | `properties`'ten çıkar |
| `input` tip olarak `object` | derin `_hex` kuralının denetlenebilmesi için | `"type": "object"` satırı |
| `reason` biçimi **kısıtlanmadı** (yalnız boş değil, kırpılmış, ≤200) | kilitli metin biçim dayatmadı; üreticiyi kırmamak için serbest bırakıldı | — |
| `_hex` anahtar soneki zorunluluğu (kural 4.8) | "byte dizileri hex string" kuralı ancak böyle makine-denetlenebilir | `_deep_hex_findings` |
| yinelenen JSON anahtarı = hata | iki aracın aynı dosyadan farklı byte okumasını engeller | `_no_duplicates` |

## 10. Kapsam dışı (T-00.1'de YOK)

- **Gerçek vektör içeriği.** `vectors/` altında suite dizini yok; `examples/` dışında dosya yok.
- **Python üretici.** Ayrı kalem.
- **Rust okuyucu.** `ccr-codec` bugün API-first iskelet; vektör okuyan test yardımcısı ayrı kalem.
- **`reason` sözlüğü.** Hangi ret gerekçesinin hangi token'a karşılık geldiği suite bazında, vektör
  üretildiğinde sabitlenir.
- **ADR-12 `testvectors/` → `vectors/` satır düzeltmesi.** Spec değişikliği; talimatsız yapılmaz.
