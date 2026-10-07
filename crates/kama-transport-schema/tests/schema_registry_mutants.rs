//! MUTATION-GÜDÜMLÜ TESTLER — `kama-transport-schema` (registry + B4 sınırları + codepoint).
//!
//! **Kaynak:** `cargo mutants -p kama-transport-schema`
//! [ÖLÇÜLDÜ] `54 mutants tested in 29s: 11 missed, 39 caught, 4 unviable`
//!
//! Kaçan 11 mutant üç kümede toplanıyordu; üçü de aynı kör noktayı gösteriyor:
//! **bir değerin DOĞRU olduğu hiç assert edilmemiş, yalnız yanlış olanlar reddedilmiş.**
//!
//!   1. `inner_kind.rs:34` `ShardKind::codepoint` → sabit 0/1 döndürülebiliyordu (2 mutant).
//!      Codepoint'ler wire'a yazılıyor; yanlış değer sessiz protokol uyumsuzluğu demektir.
//!   2. `inner_kind.rs:43-45` `from_codepoint` → `None` veya match-arm silinmesi (3 mutant).
//!      Ters yön hiç test edilmemişti.
//!   3. `param.rs:38,41` `ParamFields::validate` B4 sınırları (4 mutant) — sınırın KABUL
//!      tarafı test edilmemiş. `param.rs`'in HİÇ test dosyası yoktu.
//!   4. `control.rs:66` `Control::codepoint` → sabit 0/1 (2 mutant).
//!
//! **Neden bu bir kapsam meselesi değil, sözleşme meselesi:** codepoint'ler §2.56/§2.57
//! registry'sinin wire karşılığıdır. `golden_v0.rs` byte-golden'ları tutuyor ama o vektörler
//! `encode_*` yollarından geçiyor; enum→codepoint eşlemesinin KENDİSİ pinli değildi.
//!
//! Koş: `cargo test -p kama-transport-schema --test schema_registry_mutants`

use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::{self, classify, InnerKind, ShardKind};
use kama_transport_schema::param::{ParamFields, PARAM_DS};
use kama_transport_schema::SchemaError;

// ════════════════════════════════════════════════════════════════════════════
// ShardKind ↔ codepoint (mutant 34:9 ×2, 43:9, 44:13, 45:13)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn shard_kind_codepoints_are_pinned_to_the_registry() {
    // §2.56 registry: FEC_SOURCE=0x01, FEC_REPAIR=0x02. Bu değerler WIRE'a yazılır;
    // değişirse karşı uç paketi yanlış sınıflandırır. Sabit döndüren bir mutant
    // (hep 0 veya hep 1) buradan geçemez çünkü İKİ değer de ayrı ayrı pinli.
    assert_eq!(ShardKind::Source.codepoint(), 0x01);
    assert_eq!(ShardKind::Repair.codepoint(), 0x02);
    assert_eq!(ShardKind::Source.codepoint(), inner_kind::FEC_SOURCE);
    assert_eq!(ShardKind::Repair.codepoint(), inner_kind::FEC_REPAIR);
    // İkisi FARKLI olmalı — sabit-döndüren mutantın ikinci kapısı.
    assert_ne!(ShardKind::Source.codepoint(), ShardKind::Repair.codepoint());
}

#[test]
fn shard_kind_from_codepoint_is_the_exact_inverse() {
    // Ters yön hiç test edilmemişti: `from_codepoint` → `None` mutantı kaçmıştı.
    assert_eq!(ShardKind::from_codepoint(0x01), Some(ShardKind::Source));
    assert_eq!(ShardKind::from_codepoint(0x02), Some(ShardKind::Repair));

    // Roundtrip: her iki yön birbirinin tersi olmalı.
    for k in [ShardKind::Source, ShardKind::Repair] {
        assert_eq!(
            ShardKind::from_codepoint(k.codepoint()),
            Some(k),
            "codepoint → from_codepoint roundtrip bozuk: {k:?}"
        );
    }

    // FEC-dışı her codepoint None vermeli (fail-closed; control-branch'ine düşsün).
    let mut none_count = 0usize;
    for cp in 0..=255u8 {
        if cp == 0x01 || cp == 0x02 {
            continue;
        }
        assert_eq!(
            ShardKind::from_codepoint(cp),
            None,
            "FEC-dışı codepoint {cp:#04x} ShardKind döndürdü"
        );
        none_count += 1;
    }
    assert_eq!(
        none_count, 254,
        "FEC codepoint sayısı değişmiş — §2.56 güncellenmiş olabilir"
    );
}

