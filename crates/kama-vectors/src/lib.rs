//! Kama test-vektör yükleyicisi — `vectors/SCHEMA.json`'un Rust tarafı (T-00.2).
//!
//! Üretici (Python) ve tüketici (Rust) aynı dosyayı **aynı kurallarla** okumalı. Python tarafı
//! `scripts/vectors/validate.py`; bu crate onun ikizidir: aynı kuralları zorlar ve **aynı `SV-`
//! hata kodlarını** üretir. İki aracın ayrıldığı yer `crates/kama-vectors/tests/parity.rs`
//! tarafından yakalanır — iki taraf da `scripts/vectors/testdata/EXPECTATIONS.json`'daki tek
//! parite tablosunu okur.
//!
//! ## Neden tipli
//!
//! Şemanın iki "tam olarak biri" kuralı ([`Input`], [`Expected`]) burada **tip sistemine**
//! gömülüdür: yüklenmiş bir [`Case`] hem `input` hem `input_hex` taşıyamaz, çünkü öyle bir
//! değer inşa edilemez. Doğrulama yalnız sınırda yapılır; aşağıdaki kod dalı taşımaz.
//!
//! ## Hex
//!
//! [`hex`] bu crate'in **tek** hex yardımcısıdır ve `SCHEMA.json` sözleşmesini uygular:
//! yalnız küçük harf `[0-9a-f]`, çift uzunluk, `0x` öneki yok, boşluk yok. Büyük harf sessizce
//! düzeltilmez — hata verir; çünkü büyük harf gören araç bozuk bir üreticiye bakıyordur.
//!
//! ```
//! assert_eq!(kama_vectors::hex("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
//! assert_eq!(kama_vectors::hex("").unwrap(), Vec::<u8>::new()); // sıfır byte GEÇERLİ
//! assert!(kama_vectors::hex("DEADBEEF").is_err()); // büyük harf
//! assert!(kama_vectors::hex("0xdead").is_err()); // 0x öneki
//! assert!(kama_vectors::hex("abc").is_err()); // tek uzunluk
//! ```

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

/// Bu yükleyicinin bildiği tek şema sürümü.
pub const SUPPORTED_SCHEMA_VERSION: u64 = 1;

// ── Hata kodları — `scripts/vectors/validate.py` ile BİREBİR aynı ────────────────────────────

/// JSON olarak ayrıştırılamadı / dosya okunamadı.
pub const SV_PARSE: &str = "SV-001";
/// Aynı nesnede yinelenen JSON anahtarı.
pub const SV_DUP_KEY: &str = "SV-002";
/// `schema_version` yok ya da tamsayı değil.
pub const SV_VERSION_SHAPE: &str = "SV-003";
/// Bilinmeyen `schema_version` — sessiz geçilmez.
pub const SV_VERSION_UNKNOWN: &str = "SV-004";
/// `SCHEMA.json` ihlali.
pub const SV_SCHEMA: &str = "SV-005";
/// Hex alanı kuralı (küçük harf, çift uzunluk, `0x` yok).
pub const SV_HEX: &str = "SV-006";
/// `id` repo genelinde tekil değil.
pub const SV_DUP_ID: &str = "SV-007";
/// Dizgede kontrol karakteri.
pub const SV_CTRL: &str = "SV-008";

// ── Hata tipleri ─────────────────────────────────────────────────────────────────────────────

/// [`hex`] hatası. Yükleyici bunu [`VectorError`] (`SV-006`) içine sarar.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HexError {
    /// Hex çift uzunlukta olmalı; byte yarımı yoktur.
    #[error("tek uzunluk ({0} karakter) — hex çift olmalı")]
    OddLength(usize),
    /// Yalnız küçük harf `[0-9a-f]`. Büyük harf ve `0x` öneki dâhil her şey buraya düşer.
    #[error(
        "geçersiz karakter {ch:?} (konum {index}) — yalnız küçük harf [0-9a-f], '0x' öneki YOK"
    )]
    InvalidChar {
        /// Karakterin dizge içindeki byte konumu.
        index: usize,
        /// Reddedilen karakter.
        ch: char,
    },
}

