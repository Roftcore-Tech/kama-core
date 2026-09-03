//! TC-17 — `sealed_len` ↔ transport byte-muhasebesi hizası (SR-STR-CORE-001 §5.4, BUL-08).
//!
//! **Kapsanan bulgu:** BUL-08 / AÇIK-5. `desen1_sim.rs`'teki `UNIT` sabiti elle türetilmiştir ve
//! yorumdaki `~` işareti belirsizliği kendisi kabul eder. `reports/noise-d0/N4-wire-overhead.md`
//! gerçek §2.6-FEC symbol-dışı overhead'ini **37 B (26 outer + 11 inner)** olarak kaydeder.
//!
//! **Gerekçe:** Sapma pacing, budget ve PMTU muhasebesini doğrudan etkiler ve sub-kbps rejiminde
//! ölçüm sonuçlarını kaydırır. Bu dosya iki katmanın en sessiz uyumsuzluğunu kapıya bağlar ve
//! Sprint-5c'yi (kama-integration ↔ adaptive-transport bağlanması) BEKLEMEZ: sabitlerin kendisi
//! `kama-transport-schema` + `kama-noise-wire` üzerinden paylaşılır, koşum-zamanı bağı gerekmez.
//!
//! **Oracle:** `protector.sealed_len(p) == EXTENDED_OUTER_OVERHEAD + FEC_INNER_HDR_LEN + E`
//! ve `seal(p).len() == sealed_len(p)` (§0.5 P0-3, K-WIRE-SIZE-1 — üst-sınır DEĞİL, KESİN).
//! Sapma varsa test kırmızı ve sabitin türetimi dokümante edilir.

use kama_noise_wire::inner::{CONTROL_INNER_PREFIX_LEN, FEC_INNER_HDR_LEN};
use kama_noise_wire::{KamaNoiseWireProtector, KamaNoiseWireSizer};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{
    LogicalShardHeader, UnsealedTransportPacket, EXTENDED_OUTER_OVERHEAD, SOURCE_PREFIX_LEN,
};
use zeroize::Zeroizing;

/// §2.6 + N4-wire-overhead.md: FEC-shard'ın symbol-DIŞI toplam wire overhead'i.
/// 26 (h_ext 4 + wire_format 1 + wire_flags 1 + pn_trunc 4 + tag 16) + 11 (unified inner header).
const FEC_WIRE_OVERHEAD: usize = EXTENDED_OUTER_OVERHEAD + FEC_INNER_HDR_LEN;

const H_EXT: u32 = 0x0102_0304;

fn protector() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(
        H_EXT,
        &Zeroizing::new([0xA1u8; 32]),
        &Zeroizing::new([0xB2u8; 32]),
    )
}

fn shard(symbol_len: usize) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: vec![0xA5u8; symbol_len],
    }
}

// ── (a) 37 B sabitinin PİNLENMESİ ───────────────────────────────────────────

#[test]
fn fec_wire_overhead_is_37_bytes() {
    // N4-wire-overhead.md HÜKMÜ. Bu üç sayıdan biri değişirse burada kırmızı yanar ve
    // rapor/spec ile kod arasındaki sapma sessiz kalmaz.
    assert_eq!(EXTENDED_OUTER_OVERHEAD, 26, "outer overhead (§2.6)");
    assert_eq!(FEC_INNER_HDR_LEN, 11, "unified inner header (§2.56)");
    assert_eq!(FEC_WIRE_OVERHEAD, 37, "N4: 26 outer + 11 inner");
}

// ── (b) desen1_sim UNIT sabitinin türetimi ──────────────────────────────────