#[test]
fn classify_routes_fec_and_control_to_the_right_branch() {
    // `classify` iki parser arasında seçim yapar; yanlış dal parse-confusion demektir.
    assert_eq!(
        classify(inner_kind::FEC_SOURCE),
        Ok(InnerKind::Fec(ShardKind::Source))
    );
    assert_eq!(
        classify(inner_kind::FEC_REPAIR),
        Ok(InnerKind::Fec(ShardKind::Repair))
    );
    // [D2 2026-10-08] ~~ACK/SACK~~ listeden çıktı — Ack/Sack ailesi kodda yok (K3).
    for cp in [
        inner_kind::RECOVERED,
        inner_kind::BLOCK_COMPLETE,
        inner_kind::PARAM_SET,
        inner_kind::PARAM_ACK,
        inner_kind::BLOCK_ABORT,
        inner_kind::SESSION_CLOSE,
        inner_kind::SESSION_RENEW_REQUIRED,
    ] {
        assert_eq!(
            classify(cp),
            Ok(InnerKind::Control(cp)),
            "control codepoint {cp:#04x} yanlış dala gitti"
        );
    }
    // [D2 2026-10-08] RESERVED → reject: eski ACK/SACK codepoint'leri (0x10/0x11) hiçbir dala
    // gitmez — tipli `UnknownInnerKind` (fail-closed).
    assert_eq!(inner_kind::RESERVED_ACK, 0x10);
    assert_eq!(inner_kind::RESERVED_SACK, 0x11);
    for cp in [inner_kind::RESERVED_ACK, inner_kind::RESERVED_SACK] {
        assert_eq!(
            classify(cp),
            Err(SchemaError::UnknownInnerKind(cp)),
            "reserved {cp:#04x} reddedilmedi"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Control::codepoint (mutant control.rs:66:9 ×2)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn control_codepoints_are_pinned_and_pairwise_distinct() {
    // §2.57: her control türü kendi codepoint'ini taşır. Sabit döndüren mutant, dokuz
    // türün AYRI AYRI pinlenmesiyle ve hepsinin FARKLI olmasıyla iki kez yakalanır.
    // [D2 2026-10-08] ~~Ack/Sack~~ çıktı (aile kodda yok, K3); CsProbe/CsEcho (DR-CS-1) eklendi →
    // `Control::codepoint`'in güncel dokuz kolunun hepsi pinli.
    let cases: [(Control, u8); 9] = [
        (
            Control::Recovered {
                id: 1,
                block_id: 0,
                esi_ranges: vec![],
            },
            inner_kind::RECOVERED,
        ),
        (
            Control::BlockComplete {
                id: 1,
                block_id: 0,
                final_source_count: 0,
            },
            inner_kind::BLOCK_COMPLETE,
        ),
        (
            Control::ParamSet {
                id: 1,
                param_hash: [0; 32],
                profile_id: 1,
                code_suite_id: 1,
            },
            inner_kind::PARAM_SET,
        ),
        (
            Control::ParamAck {
                id: 1,
                param_hash: [0; 32],
            },
            inner_kind::PARAM_ACK,
        ),
        (
            Control::BlockAbort {
                id: 1,
                block_id: 0,
                reason_code: 0,
            },
            inner_kind::BLOCK_ABORT,
        ),
        (
            Control::SessionClose {
                id: 1,
                reason_code: 0,
            },
            inner_kind::SESSION_CLOSE,
        ),
        (
            Control::SessionRenewRequired {
                id: 1,
                reason_code: 0,
                renew_hint: 0,
            },
            inner_kind::SESSION_RENEW_REQUIRED,
        ),
        (
            Control::CsProbe {
                id: 1,
                subtype: 1,
                probe_seq: 0,
            },
            inner_kind::CTRL_CS_PROBE,
        ),
        (
            Control::CsEcho {
                id: 1,
                subtype: 1,
                probe_seq: 0,
                seen_count: 0,
                arrival_gap: 0,
            },
            inner_kind::CTRL_CS_ECHO,
        ),
    ];

    let mut seen: Vec<u8> = Vec::new();
    for (c, want) in &cases {
        assert_eq!(c.codepoint(), *want, "{c:?} yanlış codepoint");
        assert!(
            !seen.contains(&c.codepoint()),
            "codepoint {:#04x} iki türde tekrar ediyor",
            c.codepoint()
        );
        seen.push(c.codepoint());
    }
    assert_eq!(seen.len(), 9);
    // Registry aralığı §2.57: ~~0x10..=0x18~~ 0x12..=0x1A ([D2 2026-10-08] 0x10/0x11 RESERVED;
    // DR-CS-1 0x19/0x1A).
    assert!(
        seen.iter().all(|&cp| (0x12..=0x1A).contains(&cp)),
        "control codepoint'i registry aralığı dışında: {seen:?}"
    );
    // [D2 2026-10-08] RESERVED → reject: hiçbir canlı varyant 0x10/0x11 taşımaz.
    for cp in [inner_kind::RESERVED_ACK, inner_kind::RESERVED_SACK] {
        assert!(
            !seen.contains(&cp),
            "reserved {cp:#04x} bir varyanta atanmış"
        );
    }
}

#[test]
fn control_id_is_carried_not_recomputed() {
    // `control_id()` accessor'ı da sabit döndürülebilir. Farklı id'lerle pinle.
    for id in [0u32, 1, 0x4142_4344, u32::MAX] {
        let c = Control::SessionClose { id, reason_code: 7 };
        assert_eq!(c.control_id(), id, "control_id taşınmıyor");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ParamFields::validate — B4 sınırları (mutant param.rs:38:25 ×2, 38:45, 41:19)
// ════════════════════════════════════════════════════════════════════════════

fn params(k: u16, e: u16) -> ParamFields {
    ParamFields {
        protocol_version: 1,
        fec_version: 1,
        code_suite_id: 1,
        profile_id: 1,
        k_shape: k,
        r_code_max: 2,
        e,
        partial_policy: 0,
        packet_scheduling_policy: 0,
    }
}

#[test]
fn param_validate_accepts_the_exact_boundary_values() {
    // §8 B4-invariant: 1 ≤ K_shape ≤ 255, E ≥ 11.
    // [ÖLÇÜLDÜ] `param.rs`'in HİÇ test dosyası yoktu; dört sınır mutantı da kaçmıştı.
    // Sınırın KABUL tarafı pinlenmezse bir off-by-one meşru PARAM_SET'i reddeder ve
    // capability negotiation sessizce çöker.
    assert_eq!(
        params(1, 11).validate(),
        Ok(()),
        "K_shape=1, E=11 kabul edilmeli"
    );
    assert_eq!(
        params(255, 11).validate(),
        Ok(()),
        "K_shape=255 kabul edilmeli"
    );
    assert_eq!(
        params(8, 1024).validate(),
        Ok(()),
        "tipik profil kabul edilmeli"
    );
}

#[test]
fn param_validate_rejects_just_outside_the_boundary() {
    for (k, e, why) in [
        (0u16, 11u16, "K_shape=0"),
        (256, 11, "K_shape=256"),
        (8, 10, "E=10"),
        (8, 0, "E=0"),
    ] {
        assert!(
            matches!(params(k, e).validate(), Err(SchemaError::ProfileBound(_))),
            "{why} reddedilmeliydi"
        );
    }
}

#[test]
fn param_canonical_hash_input_is_deterministic_and_field_sensitive() {
    // §8: canonical hash-girdisi `DS || fields`, big-endian, sıra-sabit. Bu, capability
    // negotiation'ın eşitlik testidir; alan sırası veya endianness kayarsa iki uç
    // aynı parametreyi farklı hash'ler ve negotiation başarısız olur.
    let a = params(8, 1024);
    let h1 = a.canonical_hash_input().expect("geçerli param");
    let h2 = a.canonical_hash_input().expect("geçerli param");
    assert_eq!(
        h1, h2,
        "aynı girdi farklı byte üretti — deterministik değil"
    );
    assert!(h1.starts_with(PARAM_DS), "domain-separator başta olmalı");

    // Her alan hash'i ETKİLEMELİ — biri düşerse iki farklı parametre aynı hash'i alır.
    let b = params(16, 1024);
    assert_ne!(
        h1,
        b.canonical_hash_input().expect("geçerli param"),
        "K_shape hash girdisini etkilemiyor"
    );
    let c = params(8, 64);
    assert_ne!(
        h1,
        c.canonical_hash_input().expect("geçerli param"),
        "E hash girdisini etkilemiyor"
    );

    // Geçersiz parametre hash üretmemeli (fail-closed).
    assert!(params(0, 11).canonical_hash_input().is_err());
}