/// Tek bir doğrulama bulgusu. Biçim Python tarafıyla aynı: `yol/işaretçi: [KOD] mesaj`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path}{pointer}: [{code}] {message}")]
pub struct VectorError {
    /// `SV-001` … `SV-008`.
    pub code: &'static str,
    /// Bulgunun geldiği dosya.
    pub path: String,
    /// Dosya içindeki JSON işaretçisi, örn. `/cases/3/expected/result`.
    pub pointer: String,
    /// İnsan-okunur açıklama.
    pub message: String,
}

impl VectorError {
    fn new(
        code: &'static str,
        path: &str,
        pointer: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let pointer = pointer.into();
        Self {
            code,
            path: path.to_owned(),
            pointer: if pointer.is_empty() {
                "/".to_owned()
            } else {
                pointer
            },
            message: message.into(),
        }
    }
}

/// Bir yükleme çağrısının TÜM bulguları.
///
/// Python doğrulayıcı gibi tek hatada durmaz, hepsini toplar — bir dosyayı düzeltirken
/// kalan ihlalleri tek koşuda görmek için.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorErrors(pub Vec<VectorError>);

impl VectorErrors {
    /// Bulguların hata kodları, görülme sırasında.
    #[must_use]
    pub fn codes(&self) -> Vec<&'static str> {
        self.0.iter().map(|e| e.code).collect()
    }

    /// Bulgular arasında bu kod var mı?
    #[must_use]
    pub fn has_code(&self, code: &str) -> bool {
        self.0.iter().any(|e| e.code == code)
    }
}

impl fmt::Display for VectorErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for VectorErrors {}

// ── Tek hex yardımcısı ───────────────────────────────────────────────────────────────────────

/// `SCHEMA.json` hex sözleşmesi: küçük harf `[0-9a-f]`, çift uzunluk, `0x` yok, boşluk yok.
///
/// Boş dizge geçerlidir ve sıfır byte demektir (örn. ccr §35 `early_data` N=0) — yokluk değildir.
///
/// # Errors
///
/// Tek uzunlukta ([`HexError::OddLength`]) ya da hex olmayan/büyük harf karakter içeriyorsa
/// ([`HexError::InvalidChar`]).
pub fn hex(s: &str) -> Result<Vec<u8>, HexError> {
    if !s.len().is_multiple_of(2) {
        return Err(HexError::OddLength(s.len()));
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for i in (0..bytes.len()).step_by(2) {
        let hi = nibble(bytes[i], i)?;
        let lo = nibble(bytes[i + 1], i + 1)?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn nibble(byte: u8, index: usize) -> Result<u8, HexError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(HexError::InvalidChar {
            index,
            ch: char::from(byte),
        }),
    }
}

// ── Doğrulanmış genel tipler ─────────────────────────────────────────────────────────────────

/// Yüklenmiş ve doğrulanmış bir vektör dosyası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VectorFile {
    /// Her zaman [`SUPPORTED_SCHEMA_VERSION`] — başka bir değer yüklemede reddedilir.
    pub schema_version: u64,
    /// Dosya başına tek suite adı.
    pub suite: String,
    /// Vektörlerin üretildiği donmuş spec sürümü.
    pub spec_version: String,
    /// Serbest metin; araç davranışını etkilemez.
    pub description: Option<String>,
    /// En az bir vaka.
    pub cases: Vec<Case>,
    /// Bu dosyanın geldiği yol (tanı için).
    pub source: String,
}

/// Tek bir vektör vakası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    /// Repo genelinde tekil.
    pub id: String,
    /// Tüketicinin üzerinden dağıtım yaptığı anahtar (`encode`, `decode`, `verify`, …).
    pub kind: String,
    /// Serbest metin; araç davranışını etkilemez.
    pub note: Option<String>,
    /// Otorite belge atfı; araç davranışını etkilemez.
    pub spec_ref: Option<String>,
    /// Girdi — yapısal ya da ham wire, **tam olarak biri** (tip düzeyinde).
    pub input: Input,
    /// Beklenen sonuç — wire ya da ret, **tam olarak biri** (tip düzeyinde).
    pub expected: Expected,
}

