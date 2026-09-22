//! Python doğrulayıcı ile Rust yükleyicisi arasındaki PARİTE testi (T-00.2).
//!
//! İki araç aynı dosyayı aynı kurallarla okumalı. Bu test bunu iki katmanda kanıtlar:
//!
//! 1. **Tablo paritesi** — her iki taraf da `scripts/vectors/testdata/EXPECTATIONS.json`'daki
//!    tek listeden sürülür. Rust yükleyicisi `accept` girdilerini yüklemeli, `reject`
//!    girdilerinde ise **beklenen `SV-` koduyla** düşmelidir; yanlış nedenle düşen fixture
//!    yeşil sayılmaz.
//! 2. **Araç paritesi** — aynı yollar `python3 scripts/vectors/validate.py`'ye de verilir ve
//!    iki aracın HÜKMÜ (kabul/ret) ile ret kodunun aynı olduğu doğrulanır. `python3` yoksa bu
//!    katman atlanır ve atlandığı basılır (Rust job'unun tek bağımlılığı Rust kalsın diye).

use std::path::{Path, PathBuf};
use std::process::Command;

use kama_vectors::{Expected, Input, RejectResult, VectorErrors};
use serde_json::Value;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/kama-vectors iki üstü repo kökü")
        .to_path_buf()
}

fn expectations() -> Value {
    let path = repo_root().join("scripts/vectors/testdata/EXPECTATIONS.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("parite tablosu okunamadı {}: {e}", path.display()));
    serde_json::from_str(&text).expect("EXPECTATIONS.json geçerli JSON")
}

fn entry_paths(entry: &Value) -> Vec<PathBuf> {
    let root = repo_root();
    entry["paths"]
        .as_array()
        .expect("paths dizi")
        .iter()
        .map(|p| root.join(p.as_str().expect("paths elemanı dizge")))
        .collect()
}

fn label(entry: &Value) -> &str {
    entry["label"].as_str().unwrap_or("<etiketsiz>")
}

/// Tablo girdisi kendi dosya-boyutu sınırını taşıyabilir (8 MiB'lik gerçek bir fixture
/// repoya konmasın diye). Taşımıyorsa ürün sınırı kullanılır.
fn entry_limit(entry: &Value) -> u64 {
    entry["max_file_bytes"]
        .as_u64()
        .unwrap_or(kama_vectors::MAX_FILE_BYTES)
}

/// Girdiyi tablodaki sınırla yükler.
fn load_entry(entry: &Value) -> Result<Vec<kama_vectors::VectorFile>, VectorErrors> {
    kama_vectors::load_paths_bounded(&entry_paths(entry), entry_limit(entry))
}

// ── 1. Tablo paritesi ────────────────────────────────────────────────────────────────────────

#[test]
fn accepts_every_expected_accept_entry() {
    let table = expectations();
    let entries = table["accept"].as_array().expect("accept dizi");
    assert!(!entries.is_empty(), "parite tablosunda accept girdisi yok");

    for entry in entries {
        match load_entry(entry) {
            Ok(files) => assert!(
                !files.is_empty(),
                "kabul: {} — 0 dosya yüklendi (yol yanlış olabilir)",
                label(entry)
            ),
            Err(errors) => panic!("kabul: {} — yükleyici REDDETTİ:\n{errors}", label(entry)),
        }
    }
}

#[test]
fn rejects_every_expected_reject_entry_with_the_expected_code() {
    let table = expectations();
    let entries = table["reject"].as_array().expect("reject dizi");
    assert!(!entries.is_empty(), "parite tablosunda reject girdisi yok");

    for entry in entries {
        let code = entry["code"].as_str().expect("reject girdisinde code yok");
        match load_entry(entry) {
            Ok(_) => panic!("ret: {} — yükleyici KABUL etti; sessiz geçiş", label(entry)),
            Err(errors) => assert!(
                errors.has_code(code),
                "ret: {} — reddedildi ama {code} yok (gelen {:?}):\n{errors}",
                label(entry),
                errors.codes()
            ),
        }
    }
}

// ── 2. Araç paritesi: aynı yollar, aynı hüküm ────────────────────────────────────────────────

/// `python3 scripts/vectors/validate.py <paths>` çalıştırır.
/// `None` = python3 yok (katman atlanır).
fn python_verdict(paths: &[PathBuf], max_bytes: u64) -> Option<(bool, String)> {
    let out = Command::new("python3")
        .arg(repo_root().join("scripts/vectors/validate.py"))
        .args(paths)
        .arg("--max-file-bytes")
        .arg(max_bytes.to_string())
        .current_dir(repo_root())
        .output()
        .ok()?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), text))
}

