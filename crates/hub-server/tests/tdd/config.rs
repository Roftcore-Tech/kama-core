//! `hub.json` v1 — T-C1, T-C2 (KM-HUB-TDD-001 §6). T-C3 (okunamayan dosya → tek satır, çıkış 1) gerçek
//! ikiliyle `tests/udp_loopback.rs`'te.

use std::collections::BTreeSet;

use hub_server::config::{self, ConfigError};

use crate::harness::{base64, hex, hub_json, hub_public, Phone, HUB_SEED};

fn valid() -> String {
    hub_json(&Phone::directory())
}

fn with(replace: &str, by: &str) -> String {
    let json = valid();
    assert!(json.contains(replace), "parça belgede yok: {replace}");
    json.replacen(replace, by, 1)
}

#[test]
fn the_documented_hub_json_is_accepted_and_derives_cert_ref_and_device_key() {
    let config = config::parse(&valid()).expect("geçerli");
    let alice = Phone::alice();
    assert_eq!(config.listen.to_string(), "127.0.0.1:0");
    assert_eq!(config.hub_pub, hub_public());
    assert_eq!(config.devices.len(), 6);
    assert_eq!(config.devices[0].dest_hash, alice.id);
    assert_eq!(
        config.devices[0].cert_ref, alice.cert_ref,
        "cert_ref = SHA-256(cert)"
    );
    assert_eq!(
        config.devices[0].ed25519_pub,
        alice.key.verifying_key().to_bytes(),
        "cert[0..32]"
    );
}

/// T-C1 — bozuk / bilinmeyen alanlı `hub.json` → başlamaz.
#[test]
fn t_c1_malformed_or_unknown_field_config_is_refused() {
    let alice = Phone::alice();
    let cert = base64(&alice.cert);
    let mut bad_key_cert = alice.cert;
    bad_key_cert[..32].copy_from_slice(&[0xff; 32]);
    let mut other = Phone::bob();
    other.id = alice.id;
    let cases: Vec<(&str, String)> = vec![
        ("boş", String::new()),
        ("JSON değil", "hub = 1".to_owned()),
        ("kesik", valid()[..150].to_owned()),
        (
            "bilinmeyen kök alanı",
            with(
                r#""format_version": 1,"#,
                r#""format_version": 1, "limits": {},"#,
            ),
        ),
        (
            "bilinmeyen cihaz alanı",
            with(r#""name": "alice","#, r#""name": "alice", "role": "x","#),
        ),
        (
            "bilinmeyen anahtar alanı",
            with(r#""pub": ""#, r#""seed": "00", "pub": ""#),
        ),
        ("eksik alan", with(r#""listen": "127.0.0.1:0","#, "")),
        ("eksik cert", with(&format!(r#", "cert": "{cert}""#), "")),
        (
            "sürüm 0",
            with(r#""format_version": 1"#, r#""format_version": 0"#),
        ),
        ("listen ad", with("127.0.0.1:0", "hub.example:47000")),
        (
            "pub büyük harf",
            with(&hex(&hub_public()), &hex(&hub_public()).to_uppercase()),
        ),
        ("priv kısa", with(&hex(&HUB_SEED), &hex(&HUB_SEED)[..62])),
        (
            "pub priv'e ait değil",
            with(
                &hex(&hub_public()),
                &hex(&alice.key.verifying_key().to_bytes()),
            ),
        ),
        (
            "dest_hash kısa",
            with(&hex(&alice.id), &hex(&alice.id)[..60]),
        ),
        ("cert kısa", with(&cert, &cert[..200])),
        ("cert dolgusuz", with(&cert, &cert.replace('=', "A"))),
        ("cert alfabe dışı", with(&cert, &format!("-{}", &cert[1..]))),
        (
            "cert anahtarı nokta değil",
            with(&cert, &base64(&bad_key_cert)),
        ),
        (
            "aynı cert iki kez",
            with(&base64(&Phone::bob().cert), &cert),
        ),
        (
            "aynı dest_hash iki kez",
            with(&hex(&Phone::bob().id), &hex(&alice.id)),
        ),
        ("boş ad", with(r#""name": "alice""#, r#""name": """#)),
    ];
    for (what, json) in cases {
        assert!(config::parse(&json).is_err(), "kabul edildi: {what}");
    }
    let _ = other;
}

/// T-C1 — hata metni dosyadan içerik taşımaz.
#[test]
fn t_c1_errors_never_echo_the_secret() {
    let secret = hex(&HUB_SEED);
    for json in [
        with(r#""format_version": 1,"#, r#""format_version": 1, "x": 1,"#),
        with(&secret, &format!("{}zz", &secret[..62])),
        with(&hex(&hub_public()), &hex(&[7; 32])),
    ] {
        let error = config::parse(&json).expect_err("bozuk");
        let shown = format!("{error} / {error:?}");
        assert!(!shown.contains(&secret[..16]), "{shown}");
    }
    let missing = std::path::Path::new("/nonexistent/kama/hub.json");
    let error = config::load(missing).expect_err("yok");
    assert!(matches!(error, ConfigError::Unreadable(_)));
    assert_eq!(
        error.to_string(),
        "config unreadable: /nonexistent/kama/hub.json"
    );
    // Okunabilen ama UTF-8 olmayan dosya biçim hatasıdır, "unreadable" değil (Deploy T-DEP-6b o satırı
    // izin sorunu kanıtı sayar).
    let path = std::env::temp_dir().join(format!("kama-hub-nonutf8-{}.json", std::process::id()));
    std::fs::write(&path, [0xff, 0xfe, b'{', b'}']).expect("geçici dosya");
    let error = config::load(&path).expect_err("UTF-8 değil");
    let _ = std::fs::remove_file(&path);
    assert!(matches!(error, ConfigError::Invalid(_)), "{error}");
}

/// T-C2 — tek gizli alan `hub_ed25519.priv`; cihaz dosyasından sızmış bir gizli alan taşıyan dosya
/// okunmaz; okunan yapılandırma gizliyi basmaz.
#[test]
fn t_c2_hub_ed25519_priv_is_the_only_secret_field() {
    let document: serde_json::Value = serde_json::from_str(&valid()).expect("JSON");
    let keys = |v: &serde_json::Value| -> BTreeSet<String> {
        v.as_object().expect("nesne").keys().cloned().collect()
    };
    let set = |names: &[&str]| {
        names
            .iter()
            .map(|n| (*n).to_owned())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        keys(&document),
        set(&["format_version", "listen", "hub_ed25519", "devices"])
    );
    assert_eq!(keys(&document["hub_ed25519"]), set(&["priv", "pub"]));
    assert_eq!(
        keys(&document["devices"][0]),
        set(&["name", "dest_hash", "cert"])
    );

    for leaked in [
        "ed25519",
        "identity_ed25519",
        "x25519_static",
        "priv",
        "root_pub",
        "peers",
    ] {
        let json = with(
            r#""name": "alice","#,
            &format!(
                r#""name": "alice", "{leaked}": {{ "priv": "{}" }},"#,
                "11".repeat(32)
            ),
        );
        assert!(
            config::parse(&json).is_err(),
            "cihaz gizli alanı kabul edildi: {leaked}"
        );
    }
    let shown = format!("{:?}", config::parse(&valid()).expect("geçerli"));
    assert!(
        !shown.contains(&hex(&HUB_SEED)) && !shown.contains("72, 72, 72, 72"),
        "{shown}"
    );
    assert!(shown.contains("<redacted>"));
}