/// Vaka girdisi. Şemanın `input | input_hex` "tam olarak biri" kuralının tip karşılığı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Yapısal girdi: alanlardan kodlama yönü. Byte alanları hex dizgedir (anahtarı `_hex`).
    Structured(Map<String, Value>),
    /// Ham wire byte'ları: çözümleme yönü. `input_hex` çözülmüş hâlde.
    Wire(Vec<u8>),
}

/// Beklenen sonuç. Şemanın `expected_hex | expected` "tam olarak biri" kuralının tip karşılığı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expected {
    /// Pozitif vaka: beklenen wire byte'ları (`expected_hex` çözülmüş hâlde).
    Wire(Vec<u8>),
    /// Negatif vaka: beklenen ret.
    Rejected {
        /// Gözlemlenebilir davranış.
        result: RejectResult,
        /// Kararlı, tam-dizge karşılaştırılan gerekçe.
        reason: String,
    },
}

/// Negatif vakanın beklenen davranışı. Üçü farklı **gözlemlenebilir** sonuçtur, eş anlamlı değil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RejectResult {
    /// Sessiz atım: çağıran hiçbir şey görmez, yalnız bir sayaç artar.
    Drop,
    /// Tanımlı ret: tipli hata. Girdi **gerçekten geçersizdir**.
    Reject,
    /// Fail-closed / defer: işlem tamamlanamaz; girdi mutlaka geçersiz **değildir**.
    Fail,
}

impl RejectResult {
    /// Şemadaki dizge karşılığı.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Drop => "drop",
            Self::Reject => "reject",
            Self::Fail => "fail",
        }
    }
}

// ── serde ham şekilleri ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    schema_version: u64,
    suite: String,
    spec_version: String,
    #[serde(default)]
    description: Option<String>,
    cases: Vec<RawCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    id: String,
    kind: String,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    spec_ref: Option<String>,
    #[serde(default)]
    input: Option<Map<String, Value>>,
    #[serde(default)]
    input_hex: Option<String>,
    #[serde(default)]
    expected_hex: Option<String>,
    #[serde(default)]
    expected: Option<RawExpected>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExpected {
    result: RejectResult,
    reason: String,
}

// ── Yinelenen anahtar yakalayan JSON değeri (SV-002) ─────────────────────────────────────────
//
// serde'nin `deny_unknown_fields` türetmesi yinelenen anahtarı YALNIZ bildiği alanlarda görür;
// `input` gibi serbest haritalarda ve iç içe nesnelerde görmez. `serde_json::Value` ise
// yinelenende sessizce SONUNCUYU tutar — tam olarak "iki araç aynı dosyadan farklı byte okur"
// senaryosu. Bu sarmalayıcı belgeyi özyinelemeli gezer ve ilk yinelenmede hata verir; Python
// tarafındaki `object_pairs_hook` ile aynı kapsam.

struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = UniqueValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("yinelenen anahtar taşımayan bir JSON değeri")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Null))
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::Bool(v)))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::from(v)))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::from(v)))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::from(v)))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(UniqueValue(Value::String(v.to_owned())))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(UniqueValue(v)) = seq.next_element()? {
            items.push(v);
        }
        Ok(UniqueValue(Value::Array(items)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut out = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let UniqueValue(value) = map.next_value()?;
            if out.insert(key.clone(), value).is_some() {
                return Err(de::Error::custom(format!("duplicate field `{key}`")));
            }
        }
        Ok(UniqueValue(Value::Object(out)))
    }
}

// ── Yükleme ──────────────────────────────────────────────────────────────────────────────────