#[test]
fn python_validator_and_rust_loader_reach_the_same_verdict() {
    let table = expectations();
    let mut checked = 0usize;

    let accept = table["accept"].as_array().expect("accept dizi").iter();
    let reject = table["reject"].as_array().expect("reject dizi").iter();

    for entry in accept.chain(reject) {
        let paths = entry_paths(entry);
        let rust_ok = load_entry(entry).is_ok();

        let Some((py_ok, py_out)) = python_verdict(&paths, entry_limit(entry)) else {
            eprintln!("python3 bulunamadı — araç-paritesi katmanı ATLANDI");
            return;
        };
        checked += 1;

        assert_eq!(
            rust_ok,
            py_ok,
            "hüküm ayrışması — {}: rust_ok={rust_ok} python_ok={py_ok}\npython çıktısı:\n{py_out}",
            label(entry)
        );

        if !rust_ok {
            let code = entry["code"].as_str().unwrap_or_default();
            assert!(
                py_out.contains(&format!("[{code}]")),
                "kod ayrışması — {}: python {code} basmadı\n{py_out}",
                label(entry)
            );
        }
    }

    assert!(checked > 0, "hiçbir girdi karşılaştırılmadı");
    eprintln!("araç paritesi: {checked} girdi, iki araç da aynı hükmü verdi");
}

// ── 3. Done kriteri: ikisi de AYNI örnek dosyayı yüklüyor ────────────────────────────────────

#[test]
fn loads_the_positive_example_file() {
    let path = repo_root().join("vectors/examples/example-positive.json");
    let file = kama_vectors::load_file(&path).expect("pozitif örnek yüklenmeli");

    assert_eq!(file.schema_version, kama_vectors::SUPPORTED_SCHEMA_VERSION);
    assert_eq!(file.suite, "ccr-compact-device-cert-v1");
    assert_eq!(file.cases.len(), 2);

    // encode vakası: yapısal girdi → 152B beklenen wire (ccr §30.4 CompactDeviceCertV1).
    let encode = &file.cases[0];
    assert_eq!(encode.kind, "encode");
    let Input::Structured(ref fields) = encode.input else {
        panic!("encode vakası yapısal girdi taşımalı");
    };
    assert_eq!(
        fields["not_after_hex"].as_str(),
        Some("000000006a1b2c00"),
        "ccr §30.4 not_after u64_be"
    );
    let Expected::Wire(ref wire) = encode.expected else {
        panic!("encode vakası pozitif olmalı");
    };
    assert_eq!(wire.len(), 152, "CompactDeviceCertV1 sabit 152B");
    assert_eq!(&wire[..4], &[0x11, 0x11, 0x11, 0x11]);

    // decode vakası: ham wire → kanonik yeniden-kodlama byte-özdeş.
    let decode = &file.cases[1];
    let (Input::Wire(ref input), Expected::Wire(ref expected)) = (&decode.input, &decode.expected)
    else {
        panic!("decode vakası ham wire girdisi + pozitif beklenti taşımalı");
    };
    assert_eq!(input, expected, "kanonik roundtrip byte-özdeş olmalı");
}

