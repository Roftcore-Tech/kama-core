# `vectors/` — test-vektör dosya formatı

> **Otorite:** bu dizindeki [`SCHEMA.json`](SCHEMA.json) makine-okunur sözleşmedir. Bu README onu
> açıklar; **çelişkide `SCHEMA.json` kazanır.** Vektörlerin *içeriği* ise `specs/` altındaki donmuş
> spec'lere aittir (ccr §30 / §30.7, fec-core, noise-wire) — bu dizin yalnız **taşıma biçimini**
> sabitler.

## 1. Neden var

Beklenen byte çıktılarını üç ayrı araç okuyacak:

| tüketici | nerede | rol |
|---|---|---|
| Python üretici | *(henüz yok)* | vektör dosyalarını **yazar** |
| Rust yükleyici | `crates/kama-vectors` | dosyaları okur, motoru ona karşı **koşturur** |
| CI doğrulayıcı | `scripts/vectors/validate.py` | dosyaların formata uyduğunu **kapıda** denetler |

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
  <suite>/...                     gerçek vektör dosyaları (henüz yok)

scripts/vectors/
  validate.py                     Python doğrulayıcı (stdlib-only, bağımlılıksız)
  spec_coverage.py                spec ↔ vektör kapsam ratchet'i
  spec-coverage.json              ratchet tabanı (araç durumu — vektör dosyası DEĞİL)
  selftest.sh                     iki kapının da kendi kapısı
  testdata/EXPECTATIONS.json      PARİTE TABLOSU — Python ve Rust tarafı aynı listeyi okur
  testdata/accept/                kabul edilmesi gereken fixture'lar
  testdata/reject/                reddedilmesi gereken fixture'lar
  testdata/canonical/             kanonik-biçim fixture'ı
  testdata/vacuum/                vakum kapısı fixture'ı (kasten .json içermez)