/// Tek bir vektör dosyasını yükler ve doğrular.
///
/// # Errors
///
/// Dosya okunamazsa, JSON bozuksa, `schema_version` bilinmiyorsa ya da `SCHEMA.json`
/// kurallarından biri ihlal edilirse. Bulguların tamamı döner.
pub fn load_file(path: impl AsRef<Path>) -> Result<VectorFile, VectorErrors> {
    let mut ids = BTreeMap::new();
    let mut errors = Vec::new();
    let file = load_one(path.as_ref(), &mut ids, &mut errors);
    match (file, errors.is_empty()) {
        (Some(f), true) => Ok(f),
        _ => Err(VectorErrors(errors)),
    }
}

/// Birden çok yolu **tek küme** olarak yükler. Yol bir dizinse özyinelemeli gezilir
/// (`SCHEMA.json` atlanır).
///
/// `id` tekilliği bu kümenin tamamında denetlenir — kural dosya içi değil, repo genelidir.
///
/// # Errors
///
/// Kümedeki herhangi bir dosya doğrulamadan geçemezse; bulguların tamamı döner.
pub fn load_paths<P: AsRef<Path>>(paths: &[P]) -> Result<Vec<VectorFile>, VectorErrors> {
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    let mut errors = Vec::new();
    let mut files = Vec::new();

    let mut targets = Vec::new();
    for p in paths {
        collect(p.as_ref(), &mut targets, &mut errors);
    }
    targets.sort();

    for target in &targets {
        if let Some(file) = load_one(target, &mut ids, &mut errors) {
            files.push(file);
        }
    }

    if errors.is_empty() {
        Ok(files)
    } else {
        Err(VectorErrors(errors))
    }
}

/// Bir dizin ağacındaki tüm vektör dosyalarını yükler. [`load_paths`] kısayolu.
///
/// # Errors
///
/// [`load_paths`] ile aynı.
pub fn load_dir(dir: impl AsRef<Path>) -> Result<Vec<VectorFile>, VectorErrors> {
    load_paths(&[dir.as_ref()])
}

fn collect(path: &Path, out: &mut Vec<std::path::PathBuf>, errors: &mut Vec<VectorError>) {
    if path.is_dir() {
        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(err) => {
                errors.push(VectorError::new(
                    SV_PARSE,
                    &path.display().to_string(),
                    "",
                    format!("dizin okunamadı: {err}"),
                ));
                return;
            }
        };
        for entry in entries.flatten() {
            collect(&entry.path(), out, errors);
        }
        return;
    }
    // Şemanın kendisi vektör dosyası değildir.
    if path.file_name().is_some_and(|n| n == "SCHEMA.json") {
        return;
    }
    if path.extension().is_some_and(|e| e == "json") {
        out.push(path.to_path_buf());
    }
}

fn load_one(
    path: &Path,
    ids: &mut BTreeMap<String, String>,
    errors: &mut Vec<VectorError>,
) -> Option<VectorFile> {
    let display = path.display().to_string();

    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(err) => {
            errors.push(VectorError::new(
                SV_PARSE,
                &display,
                "",
                format!("okunamadı: {err}"),
            ));
            return None;
        }
    };

    // 1) Ayrıştırma — yinelenen anahtar burada yakalanır (her derinlikte).
    let value = match serde_json::from_str::<UniqueValue>(&text) {
        Ok(UniqueValue(v)) => v,
        Err(err) => {
            let msg = err.to_string();
            let code = if msg.starts_with("duplicate field") {
                SV_DUP_KEY
            } else {
                SV_PARSE
            };
            errors.push(VectorError::new(code, &display, "", msg));
            return None;
        }
    };

    // 2) schema_version kapısı — YAPISAL doğrulamadan ÖNCE. Bilinmeyen sürümde şema-ihlali
    //    yığını basıp asıl nedeni gömmeyiz.
    let Value::Object(ref root) = value else {
        errors.push(VectorError::new(
            SV_SCHEMA,
            &display,
            "",
            format!("kök nesne olmalı, gelen {}", kind_of(&value)),
        ));
        return None;
    };
    match root.get("schema_version") {
        None => {
            errors.push(VectorError::new(
                SV_VERSION_SHAPE,
                &display,
                "/schema_version",
                "alan yok",
            ));
            return None;
        }
        Some(v) => match v.as_u64() {
            None => {
                errors.push(VectorError::new(
                    SV_VERSION_SHAPE,
                    &display,
                    "/schema_version",
                    format!("tamsayı olmalı, gelen {} ({v})", kind_of(v)),
                ));
                return None;
            }
            Some(n) if n != SUPPORTED_SCHEMA_VERSION => {
                errors.push(VectorError::new(
                    SV_VERSION_UNKNOWN,
                    &display,
                    "/schema_version",
                    format!(
                        "desteklenmeyen sürüm {n}; bu yükleyici yalnız \
                         {SUPPORTED_SCHEMA_VERSION} biliyor — sessizce GEÇİLMEZ"
                    ),
                ));
                return None;
            }
            Some(_) => {}
        },
    }

    // 3) Yapı — serde: zorunlu alanlar, tipler, bilinmeyen alan (deny_unknown_fields).
    let raw = match serde_json::from_value::<RawFile>(value.clone()) {
        Ok(r) => r,
        Err(err) => {
            errors.push(VectorError::new(SV_SCHEMA, &display, "", err.to_string()));
            return None;
        }
    };

    // 4) Şemayla ifade edilemeyen kurallar (Python tarafıyla aynı).
    deep_lints(&display, &value, errors);

    // 5) Anlamsal kurallar + tipli yapıya dönüştürme.
    let before = errors.len();
    let file = validate(&display, raw, ids, errors);
    if errors.len() > before {
        return None;
    }
    file
}