#[test]
fn loads_the_negative_example_file() {
    let path = repo_root().join("vectors/examples/example-negative.json");
    let file = kama_vectors::load_file(&path).expect("negatif örnek yüklenmeli");

    assert_eq!(file.suite, "ccr-parser-reject-v1");
    assert_eq!(file.cases.len(), 4);

    for case in &file.cases {
        assert!(
            matches!(case.expected, Expected::Rejected { .. }),
            "negatif dosyadaki her vaka ret beklemeli: {}",
            case.id
        );
    }

    // INVALID-13 `fail` (fail-closed/defer), `reject` DEĞİL — ayrım korunmalı.
    let invalid13 = file
        .cases
        .iter()
        .find(|c| c.id.contains("invalid-13"))
        .expect("invalid-13 vakası");
    let Expected::Rejected { result, ref reason } = invalid13.expected else {
        unreachable!()
    };
    assert_eq!(result, RejectResult::Fail);
    assert_eq!(reason, "invalid-13-cert-ref-cache-miss");

    let reject_count = file
        .cases
        .iter()
        .filter(|c| {
            matches!(
                c.expected,
                Expected::Rejected {
                    result: RejectResult::Reject,
                    ..
                }
            )
        })
        .count();
    assert_eq!(reject_count, 3);
}

// ── 4. Şema ihlalinde YÜKLEYİCİ DE hata veriyor (Done kriteri) ───────────────────────────────

#[test]
fn unknown_schema_version_is_an_error_not_a_silent_skip() {
    let path = repo_root().join("scripts/vectors/testdata/reject/schema-version-3.json");
    let Err(errors) = kama_vectors::load_file(&path) else {
        panic!("schema_version: 3 sessizce kabul edildi");
    };
    assert!(
        errors.has_code(kama_vectors::SV_VERSION_UNKNOWN),
        "{errors}"
    );
    // Sürüm kapısı yapısal doğrulamadan ÖNCE: tek ve net bulgu, şema-ihlali yığını değil.
    assert_eq!(
        errors.0.len(),
        1,
        "sürüm kapısı tek bulgu üretmeli:\n{errors}"
    );
}

#[test]
fn id_uniqueness_is_repo_wide_not_per_file() {
    let root = repo_root();
    let a = root.join("scripts/vectors/testdata/reject/dup-id-a.json");
    let b = root.join("scripts/vectors/testdata/reject/dup-id-b.json");

    assert!(kama_vectors::load_file(&a).is_ok(), "a tek başına geçmeli");
    assert!(kama_vectors::load_file(&b).is_ok(), "b tek başına geçmeli");

    let Err(errors) = kama_vectors::load_paths(&[a, b]) else {
        panic!("aynı id iki dosyada — birlikte yüklenince hata olmalı");
    };
    assert!(errors.has_code(kama_vectors::SV_DUP_ID), "{errors}");
}

#[test]
fn the_schema_file_itself_is_not_loaded_as_a_vector() {
    let files = kama_vectors::load_dir(repo_root().join("vectors")).expect("vectors/ yüklenmeli");
    assert!(
        files.iter().all(|f| !f.source.ends_with("SCHEMA.json")),
        "SCHEMA.json vektör dosyası olarak yüklenmemeli"
    );
    assert!(!files.is_empty(), "vectors/ altında en az örnekler olmalı");
}

#[test]
fn every_loaded_case_id_is_unique_across_the_whole_vectors_tree() {
    let files = kama_vectors::load_dir(repo_root().join("vectors")).expect("vectors/ yüklenmeli");
    let mut seen = std::collections::BTreeSet::new();
    for file in &files {
        for case in &file.cases {
            assert!(seen.insert(case.id.clone()), "tekrarlı id: {}", case.id);
        }
    }
}

// ── 5. Hata tipinin kendisi ──────────────────────────────────────────────────────────────────

#[test]
fn error_display_carries_the_sv_code() {
    let path = repo_root().join("scripts/vectors/testdata/reject/hex-uppercase.json");
    let Err(VectorErrors(errors)) = kama_vectors::load_file(&path) else {
        panic!("büyük harf hex kabul edildi");
    };
    let text = errors[0].to_string();
    assert!(
        text.contains("[SV-006]"),
        "hata metni kodu taşımalı: {text}"
    );
}