crates/kama-vectors/
  src/lib.rs                      Rust yükleyici + tek hex yardımcısı
  tests/parity.rs                 Python ↔ Rust parite testi
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
  "schema_version": 2,
  "suite": "ccr-compact-device-cert-v1",
  "spec_version": "ccr-format-spec@v1.0+2026-09-18b",
  "generator": "kama-vector-gen 0.1.0",
  "notes": { "placeholder-signature": "imza hesaplanmadı; kriptografik kanıt DEĞİL" },
  "case_count": 2,
  "cases": [
    {
      "id": "ccr-compact-device-cert-v1.encode.field-order",
      "kind": "encode",
      "flags": ["placeholder-signature"],
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
| `schema_version` | ✔ | tamsayı. **Yalnız `2` tanımlı** (eski `1` artık reddedilir). |
| `suite` | ✔ | küçük harf token `^[a-z0-9]+(-[a-z0-9]+)*$`. **Dosya başına TEK suite.** |
| `spec_version` | ✔ | vektörlerin üretildiği donmuş spec sürümü. Önerilen biçim `<spec-slug>@<versiyon>[+<yama-tarihi>]`. |
| `generator` | — | dosyayı üreten aracın kimliği/sürümü. `spec_version` "hangi spec" der, bu "hangi üretici" der — bir golden değiştiğinde **neyin** değiştirdiği ancak böyle izlenir. |
| `description` | — | serbest metin; **araç davranışını etkilemez**. |
| `notes` | — | flag sözlüğü: `{token: açıklama}`. Kullanılan her flag burada tanımlı, tanımlı her flag en az bir vakada kullanılmış olmalı. |
| `case_count` | ✔ | `cases` uzunluğu, **dosyanın içinde**. Kesilmiş dosyayı ve tüketici vakumunu yakalar. |
| `cases` | ✔ | en az 1 vaka. |

Anahtarların **bu sırası** aynı zamanda kanonik serileştirme sırasıdır (§6).

### Vaka

| alan | zorunlu | kural |
|---|---|---|
| `id` | ✔ | **repo genelinde tekil.** Önerilen biçim `<suite>.<yerel-ad>`. |
| `kind` | ✔ | aracın üzerinden dağıtım yaptığı anahtar (`encode`, `decode`, `verify`, …). Söz dağarcığı suite'e özel ve **açık**; biçim sabit (küçük harf token). |
| `input` **\|** `input_hex` | ✔ | **tam olarak biri.** `input` = yapısal girdi (alanlardan kodlama yönü), `input_hex` = ham wire byte'ları (çözümleme yönü). |
| `expected_hex` **\|** `expected` | ✔ | **tam olarak biri.** `expected_hex` = pozitif vaka, `expected` = negatif vaka. |
| `flags` | — | vakanın **neden** var olduğunu makine-okunur etiketler; her biri `notes`'ta açıklanır. `note` insan içindir, `flags` araç içindir. |
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
8. **`case_count` == `cases` uzunluğu.** Tüketici koştuğu vaka sayısını buna karşı doğrular.
9. **Her flag `notes`'ta tanımlı, her not en az bir vakada kullanılmış.** İlk yön yazım
   hatasını (`shrot-key`) yakalar, ikinci yön ölü kayıt birikmesini engeller.
10. **Dosyalar kanonik biçimde** (§6). **En çok 8 MiB** (SR-AUDIT-001: bounds-before-alloc).
11. **`input` içindeki her byte alanının anahtarı `_hex` ile biter** (ya da `hex`'tir). Değeri hex
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

## 6. İki araç, tek kural kümesi

### Python doğrulayıcı — `scripts/vectors/validate.py`

```bash
python3 scripts/vectors/validate.py vectors/
```

```bash
scripts/vectors/selftest.sh
```

Bağımlılık yok (Python 3.9+ stdlib). Çıkış kodu: `0` temiz · `1` doğrulama hatası · `2` kullanım/ortam hatası.

**Vakum kapısı.** `--min-files N` olmadan, yol yanlış yazılırsa ya da ağaç boşalırsa doğrulayıcı
0 dosya gezip **yeşil** döner. CI bu yüzden `--min-files 2` ile koşar — `ci.yml`'deki
`test result: ok. [1-9]` kapılarıyla aynı sınıf (BUL-02/BUL-03).

### Rust yükleyici — `crates/kama-vectors`

```rust
let files = kama_vectors::load_dir("vectors")?;   // ağaç; id tekilliği tüm küme boyunca
let file  = kama_vectors::load_file(path)?;
let bytes = kama_vectors::hex("deadbeef")?;       // TEK hex yardımcısı

file.require_kinds(&["encode", "decode"])?;       // TÜKETİCİ SÖZLEŞMESİ (§7)
assert_eq!(ran, file.case_count());               // vakum kapısı
```

Aynı kuralları zorlar ve **aynı `SV-` kodlarını** üretir (`VectorError::code`). Şemanın iki
"tam olarak biri" kuralı burada **tip sistemine** gömülüdür — yüklenmiş bir `Case` hem `input`
hem `input_hex` taşıyamaz, çünkü öyle bir değer inşa edilemez:

```rust
pub enum Input    { Structured(Map<String, Value>), Wire(Vec<u8>) }
pub enum Expected { Wire(Vec<u8>), Rejected { result: RejectResult, reason: String } }
pub enum RejectResult { Drop, Reject, Fail }
```

`kama_vectors::hex` repo genelinde **tek** hex uygulamasıdır; dört test dosyasındaki
(`kama-fec-codec/tests/golden.rs`, `kama-noise-wire/tests/{golden_wire,sprint5b_legacy,rfc8439_vectors}.rs`)
kopya `fn hex` gövdelerinin yerini aldı. `SCHEMA.json` sözleşmesini uygular: küçük harf, çift
uzunluk, `0x` yok, boşluk yok — **büyük harfi sessizce düzeltmez, hata verir.** (RFC-8439
vektörleri belgeden boşluklu kopyalandığı için o testte boşluk temizliği yerel kalır; çözüm yine
ortak fonksiyondur.)

### Şemanın kendisi de doğrulanır

Doğrulayıcı `SCHEMA.json`'a **güvenir**. Şemadaki bir anahtar sözcüğün *değeri* yanlış tipteyse
(`"maxLength": "64"`) doğrulama ortasında Python istisnasıyla düşerdi: operatör teşhis yerine
yığın izi görür ve hata **veri** hatası sanılırdı. `check_schema_shape()` artık **hiçbir dosyaya
bakılmadan önce** koşar; kuruluysa resmî meta-şema da ikinci merci olarak çalıştırılır.

İki yapısal değişmez burada zorlanır:

**`$id` ↔ `schema_version` bağlı.** `$id`, `/v<N>/SCHEMA.json` ile bitmeli ve `N` =
`schema_version`'ın `const`'u olmalı. `$id` bir şema kaynağının **kimliğidir**; iki farklı şema
aynı kimliği taşırsa v1'i önbelleğe almış bir tüketici v2 için onu servis eder. Bu kural yordamı
insan diline bırakmıyor: sürüm yükseltilip `$id` unutulamaz. (`1`→`2` geçişinde tam olarak bu
kaçırılmıştı; kuralı yazınca kendi hatamızı yakaladı.)

**Combinator tuzağı yasak.** `additionalProperties: false` olan bir düğümün `allOf`/`anyOf`/
`oneOf` kolları `properties` tanımlayamaz. Draft 2020-12'de `additionalProperties` kardeş
kolların tanıttığı alanları **görmez** — böyle bir kol eklendiği gün geçerli belgeler sessizce
reddedilir ve sebebi bariz olmaz. (Doğru cevap `unevaluatedProperties`; onu desteklemek
annotation-izleme semantiği ister, bu değerlendiricinin kapsamı değil — o yüzden tuzak
**kullanımda** yasaklandı. Ölçüldü: kol eklenince `'tier' was unexpected`,
`unevaluatedProperties` ile kabul.)

### Bağımsız merci (çapraz kontrol)

ADR-03 doktrini (primary + cross-check) şema tarafında da uygulanır: alt-küme motoru
**birincil**, `jsonschema` **bağımsız merci**.

```bash
python3 scripts/vectors/validate.py vectors/ --require-cross-check
```

`--cross-check` tek başına, kütüphane yoksa **sessizce atlar ve yeşil döner** — CI'da bu,
çapraz kontrolün hiç koşmaması demektir. `--require-cross-check` bunu hataya çevirir ve CI
o bayrakla koşar (job `jsonschema`'yı kendi kurar).

### CI anotasyonu

`GITHUB_ACTIONS=true` iken (ya da `--github` ile) her bulgu ayrıca iş-akışı komutu olarak
basılır, böylece PR diff'inde **ilgili dosyaya iliştirilir**:

```
::error file=vectors/examples/example-positive.json,title=SV-006::/cases/0/expected_hex: ...
```

Satır numarası yok: JSON işaretçisinden satır türetmek güvenilmez, o yüzden işaretçi mesajda
durur ve anotasyon dosya düzeyinde kalır.

### Kanonik serileştirme (üretici disiplini)

Vektör dosyalarını bir üretici yazacak. Aynı içeriği her koşuda farklı byte'larla yazarsa
git diff'i gürültüyle dolar ve golden incelemesi okunmaz olur — "bu byte kasıtlı mı değişti"
sorusu cevaplanamaz. Kanonik biçim o soruyu geri getirir: diff'te görünen her satır gerçek
bir içerik değişikliğidir.

```
UTF-8, BOM yok · satır sonu LF · dosya sonunda tek satırsonu
girinti 2 boşluk · ensure_ascii (ASCII olmayan her karakter \uXXXX)
üst düzey + vaka anahtar sırası = SCHEMA.json'daki bildirim sırası
`input` içindeki serbest haritalar: özyinelemeli leksikografik anahtar sırası
```

```bash
python3 scripts/vectors/validate.py vectors/ --canonical         # denetle (SV-011)
python3 scripts/vectors/validate.py vectors/ --write-canonical   # düzelt
```

Kapsam `vectors/` ağacıdır. `scripts/vectors/testdata/` fixture'ları **kapsam dışıdır**:
bazıları kasten bozuk JSON, bazıları kasten yinelenen anahtar taşır — kanonikleştirilemezler.
`--write-canonical` doğrulamadan geçmeyen dosyaya **dokunmaz**; kanonikleştirme bir düzeltme
aracı değildir, bozuk dosyayı "düzeltip" ihlali gizlemesi istenmez.

### Spec ↔ vektör kapsamı (ratchet)

Şema kapısı dosyaların **biçimini** denetler, **kapsamını** değil. `ccr-format-spec.md` §30.7
adı geçen 24 negatif vaka tanımlıyor; hiçbiri yazılmasa bile şema kapısı yemyeşil kalırdı.

```bash
python3 scripts/vectors/spec_coverage.py            # ratchet kapısı (CI)
python3 scripts/vectors/spec_coverage.py --update   # kapsam arttıysa tabanı yükselt
```

Kapsam `scripts/vectors/spec-coverage.json` tabanına karşı kıyaslanır ve **düşerse kapı
kırmızı** olur. **Bugün 4/24.** Borç görünür ve yalnız azalabilir; "sonra yazarız" sessizce
kalıcı olamaz. Öz-test ratchet'in kendisinin de vakuma düşmediğini kanıtlar (sahte bir
tabanla kapının gerçekten kırmızı olduğu sınanır).

### Parite nasıl korunuyor

İki aracın vaka listesi `scripts/vectors/testdata/EXPECTATIONS.json`'da, **tek yerde** durur:

```json
{ "accept": [ { "label": "...", "paths": ["..."] } ],
  "reject": [ { "label": "...", "code": "SV-004", "paths": ["..."] } ] }
```

`selftest.sh` (bash+python) ve `crates/kama-vectors/tests/parity.rs` (Rust) aynı dosyadan
sürülür. Ret vakalarında yalnız "kırmızı mı" değil, **beklenen `SV-` koduyla mı kırmızı**
denetlenir. `parity.rs` ayrıca `validate.py`'yi alt süreç olarak çağırıp iki aracın **hükmünü**
karşılaştırır (`python3` yoksa o katman atlanır). Bir taraf diğerinden ayrıldığında ilgili satır
kırmızı olur; tablo iki yerde ayrı bakım istemez.

Parite tanımı: **aynı hüküm** (kabul/ret) **+ beklenen `SV-` kodu iki tarafta da var.** Bulgu
kümelerinin birebir eşitliği aranmaz — bir taraf aynı ihlal için fazladan bulgu üretebilir
(örn. büyük harf hex Python'da hem şema deseninden `SV-005` hem derin taramadan `SV-006` verir,
Rust'ta yalnız `SV-006`). Hüküm ve beklenen kod aynıdır.

### Negatif test korpusu

`scripts/vectors/testdata/reject/` — her biri **tek bir kuralı** ihlal eden dosyalar. Dokuz
doğrulayıcı hata kodunun tamamı koşuyor:

| kod | fixture | ne pinliyor |
|---|---|---|
| `SV-001` | 1 | bozuk JSON |
| `SV-002` | 2 | yinelenen anahtar — vaka düzeyi **ve** `input` içinde iç içe |
| `SV-003` | 4 | `schema_version` yok / dizge / ondalıklı / boolean |
| `SV-004` | 3 | bilinmeyen sürüm: `2`, `999`, negatif |
| `SV-005` | 24 | şema ihlalleri: oneOf ikilileri, bilinmeyen alan, `result` üçlüsü, **sekiz `maxLength` sınırı** |
| `SV-006` | 4 | hex: büyük harf, `0x`, tek uzunluk, dizi elemanı |
| `SV-007` | 1 | `id` tekilliği (iki dosya tek tek geçer, birlikte geçmez) |
| `SV-008` | 1 | dizgede kontrol karakteri |
| `SV-010` | — | vakum kapısı; `--min-files` bayrağına bağlı, dosya fixture'ı değil |
| `SV-009` | — | şema alt-kümesinde olmayan anahtar sözcük — **yalnız Python**, çünkü Rust yükleyici `SCHEMA.json`'u okumaz (kurallar koda gömülü). `selftest.sh` şemaya anahtar sözcük enjekte edip fail-closed davranışını ayrıca kanıtlar. |

**Dizin gezintisi.** `load_dir` sembolik dizin bağlarına **inmez** ve 32 seviye derinlik
sınırı uygular. `is_dir()` bağı izlediği için kendine bağlı bir dizin gezintiyi döndürüp aynı
dosyayı defalarca yükler ve sahte `SV-007` üretir (ölçüldü: python 1 dosya, rust 40 sahte
bulgu). Python'daki `Path.rglob` de bağlı dizinlere inmez; bu kural iki aracın **aynı dosya
kümesini** görmesini sağlar. Regresyon testi: `directory_symlink_loops_do_not_multiply_files`.

> **`maxLength` satırları neden ayrı önemli:** Python bu sınırları `SCHEMA.json`'dan **okur**,
> Rust'ta `const MAX_*` olarak **elle taşınırlar**. T-00.2'de bu fark ölçüldü: fixture yokken iki
> araç **sekiz sınırda sessizce ayrışıyordu** (aşırı uzun `suite`/`id`/`kind`/`note`/`spec_ref`/
> `description`/`hex` ve negatif `schema_version`). Şimdi her biri pinli.

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
| `SV-009` | **şema öz-denetimi**: okunamadı · tanınmayan anahtar sözcük · anahtar sözcüğün değeri yanlış tipte · derlenemeyen `pattern` · çözülemeyen `$ref` · `$id`↔sürüm ayrışması · combinator tuzağı · `--require-cross-check` ile `jsonschema` yok. Hepsi **rc=2** (araç hatası), rc=1 (veri hatası) değil |
| `SV-010` | `--min-files N` verildi ve daha az dosya doğrulandı — **vakum kapısı** |
| `SV-011` | kanonik serileştirme ihlali (`--canonical`) |
| `SV-012` | dosya boyutu sınırı aşıldı (8 MiB; SR-AUDIT-001 bounds-before-alloc) |
| `SV-013` | `case_count` ile `cases` uzunluğu uyuşmuyor |
| `SV-014` | flag/`notes` tutarsızlığı (tanımsız flag ya da ölü not) |
| `SV-015` | **tüketici** tanımadığı bir `kind` gördü — yalnız Rust (`require_kinds`) |

### İki tasarım kararı

**Fail-closed değerlendirici.** `validate.py` içindeki JSON Schema motoru tam bir
Draft 2020-12 uygulaması değil, `SCHEMA.json`'un kullandığı alt kümedir. Tanımadığı bir anahtar
sözcükle karşılaşırsa **onu atlamaz, `SV-009` ile durur**. Yani şemaya yeni bir kural eklenip
doğrulayıcının onu sessizce zorlamadığı bir ara durum oluşamaz — kapı kırmızı olur, geliştirici
motoru genişletir. Üçüncü parti bir uygulamayla çapraz kontrol:

```bash
python3 scripts/vectors/validate.py vectors/ --cross-check   # `jsonschema` kuruluysa
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
5. **Tanıdığın `kind` kümesini beyan et** (`require_kinds`). `match` içindeki bir `_ => {}`
   kolu, `kind`'daki bir yazım hatasını ya da dosyaya sonradan eklenen yeni bir `kind`'ı
   **sessizce hiç koşmamaya** çevirir: test yeşil, kapsam sıfır.
6. **Koştuğun vaka sayısını `case_count`'a karşı doğrula.** Filtreleyip 0 vaka koşmak ile
   dosyanın boş olması ayırt edilemez; ikisi de yeşil görünür.
7. Bir `flag`'e dayanarak vaka atlıyorsan bunu **rapor et** — sessiz atlama yine vakumdur.

## 8. `schema_version` nasıl değişir

> **Bu yordam bir kez uygulandı: `1` → `2`** (T-00.2). Eklenenler: `generator`, `notes`,
> `case_count`, `cases[].flags`. `1` artık **desteklenmiyor** — `SV-004` ile reddedilir
> (`reject/schema-version-1.json` bunu pinliyor). İki sürümü birden desteklemek gerekmedi:
> repo dışında üretici yoktu, hepsi tek commit'te taşındı.

`schema_version: 1` **donmuştur.** Alan eklemek, silmek, bir alanı zorunlu yapmak ya da bir kuralı
gevşetmek → `2`. Aynı PR'da:

1. `SCHEMA.json` + bu README güncellenir — **`$id`'nin `/v<N>/` parçası da** (doğrulayıcı
   `$id` ile `const`'un ayrışmasına izin vermez, bu adım unutulamaz),
2. `validate.py` ve `crates/kama-vectors/src/lib.rs` içindeki `SUPPORTED_SCHEMA_VERSION`
   güncellenir (gerekirse iki sürüm birden desteklenir),
3. `scripts/vectors/testdata/reject/schema-version-2.json` **bir üst sürüme** taşınır — yoksa
   "bilinmeyen sürüm reddedilir" kapısı vakumda kalır,
4. Python üretici (yazıldığında) aynı PR'da taşınır.

Serbest metin alanı (`description`, `note`, `spec_ref`) eklemek de sürüm artırır: `additionalProperties: false`
eski araçta yeni dosyayı kırar, sessiz geçiş olmaz — istenen davranış budur.

## 9. Bilinçli olarak YAPILMAYANLAR

Dış pratikte (Wycheproof) olup burada **kasten** almadığımız iki şey:

**`result: "acceptable"`** — Wycheproof'ta "geçerli ama uygulama makul olarak reddedebilir"
demek. Bizde `result` üçlüsü **kilitli** (`drop`/`reject`/`fail`, T-00.1) ve üçü de rettir.
Dördüncü bir değer eklemek yerine aynı anlam **pozitif vaka + flag** ile ifade edilir:
`expected_hex` + `flags: ["may-reject"]` ve `notes`'ta gerekçesi. Kilidi tek taraflı kırmaya
gerek yok, mekanizma zaten var.

**`testGroups`** — ortak parametreleri (aynı anahtar, aynı eğri) paylaşan vakaları gruplar.
Şemanın kilitli şekli düz bir `cases[]`; gruplama birleştirme (merge) semantiği getirir ve
bugün bunu gerektiren **tek bir gerçek vektör dosyası yok**. Spekülatif makine yazmak yerine
ertelendi: ccr §35 oturum vektörleri yazıldığında tekrar bakılır (`schema_version: 3` adayı).

## 10. Kilitli metnin ötesine geçen kararlar

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

## 11. Kapsam dışı

- **Gerçek vektör içeriği.** `vectors/` altında suite dizini yok; `examples/` dışında dosya yok.
- **Python üretici.** Ayrı kalem.
- **`reason` sözlüğü.** Hangi ret gerekçesinin hangi token'a karşılık geldiği suite bazında, vektör
  üretildiğinde sabitlenir.
- **Kapsam borcu: ccr §30.7'nin 20 vakası** (bugün 4/24). Ratchet borcu görünür tutuyor ve
  büyümesini engelliyor; kapatmak vektör yazma işidir.
- **ADR-12 `testvectors/` → `vectors/` satır düzeltmesi.** Spec değişikliği; talimatsız yapılmaz.