fn validate(
    display: &str,
    raw: RawFile,
    ids: &mut BTreeMap<String, String>,
    errors: &mut Vec<VectorError>,
) -> Option<VectorFile> {
    if !is_token(&raw.suite, &['-']) {
        errors.push(VectorError::new(
            SV_SCHEMA,
            display,
            "/suite",
            format!(
                "küçük harf token olmalı ^[a-z0-9]+(-[a-z0-9]+)*$ — gelen {:?}",
                raw.suite
            ),
        ));
    }
    if !is_trimmed_nonempty(&raw.spec_version, 128) {
        errors.push(VectorError::new(
            SV_SCHEMA,
            display,
            "/spec_version",
            "boş olmayan, kırpılmış, ≤128 karakter bir dizge olmalı",
        ));
    }
    if raw.cases.is_empty() {
        errors.push(VectorError::new(
            SV_SCHEMA,
            display,
            "/cases",
            "en az 1 eleman olmalı",
        ));
    }

    let mut cases = Vec::with_capacity(raw.cases.len());
    for (i, case) in raw.cases.into_iter().enumerate() {
        let at = format!("/cases/{i}");

        if !is_token(&case.id, &['.', '_', '-', '/']) {
            errors.push(VectorError::new(
                SV_SCHEMA,
                display,
                format!("{at}/id"),
                format!("küçük harf token olmalı — gelen {:?}", case.id),
            ));
        }
        if !is_token(&case.kind, &['.', '_', '-']) {
            errors.push(VectorError::new(
                SV_SCHEMA,
                display,
                format!("{at}/kind"),
                format!("küçük harf token olmalı — gelen {:?}", case.kind),
            ));
        }

        // id repo genelinde tekil.
        match ids.get(&case.id) {
            None => {
                ids.insert(case.id.clone(), format!("{display}{at}"));
            }
            Some(first) => errors.push(VectorError::new(
                SV_DUP_ID,
                display,
                format!("{at}/id"),
                format!(
                    "id {:?} repo genelinde tekil değil; ilk görülme: {first}",
                    case.id
                ),
            )),
        }

        let input = match (case.input, case.input_hex) {
            (Some(map), None) => Some(Input::Structured(map)),
            (None, Some(h)) => {
                decode_at(display, &format!("{at}/input_hex"), &h, errors).map(Input::Wire)
            }
            (Some(_), Some(_)) => {
                errors.push(VectorError::new(
                    SV_SCHEMA,
                    display,
                    &at,
                    "oneOf: `input` ile `input_hex` TAM OLARAK biri olmalı, ikisi de var",
                ));
                None
            }
            (None, None) => {
                errors.push(VectorError::new(
                    SV_SCHEMA,
                    display,
                    &at,
                    "oneOf: `input` ile `input_hex` TAM OLARAK biri olmalı, hiçbiri yok",
                ));
                None
            }
        };

        let expected = match (case.expected_hex, case.expected) {
            (Some(h), None) => {
                decode_at(display, &format!("{at}/expected_hex"), &h, errors).map(Expected::Wire)
            }
            (None, Some(rej)) => {
                if !is_trimmed_nonempty(&rej.reason, 200) {
                    errors.push(VectorError::new(
                        SV_SCHEMA,
                        display,
                        format!("{at}/expected/reason"),
                        "boş olmayan, kırpılmış, ≤200 karakter bir dizge olmalı",
                    ));
                }
                Some(Expected::Rejected {
                    result: rej.result,
                    reason: rej.reason,
                })
            }
            (Some(_), Some(_)) => {
                errors.push(VectorError::new(
                    SV_SCHEMA,
                    display,
                    &at,
                    "oneOf: `expected_hex` ile `expected` TAM OLARAK biri olmalı, ikisi de var",
                ));
                None
            }
            (None, None) => {
                errors.push(VectorError::new(
                    SV_SCHEMA,
                    display,
                    &at,
                    "oneOf: `expected_hex` ile `expected` TAM OLARAK biri olmalı, hiçbiri yok",
                ));
                None
            }
        };

        if let (Some(input), Some(expected)) = (input, expected) {
            cases.push(Case {
                id: case.id,
                kind: case.kind,
                note: case.note,
                spec_ref: case.spec_ref,
                input,
                expected,
            });
        }
    }

    if errors.is_empty() {
        Some(VectorFile {
            schema_version: raw.schema_version,
            suite: raw.suite,
            spec_version: raw.spec_version,
            description: raw.description,
            cases,
            source: display.to_owned(),
        })
    } else {
        None
    }
}