// ── 6. Dizin gezintisi: sembolik dizin bağı izlenmez ─────────────────────────────────────────

/// `is_dir()` sembolik bağı izler; bir dizin kendine bağlanırsa gezinti döner durur ve aynı
/// dosyayı defalarca yükleyip sahte `SV-007` (id tekrarı) üretir. Python tarafındaki
/// `Path.rglob` bağlı dizinlere inmez — iki aracın aynı dosya kümesini görmesi gerekir.
#[cfg(unix)]
#[test]
fn directory_symlink_loops_do_not_multiply_files() {
    use std::os::unix::fs::symlink;

    let dir = std::env::temp_dir().join(format!("kama-vectors-symlink-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("geçici dizin");

    std::fs::write(
        dir.join("v.json"),
        r#"{"schema_version":2,"suite":"cyc","spec_version":"selftest@v1","case_count":1,
            "cases":[{"id":"cyc.c1","kind":"decode","input_hex":"00","expected_hex":"00"}]}"#,
    )
    .expect("fixture yazımı");
    symlink(&dir, dir.join("self")).expect("kendine bağlı dizin");

    let loaded = kama_vectors::load_dir(&dir);
    let _ = std::fs::remove_dir_all(&dir);

    let files = loaded.expect("döngü sahte bulgu üretmemeli");
    assert_eq!(files.len(), 1, "aynı dosya bir kez görülmeli");
    assert_eq!(files[0].cases.len(), 1);
}

// ── 7. schema_version 2 alanları ─────────────────────────────────────────────────────────────

#[test]
fn v2_fields_are_loaded_and_flags_resolve_to_notes() {
    let file = kama_vectors::load_file(repo_root().join("vectors/examples/example-positive.json"))
        .expect("pozitif örnek");

    assert_eq!(file.schema_version, 2);
    assert!(file.generator.is_some(), "provenance alanı taşınmalı");

    // `placeholder-signature` bu dosyanın dürüstlük şerhinin MAKİNE-OKUNUR hâli:
    // tüketici bunu görüp vakayı kriptografik doğruluk kanıtı saymamalı.
    let flag = "placeholder-signature";
    assert!(file.notes.contains_key(flag), "flag sözlükte açıklanmalı");
    assert!(
        file.cases.iter().all(|c| c.flags.iter().any(|f| f == flag)),
        "örnek dosyanın her vakası bu flag'i taşımalı"
    );
    // Yükleme zaten doğrular; burada sözleşmeyi testte de sabitliyoruz.
    for case in &file.cases {
        for f in &case.flags {
            assert!(file.notes.contains_key(f), "tanımsız flag: {f}");
        }
    }
}

#[test]
fn case_count_matches_and_is_the_consumer_vacuum_guard() {
    let file = kama_vectors::load_file(repo_root().join("vectors/examples/example-negative.json"))
        .expect("negatif örnek");
    assert_eq!(file.case_count(), 4);

    // Tüketici vakumu: kind'a göre filtreleyip HİÇ vaka koşmamak sessizce yeşil görünür.
    // Sayı dosyadan geldiği için bu ayırt edilebilir.
    let verify_cases = file.cases.iter().filter(|c| c.kind == "verify").count();
    assert!(verify_cases > 0 && verify_cases < file.case_count());
}

#[test]
fn require_kinds_rejects_a_kind_the_consumer_did_not_declare() {
    let file = kama_vectors::load_file(repo_root().join("vectors/examples/example-negative.json"))
        .expect("negatif örnek");

    file.require_kinds(&["decode", "verify"])
        .expect("ikisi de beyan edildi");

    let Err(errors) = file.require_kinds(&["decode"]) else {
        panic!("beyan edilmeyen 'verify' sessizce atlandı");
    };
    assert!(errors.has_code(kama_vectors::SV_UNKNOWN_KIND), "{errors}");
}