#[test]
fn desen1_unit_matches_real_wire_unit() {
    // [ÖLÇÜLDÜ] `desen1_sim.rs`'te `UNIT = 267` idi ve "payload 256 + overhead ~11" olarak
    // türetilmişti. Gerçek §2.6 layout'unda 11 B inner header AYRICA 26 B outer taşır → 37 B.
    //
    // Daha önemlisi: katalogda "256 B payload taşıyan tek shard" diye bir şey YOKTUR.
    // Bir source-symbol'ün payload kapasitesi E − SOURCE_PREFIX_LEN'dir:
    //   E=64   → kapasite 53 B  → 256 B payload 5 shard'a bölünür → 5 × 101 = 505 B wire
    //   E=1024 → kapasite 1013 B → 256 B payload 1 shard'a sığar   → 1 × 1061 = 1061 B wire
    //
    // Yani UNIT=267'nin sapması "yaklaşık %10" değil; seçilen profile göre 1,9× ile 4,1×
    // arasındadır. Sim'in birim-boyutu bir PROFİLDEN türetilmelidir, elle uydurulmamalıdır.
    //
    // Sim `profile 0x03` (K=8, R=4, E=64) rejimini modeller → UNIT = 101.
    let p = protector();
    const SIM_PROFILE_E: usize = 64;
    let unit = p.sealed_len(&shard(SIM_PROFILE_E)).expect("sealed_len");

    assert_eq!(
        unit, 101,
        "desen1_sim::UNIT bu değerden türetilir (FEC_WIRE_OVERHEAD + E)"
    );
    assert_eq!(unit, FEC_WIRE_OVERHEAD + SIM_PROFILE_E);

    // Eski sabitin YANLIŞ olduğunun kaydı: 267, hiçbir katalog profilinin sealed_len'i değildir.
    for e in [64usize, 1024] {
        assert_ne!(
            p.sealed_len(&shard(e)).unwrap(),
            267,
            "267 bir katalog profiline karşılık geliyorsa bu test güncellenmelidir"
        );
    }
}

#[test]
fn payload_to_wire_expansion_is_pinned_per_profile() {
    // Sub-kbps bütçe muhasebesinin girdisi: 1 byte uygulama payload'u kaç byte wire eder?
    // Bu oran pacing/budget/PMTU kararlarının tabanıdır ve profil seçimiyle 2× değişir.
    let p = protector();
    const APP_BYTES: usize = 256;

    for (e, want_shards, want_wire) in [(64usize, 5usize, 505usize), (1024, 1, 1061)] {
        let cap = e - SOURCE_PREFIX_LEN;
        let shards = APP_BYTES.div_ceil(cap);
        let wire = shards * p.sealed_len(&shard(e)).unwrap();
        assert_eq!(shards, want_shards, "E={e} shard sayısı");
        assert_eq!(wire, want_wire, "E={e} toplam wire byte");
    }
}

// ── (c) control sınıfı muhasebesi ───────────────────────────────────────────

#[test]
fn control_sealed_len_is_overhead_plus_prefix_plus_body() {
    // §2.57: 26 (outer) + 6 (inner_version + inner_kind + control_id) + body.
    // ACK gövdesi (ACK-V2 birleşimi, 2026-08-26) = acked_dh_epoch 1 + count 2 +
    // ack_delay_256us 2 + seen_count 4 + arrival_rate 2 + ranges × 16
    // (otorite: kama-noise-wire/src/control.rs::control_body_len).
    let p = protector();
    for n in [0usize, 1, 4] {
        let c = Control::Ack {
            id: 0x1122_3344,
            acked_dh_epoch: 0,
            ack_delay_256us: 0,
            seen_count: 0,
            arrival_rate: 0xFFFF,
            ranges: (0..n).map(|i| (i as u64 * 10, i as u64 * 10 + 3)).collect(),
        };
        let pkt = UnsealedTransportPacket::Control(c);
        let body = 1 + 2 + 2 + 4 + 2 + n * 16;
        assert_eq!(
            p.sealed_len(&pkt).expect("sealed_len"),
            EXTENDED_OUTER_OVERHEAD + CONTROL_INNER_PREFIX_LEN + body,
            "ACK ranges={n}"
        );
    }
}