fn decode_at(
    display: &str,
    pointer: &str,
    value: &str,
    errors: &mut Vec<VectorError>,
) -> Option<Vec<u8>> {
    match hex(value) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            errors.push(VectorError::new(SV_HEX, display, pointer, err.to_string()));
            None
        }
    }
}

// ── Şemayla ifade edilemeyen derin kurallar ──────────────────────────────────────────────────

fn deep_lints(display: &str, value: &Value, errors: &mut Vec<VectorError>) {
    walk_strings(value, &mut String::new(), &mut |pointer, text| {
        if text.chars().any(|c| c.is_control()) {
            errors.push(VectorError::new(
                SV_CTRL,
                display,
                pointer,
                "dizgede kontrol karakteri var",
            ));
        }

        let key = nearest_key(pointer);
        if key == "hex" || key.ends_with("_hex") {
            if let Err(err) = hex(text) {
                errors.push(VectorError::new(
                    SV_HEX,
                    display,
                    pointer,
                    format!("hex alanı: {err} — gelen {:?}", truncate(text)),
                ));
            }
        } else if pointer.starts_with("/cases/")
            && pointer.contains("/input/")
            && (text.starts_with("0x") || text.starts_with("0X"))
        {
            errors.push(VectorError::new(
                SV_HEX,
                display,
                pointer,
                format!("'0x' öneki yasak — gelen {:?}", truncate(text)),
            ));
        }
    });
}

fn walk_strings(value: &Value, pointer: &mut String, visit: &mut impl FnMut(&str, &str)) {
    match value {
        Value::String(s) => visit(pointer, s),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&i.to_string());
                walk_strings(item, pointer, visit);
                pointer.truncate(len);
            }
        }
        Value::Object(map) => {
            for (key, item) in map {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(key);
                walk_strings(item, pointer, visit);
                pointer.truncate(len);
            }
        }
        _ => {}
    }
}

/// Dizin segmentlerini atlayarak en yakın nesne anahtarını döndürür.
/// `/cases/3/input/cert_cache_hex/0` → `cert_cache_hex`
fn nearest_key(pointer: &str) -> &str {
    // `split('/').skip(1).rev()` derlenmez: Skip<Split> DoubleEndedIterator değil.
    pointer
        .rsplit('/')
        .find(|s| !s.is_empty() && !s.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or("")
}

fn truncate(s: &str) -> &str {
    match s.char_indices().nth(64) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// `^[a-z0-9]+([sep][a-z0-9]+)*$` — regex bağımlılığı olmadan.
fn is_token(s: &str, separators: &[char]) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut after_separator = true; // başta ayraç olamaz
    for c in s.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            after_separator = false;
        } else if separators.contains(&c) {
            if after_separator {
                return false; // çift ayraç ya da baştaki ayraç
            }
            after_separator = true;
        } else {
            return false;
        }
    }
    !after_separator // sonda ayraç olamaz
}

fn is_trimmed_nonempty(s: &str, max: usize) -> bool {
    !s.is_empty() && s.trim() == s && s.chars().count() <= max
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_and_empty() {
        assert_eq!(hex("").unwrap(), Vec::<u8>::new());
        assert_eq!(hex("00").unwrap(), vec![0x00]);
        assert_eq!(hex("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(hex("000000006a1b2c00").unwrap().len(), 8);
    }

    #[test]
    fn hex_rejects_uppercase_rather_than_normalising_it() {
        // Büyük harf sessizce düzeltilmez: gören araç bozuk bir üreticiye bakıyordur.
        assert_eq!(
            hex("DEADBEEF"),
            Err(HexError::InvalidChar { index: 0, ch: 'D' })
        );
        assert_eq!(
            hex("dead BEEF".replace(' ', "").as_str()),
            Err(HexError::InvalidChar { index: 4, ch: 'B' })
        );
    }

    #[test]
    fn hex_rejects_0x_prefix_odd_length_and_whitespace() {
        assert_eq!(
            hex("0xdead"),
            Err(HexError::InvalidChar { index: 1, ch: 'x' })
        );
        assert_eq!(hex("abc"), Err(HexError::OddLength(3)));
        assert_eq!(hex("ab cd"), Err(HexError::OddLength(5)));
        assert_eq!(hex("abcd\n"), Err(HexError::OddLength(5)));
    }

    #[test]
    fn hex_rejects_non_hex_letters() {
        assert_eq!(hex("zz"), Err(HexError::InvalidChar { index: 0, ch: 'z' }));
        assert_eq!(
            hex("agag"),
            Err(HexError::InvalidChar { index: 1, ch: 'g' })
        );
    }

    #[test]
    fn token_grammar_matches_the_schema_pattern() {
        assert!(is_token("ccr-format", &['-']));
        assert!(is_token("a", &['-']));
        assert!(is_token("v1", &['-']));
        assert!(is_token("suite.case-1_x", &['.', '_', '-']));

        assert!(!is_token("", &['-']));
        assert!(!is_token("-lead", &['-']));
        assert!(!is_token("trail-", &['-']));
        assert!(!is_token("double--sep", &['-']));
        assert!(!is_token("Upper", &['-']));
        assert!(!is_token("has space", &['-']));
        assert!(!is_token("dot.not-allowed", &['-'])); // '.' bu ayraç kümesinde yok
    }

    #[test]
    fn nearest_key_skips_array_indices() {
        assert_eq!(
            nearest_key("/cases/3/input/cert_cache_hex/0"),
            "cert_cache_hex"
        );
        assert_eq!(nearest_key("/cases/0/expected_hex"), "expected_hex");
        assert_eq!(nearest_key("/suite"), "suite");
        assert_eq!(nearest_key(""), "");
    }

    #[test]
    fn trimmed_nonempty_rejects_padding_and_overlength() {
        assert!(is_trimmed_nonempty("ccr-format-spec@v1.0", 128));
        assert!(!is_trimmed_nonempty("", 128));
        assert!(!is_trimmed_nonempty(" lead", 128));
        assert!(!is_trimmed_nonempty("trail ", 128));
        assert!(!is_trimmed_nonempty(&"x".repeat(201), 200));
        assert!(is_trimmed_nonempty(&"x".repeat(200), 200));
    }
}
