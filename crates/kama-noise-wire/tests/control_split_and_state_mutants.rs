//! MUTATION-GÜDÜMLÜ TESTLER — `kama-noise-wire` (DR-1 bölme, dedup, replay guard, demux).
//!
//! **Kaynak:** `cargo mutants -p kama-noise-wire --jobs 4`
//! [ÖLÇÜLDÜ] `436 mutants tested in 11m: 36 missed, 349 caught, 50 unviable, 1 timeouts`
//!
//! Kaçan 36 mutantın 34'ü bu dosyada hedeflenir (`pn.rs`'in 2'si eşdeğerdir, bkz.
//! `pn_boundary_mutants.rs`). Dağılım tesadüf değil; üç kör nokta var:
//!
//!   **(1) DR-1 cap sınırının KABUL tarafı — 11 mutant, `control.rs::split_for_send`.**
//!   Mevcut testler (`sprint2_control::outgoing_ack_over_cap_split`,
//!   `sack_split_and_within_cap_noop`) "aşınca böl" davranışını sürüyor ama
//!   "TAM cap'te BÖLME" sınırını pinlemiyor. Bir off-by-one, tam sınırdaki bir control'ü
//!   gereksiz böler (fazladan datagram, fazladan control_id) veya cap+1'i bölmeden gönderir
//!   (alıcı fail-closed reddeder → control kanalı sessizce kopar).
//!
//!   **(2) Accessor'lar sabit döndürülebiliyordu — 9 mutant.**
//!   `hub_handle()`, `recv_expected_pn()`, `dh_epoch()`, `retries_done()`,
//!   `has_close_tombstone()`, `seen_count()`, `get_mut()`. Mevcut testler bunları
//!   ÖLÇÜM ARACI olarak kullanıyordu (önce/sonra karşılaştırması) — sabit dönen bir mutant
//!   önce==sonra eşitliğini bozmadığı için kaçıyordu. Çözüm: değeri ARAÇ olarak değil
//!   HEDEF olarak assert etmek.
//!
//!   **(3) Guard/branch silme — 14 mutant.** `RetryClass::of` match arm, dedup'taki `!`,
//!   `PersistentReplayGuard::admit` sınırı, `sealed_len`'in cap-aşımı kolu,
//!   `protector::seal`'daki epoch maskesi (`&` → `|`), `demux::classify` uzunluk kapısı.
//!
//! Koş: `cargo test -p kama-noise-wire --test control_split_and_state_mutants`

use kama_noise_wire::protector::{KamaNoiseWireSizer, WireSizeError};
use kama_noise_wire::{
    decode_control_body, split_for_send, Direction, HandleClass, HandleRouter, InboundControlDedup,
    InboundOutcome, KamaNoiseWireProtector, PersistentReplayGuard, ReplayOutcome, RetryClass,
    SealArgs, Session, SessionId, SessionManager, ACK_MAX_RANGES, RECOVERED_MAX_ESI_RANGES,
    SACK_MAX_GAP_BLOCKS,
};
use kama_transport_schema::control::Control;
use kama_transport_schema::inner_kind::{self, ShardKind};
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn ack(n: usize) -> Control {
    Control::Ack {
        id: 100,
        acked_dh_epoch: 0,
        // ACK-V2 birleşimi (2026-08-26) alanları; bu testler split/state davranışını ölçer,
        // ölçüm-alanları nötr: delay 0, seen 0, arrival_rate = 0xFFFF ölçüm-yok-sentineli.
        ack_delay_256us: 0,
        seen_count: 0,
        arrival_rate: 0xFFFF,
        ranges: (0..n).map(|i| (i as u64 * 10, i as u64 * 10 + 3)).collect(),
    }
}
fn sack(n: usize) -> Control {
    Control::Sack {
        id: 200,
        acked_dh_epoch: 0,
        base_pn: 42,
        blocks: (0..n).map(|i| (i as u16, i as u16 + 1)).collect(),
    }
}
fn recovered(n: usize) -> Control {
    Control::Recovered {
        id: 300,
        block_id: 7,
        esi_ranges: (0..n).map(|i| (i as u16, i as u16 + 1)).collect(),
    }
}

/// Artan control_id üreteci (bölme sonrası her parça yeni id alır).
fn ids() -> impl FnMut() -> u32 {
    let mut n = 1000u32;
    move || {
        n += 1;
        n
    }
}

// ════════════════════════════════════════════════════════════════════════════
// (1) DR-1 cap sınırı — split_for_send  (11 mutant)
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `control.rs:168:14/27`, `182:14/27`, `196:14/31` — match-guard'lar ve `>` operatörleri.
#[test]
fn split_does_not_fire_at_exactly_the_cap() {
    // TAM cap'te bölme OLMAMALI: tek parça, AYNI control_id. Guard `>` yerine `>=` olsaydı
    // sınırdaki her control gereksiz bölünür — fazladan datagram + fazladan control_id, ve
    // alıcı tarafında dedup muhasebesi kayar.
    for (name, c, cap, orig_id) in [
        ("ACK", ack(ACK_MAX_RANGES), ACK_MAX_RANGES, 100u32),
        ("SACK", sack(SACK_MAX_GAP_BLOCKS), SACK_MAX_GAP_BLOCKS, 200),
        (
            "RECOVERED",
            recovered(RECOVERED_MAX_ESI_RANGES),
            RECOVERED_MAX_ESI_RANGES,
            300,
        ),
    ] {
        let out = split_for_send(&c, ids());
        assert_eq!(out.len(), 1, "{name}: tam cap ({cap}) bölündü");
        assert_eq!(out[0], c, "{name}: tam cap'te içerik değişti");
        assert_eq!(out[0].control_id(), orig_id, "{name}: id değişti");
    }
}

#[test]
fn split_fires_at_exactly_cap_plus_one() {
    // cap+1'de bölme OLMALI. Guard `>` yerine `true`/`false` sabitine dönüşürse burada yakalanır.
    for (name, c, cap) in [
        ("ACK", ack(ACK_MAX_RANGES + 1), ACK_MAX_RANGES),
        ("SACK", sack(SACK_MAX_GAP_BLOCKS + 1), SACK_MAX_GAP_BLOCKS),
        (
            "RECOVERED",
            recovered(RECOVERED_MAX_ESI_RANGES + 1),
            RECOVERED_MAX_ESI_RANGES,
        ),
    ] {
        let out = split_for_send(&c, ids());
        assert_eq!(out.len(), 2, "{name}: cap+1 ({}) bölünmedi", cap + 1);
    }
}

#[test]
fn split_preserves_every_element_exactly_once() {
    // Bölme veri KAYBETMEMELİ ve TEKRARLAMAMALI. `chunks()` yanlış boyutla çağrılsa veya
    // guard yanlış dala gitse burada yakalanır.
    let n = ACK_MAX_RANGES * 3 + 7;
    let orig = ack(n);
    let parts = split_for_send(&orig, ids());
    let mut all: Vec<(u64, u64)> = Vec::new();
    for p in &parts {
        match p {
            Control::Ack { ranges, .. } => {
                assert!(
                    ranges.len() <= ACK_MAX_RANGES,
                    "parça cap'i aşıyor: {}",
                    ranges.len()
                );
                assert!(!ranges.is_empty(), "boş parça üretildi");
                all.extend(ranges.iter().copied());
            }
            other => panic!("ACK bölünürken tür değişti: {other:?}"),
        }
    }
    match &orig {
        Control::Ack { ranges, .. } => assert_eq!(&all, ranges, "bölme veri kaybetti/çoğalttı"),
        _ => unreachable!(),
    }
    assert_eq!(parts.len(), n.div_ceil(ACK_MAX_RANGES));
}

/// MUTANT `control.rs:186:26`, `200:26` — `if i == 0 { *id } else { next_id() }` → `!=`.
#[test]
fn split_keeps_the_original_id_only_on_the_first_chunk() {
    // İlk parça ORİJİNAL id'yi korumalı, sonrakiler YENİ id almalı. `==` → `!=` olursa
    // ilk parça yeni id alır, sonrakiler orijinali paylaşır → alıcıda dedup çakışır ve
    // meşru parçalar `DuplicateAckOnly` sayılıp side-effect'siz düşer.
    for (name, c, orig_id) in [
        ("ACK", ack(ACK_MAX_RANGES * 2 + 1), 100u32),
        ("SACK", sack(SACK_MAX_GAP_BLOCKS * 2 + 1), 200),
        (
            "RECOVERED",
            recovered(RECOVERED_MAX_ESI_RANGES * 2 + 1),
            300,
        ),
    ] {
        let parts = split_for_send(&c, ids());
        assert!(parts.len() >= 3, "{name}: en az 3 parça bekleniyordu");
        assert_eq!(
            parts[0].control_id(),
            orig_id,
            "{name}: ilk parça id'si değişti"
        );
        let mut seen = vec![parts[0].control_id()];
        for p in &parts[1..] {
            assert_ne!(
                p.control_id(),
                orig_id,
                "{name}: sonraki parça orijinal id'yi aldı"
            );
            assert!(!seen.contains(&p.control_id()), "{name}: id tekrar etti");
            seen.push(p.control_id());
        }
    }
}

#[test]
fn split_is_a_noop_for_fixed_size_controls() {
    // Sabit-boy control'ler bölünmez. `other => vec![other.clone()]` kolu silinirse yakalanır.
    for c in [
        Control::BlockComplete {
            id: 1,
            block_id: 0,
            final_source_count: 0,
        },
        Control::SessionClose {
            id: 2,
            reason_code: 0,
        },
        Control::ParamAck {
            id: 3,
            param_hash: [0; 32],
        },
    ] {
        let out = split_for_send(&c, ids());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], c);
    }
}

/// MUTANT `control.rs:296:22` — ACK range doğrulaması `if s > e` → `>=`.
#[test]
fn ack_range_accepts_a_single_pn_range() {
    // `s == e` TEK PN'lik geçerli bir aralıktır (bir paket ACK'lenmiş). `>=` olsaydı
    // her tek-paket ACK'i reddedilir ve düşük trafikte ACK kanalı tamamen kapanırdı.
    // (`err_paths_control.rs` bunu RECOVERED için pinliyor; bu ACK kolu.)
    let mut body = vec![0u8]; // acked_dh_epoch
    body.extend_from_slice(&1u16.to_be_bytes()); // count
    body.extend_from_slice(&7u64.to_be_bytes()); // start
    body.extend_from_slice(&7u64.to_be_bytes()); // end == start
    match decode_control_body(inner_kind::ACK, 1, &body) {
        Ok(Control::Ack { ranges, .. }) => assert_eq!(ranges, vec![(7, 7)]),
        other => panic!("s == e tek-PN aralığı reddedildi: {other:?}"),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// (2) Accessor'lar — ölçüm aracı değil, hedef olarak assert
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `protector.rs:266:41` — `args.dh_epoch & 1` → `| 1`.
#[test]
fn sealed_meta_dh_epoch_is_masked_not_forced() {
    // `& 1` maskesi epoch'u 0/1'e indirger. `| 1` olsaydı epoch-0 paketleri meta'da 1
    // görünür ve engine'in epoch muhasebesi (send_generation, renewal) kayardı.
    // Mevcut testler yalnız `dh_epoch == 1` durumunu assert ediyordu — mutant oradan geçiyordu.
    let mut p = KamaNoiseWireProtector::new(0x0102_0304, &key(0x11), &key(0x22));
    let pkt = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 3,
            block_id: 1,
            esi: 0,
            k_actual: 1,
        },
        symbol: vec![0xA5; 64],
    };
    let s0 = p
        .seal(
            &pkt,
            SealArgs {
                wire_pn: 1,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .expect("seal");
    assert_eq!(
        s0.meta.dh_epoch, 0,
        "epoch-0 meta'da 0 kalmalı (`&` maskesi)"
    );

    // Maskeleme: 2 (bit0=0) → 0, 3 (bit0=1) → 1.
    p.install_epoch1_keys(&key(0x33), &key(0x44));
    let s2 = p
        .seal(
            &pkt,
            SealArgs {
                wire_pn: 2,
                dh_epoch: 2,
                send_generation: 0,
            },
        )
        .expect("seal");
    assert_eq!(s2.meta.dh_epoch, 0, "dh_epoch=2 maskelenip 0 olmalı");
}

/// MUTANT `retry.rs:109:9`, `114:9` — `has_close_tombstone`/`seen_count` sabit döndürme.
#[test]
fn dedup_accessors_report_real_state_not_constants() {
    let mut d = InboundControlDedup::new(4);
    // Taze durum: hiçbir id görülmemiş, hiçbir tombstone yok.
    assert_eq!(d.seen_count(), 0, "taze dedup'ta seen_count 0 olmalı");
    assert!(
        !d.has_close_tombstone(1),
        "taze dedup'ta tombstone olmamalı"
    );

    // ACK ekle → seen artar, tombstone artmaz.
    assert_eq!(
        d.admit(&Control::Ack {
            id: 1,
            acked_dh_epoch: 0,
            ack_delay_256us: 0,
            seen_count: 0,
            arrival_rate: 0xFFFF,
            ranges: vec![]
        }),
        InboundOutcome::Fresh
    );
    assert_eq!(d.seen_count(), 1);
    assert!(!d.has_close_tombstone(1), "ACK tombstone üretmemeli");

    // SESSION_CLOSE ekle → tombstone oluşur.
    assert_eq!(
        d.admit(&Control::SessionClose {
            id: 2,
            reason_code: 0
        }),
        InboundOutcome::Fresh
    );
    assert_eq!(d.seen_count(), 2, "seen_count sabit döndürülüyor olabilir");
    assert!(d.has_close_tombstone(2), "SESSION_CLOSE tombstone üretmeli");
    assert!(
        !d.has_close_tombstone(999),
        "görülmemiş id tombstone vermemeli"
    );

    // Tekrar → side-effect yok, seen_count ARTMAZ.
    assert_eq!(
        d.admit(&Control::SessionClose {
            id: 2,
            reason_code: 0
        }),
        InboundOutcome::DuplicateAckOnly
    );
    assert_eq!(d.seen_count(), 2, "tekrar seen_count'u artırdı");
}

/// MUTANT `retry.rs:91:20` — `&& !self.close_tombstones.contains(&id)` içindeki `!` silinmesi.
#[test]
fn tombstone_cap_still_admits_already_known_ids() {
    // Cap dolduğunda: YENİ id tombstone'a EKLENMEZ, ama ZATEN VAR OLAN id tekrar
    // eklenebilir (idempotent). `!` silinirse mantık tersine döner ve cap dolduğunda
    // bilinen id'ler de kaybolur → retry'lara ACK üretilemez.
    let cap = 2usize;
    let mut d = InboundControlDedup::new(cap);
    for id in 1..=cap as u32 {
        d.admit(&Control::SessionClose { id, reason_code: 0 });
        assert!(d.has_close_tombstone(id));
    }
    // Cap dolu. Yeni id → tombstone EKLENMEZ (bounded guard).
    d.admit(&Control::SessionClose {
        id: 99,
        reason_code: 0,
    });
    assert!(
        !d.has_close_tombstone(99),
        "cap dolu iken yeni tombstone eklendi — bounded guard çalışmıyor"
    );
    // Var olanlar KORUNMALI.
    for id in 1..=cap as u32 {
        assert!(
            d.has_close_tombstone(id),
            "cap dolduktan sonra mevcut tombstone {id} kayboldu"
        );
    }
}

/// MUTANT `retry.rs:35:13` — `ACK | SACK | RECOVERED | PARAM_ACK` match arm silinmesi.
#[test]
fn retry_class_table_is_pinned_per_codepoint() {
    // §2.57 retry tablosu tek kaynaktır. Arm silinirse `_ => NoRetry` fallback'i aynı
    // sonucu verir — bu yüzden NoRetry sınıfı TEK BAŞINA mutantı öldürmez. Öldüren şey
    // DİĞER iki sınıfın da pinlenmesi ve tablonun BÜTÜN olarak kontrol edilmesidir.
    assert_eq!(
        RetryClass::of(inner_kind::PARAM_SET),
        RetryClass::UntilParamAck
    );
    for cp in [
        inner_kind::BLOCK_ABORT,
        inner_kind::BLOCK_COMPLETE,
        inner_kind::SESSION_CLOSE,
        inner_kind::SESSION_RENEW_REQUIRED,
    ] {
        assert_eq!(
            RetryClass::of(cp),
            RetryClass::UntilNetworkAck,
            "cp={cp:#04x} yanlış retry sınıfı"
        );
    }
    for cp in [
        inner_kind::ACK,
        inner_kind::SACK,
        inner_kind::RECOVERED,
        inner_kind::PARAM_ACK,
    ] {
        assert_eq!(
            RetryClass::of(cp),
            RetryClass::NoRetry,
            "cp={cp:#04x} yanlış retry sınıfı"
        );
    }
    // Registry dışı → NoRetry (fail-safe: bilinmeyen control retry edilmez).
    assert_eq!(RetryClass::of(0x99), RetryClass::NoRetry);

    // `of_control` `of`'un üstüne kurulur; ikisi tutarlı olmalı.
    let c = Control::ParamSet {
        id: 1,
        param_hash: [0; 32],
        profile_id: 1,
        code_suite_id: 1,
    };
    assert_eq!(RetryClass::of_control(&c), RetryClass::UntilParamAck);
}

// ════════════════════════════════════════════════════════════════════════════
// (3) Guard / branch
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `restart.rs:98:32` — `st.recent.len() >= cap` → `<`.
#[test]
fn replay_guard_evicts_only_when_the_recent_set_is_full() {
    // `>=` → `<` olursa eviction cap dolmadan tetiklenir: her yeni counter en küçüğü atar,
    // recent set 1 elemanda kalır ve pencere-içi replay'ler tespit edilemez hale gelir.
    let cap = 4usize;
    let mut g = PersistentReplayGuard::new(cap);
    let (a, b) = ([1u8; 16], [2u8; 16]);

    // cap kadar artan counter → hepsi Fresh, hepsi recent'te.
    for n in 1..=cap as u64 {
        assert_eq!(g.admit(a, b, n), ReplayOutcome::Fresh, "n={n}");
    }
    // Hepsi hâlâ replay olarak tanınmalı (eviction erken tetiklenmediyse).
    for n in 1..=cap as u64 {
        assert_eq!(
            g.admit(a, b, n),
            ReplayOutcome::ReplayNoSideEffect,
            "n={n} replay olarak tanınmadı — eviction erken tetiklenmiş olabilir"
        );
    }
    // high_water üstü yeni counter → Fresh; en eski evict edilir.
    assert_eq!(g.admit(a, b, cap as u64 + 1), ReplayOutcome::Fresh);
    // Evict edilen de fail-closed reddedilmeli (side-effect YOK).
    assert_eq!(g.admit(a, b, 1), ReplayOutcome::ReplayNoSideEffect);
}

/// MUTANT `demux.rs:40:27` — `datagram.len() < 4` → `==` / `<=`.
#[test]
fn classify_length_gate_is_exact() {
    // Handle 4 byte'tır. Kapı `< 4` olmalı: 0..3 reddedilir, 4 KABUL edilir.
    // `<= 4` olsaydı tam 4B'lik datagram sınıflandırılamazdı; `== 4` olsaydı 0..3
    // arası datagramlar taşma riskiyle okunurdu.
    let mut r = HandleRouter::new();
    r.register_extended(0x0102_0304);
    r.register_legacy(0x0A0B_0C0D);

    for len in 0..4usize {
        assert_eq!(
            r.classify(&vec![0u8; len]),
            None,
            "len={len} sınıflandırıldı"
        );
    }
    // Tam 4 byte, kayıtlı handle → sınıflandırılmalı.
    assert_eq!(
        r.classify(&0x0102_0304u32.to_be_bytes()),
        Some(HandleClass::Extended),
        "tam 4B extended handle sınıflandırılmadı"
    );
    assert_eq!(
        r.classify(&0x0A0B_0C0Du32.to_be_bytes()),
        Some(HandleClass::Legacy)
    );
    // Kayıtsız handle → None (cheap-drop).
    assert_eq!(r.classify(&0xDEAD_BEEFu32.to_be_bytes()), None);
}

/// MUTANT `protector.rs:343:21` — `sealed_len`'in `ControlCountExceedsCap` kolunun silinmesi.
#[test]
fn sealed_len_rejects_over_cap_control_with_the_right_kind() {
    // Cap'i aşan bir control BOYUTLANAMAZ — çağıran önce `split_for_send` yapmalıydı.
    // Hata `kind` alanını taşımalı ki çağıran hangi türü böleceğini bilsin; kol silinirse
    // kind kaybolur ve pacer yanlış türü böler.
    let p = KamaNoiseWireProtector::new(1, &key(1), &key(2));
    let over = UnsealedTransportPacket::Control(ack(ACK_MAX_RANGES + 1));
    assert_eq!(
        p.sealed_len(&over),
        Err(WireSizeError::ControlCountExceedsCap {
            kind: inner_kind::ACK
        }),
        "cap-aşımı ACK yanlış kind ile reddedildi"
    );
    // Cap içindeki aynı control boyutlanabilmeli.
    assert!(p
        .sealed_len(&UnsealedTransportPacket::Control(ack(ACK_MAX_RANGES)))
        .is_ok());
}

/// MUTANT `session.rs:131:9` — `SessionManager::get_mut` → `None`.
#[test]
fn session_manager_get_mut_returns_the_registered_session() {
    // `None` döndüren mutant, "kayıtlı oturum bulunamaz" demektir; çağıran her seferinde
    // yeni oturum kurmaya çalışır. Accessor'ın gerçek veriyi döndürdüğü pinlenmeli.
    let mut m = SessionManager::new();
    let h = 0x1234_5678u32;
    m.install(Session::new(
        SessionId(7),
        Direction::Initiator,
        KamaNoiseWireProtector::new(h, &key(0xA1), &key(0xB2)),
    ));
    assert_eq!(m.active_count(), 1);
    let got = m.get_mut(h).expect("kayıtlı handle bulunamadı");
    assert_eq!(got.session_id(), SessionId(7), "yanlış oturum döndü");
    assert_eq!(got.direction(), Direction::Initiator);
    assert!(m.get_mut(0xDEAD_BEEF).is_none(), "kayıtsız handle bulundu");
}

/// MUTANT `legacy.rs:119:9` (`hub_handle` → 0/1), `123:9` (`recv_expected_pn` → 0/1).
///
/// [ÖLÇÜLDÜ] Bu dört mutant kaçmıştı çünkü mevcut testler bu accessor'ları ÖLÇÜM ARACI
/// olarak kullanıyor: `err_paths_legacy::unknown_handle_does_not_advance_recv_window`
/// önce/sonra karşılaştırması yapıyor ve sabit dönen bir mutant o eşitliği bozmuyor.
/// Değeri ARAÇ olarak değil HEDEF olarak assert etmek gerekiyor.
#[test]
fn legacy_accessors_report_real_state_not_constants() {
    use kama_noise_wire::{LegacyDirection, LegacySessionProtector};
    const HUB: u32 = 0x0A0B_0C0D;
    let sid = [0x5Au8; 16];
    let mk =
        |h: u32, dir| LegacySessionProtector::new(h, &key(0xA1), &key(0xB2), 1, 1, 7, sid, dir);

    // hub_handle: kurulduğu değeri döndürmeli — 0 ve 1 dışında bir değerle pinle.
    let a = mk(HUB, LegacyDirection::AToB);
    assert_eq!(
        a.hub_handle(),
        HUB,
        "hub_handle sabit döndürülüyor olabilir"
    );
    // İkinci bir örnek FARKLI değer döndürmeli (sabit-dönen mutantın ikinci kapısı).
    let b = mk(0x1122_3344, LegacyDirection::BToA);
    assert_eq!(b.hub_handle(), 0x1122_3344);
    assert_ne!(a.hub_handle(), b.hub_handle());

    // recv_expected_pn: taze protector'da 0, açılan her paketten sonra ilerlemeli.
    let mut tx = mk(HUB, LegacyDirection::AToB);
    let mut rx = LegacySessionProtector::new(
        HUB,
        &key(0xB2),
        &key(0xA1),
        1,
        1,
        7,
        sid,
        LegacyDirection::BToA,
    );
    assert_eq!(rx.recv_expected_pn(), 0, "taze pencere 0'dan başlamalı");
    for pn in [1u64, 2, 5, 40] {
        let d = tx.seal(b"x", pn).expect("seal");
        rx.open(&d).expect("open");
        assert_eq!(
            rx.recv_expected_pn(),
            pn + 1,
            "recv_expected_pn gerçek durumu yansıtmıyor (pn={pn})"
        );
    }
}

/// MUTANT `opened.rs:71:9` — `OpenedTransportShard::dh_epoch` → 0.
#[test]
fn opened_shard_accessors_carry_the_real_header_values() {
    // Mevcut testler hep epoch-0 açıyor; `→ 0` mutantı oradan geçiyordu. Epoch-1 bir
    // datagram açıp accessor'ın GERÇEK epoch'u taşıdığını pinle.
    let h = 0x0102_0304u32;
    let mut tx = KamaNoiseWireProtector::new(h, &key(0x11), &key(0x22));
    let mut rx = KamaNoiseWireProtector::new(h, &key(0x22), &key(0x11));
    tx.install_epoch1_keys(&key(0x33), &key(0x44));
    rx.install_epoch1_keys(&key(0x44), &key(0x33));

    let pkt = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 3,
            block_id: 1,
            esi: 0,
            k_actual: 1,
        },
        symbol: vec![0xA5; 64],
    };

    // epoch-0 ve epoch-1 datagramları FARKLI dh_epoch taşımalı.
    let d0 = tx
        .seal(
            &pkt,
            SealArgs {
                wire_pn: 1,
                dh_epoch: 0,
                send_generation: 0,
            },
        )
        .expect("seal e0")
        .bytes;
    let o0 = rx.open(&d0).expect("open e0");
    assert_eq!(o0.dh_epoch(), 0);
    assert_eq!(o0.full_pn(), 1, "full_pn taşınmıyor");
    assert_eq!(o0.h_ext(), h, "h_ext taşınmıyor");

    let d1 = tx
        .seal(
            &pkt,
            SealArgs {
                wire_pn: 7,
                dh_epoch: 1,
                send_generation: 0,
            },
        )
        .expect("seal e1")
        .bytes;
    let o1 = rx.open(&d1).expect("open e1");
    assert_eq!(o1.dh_epoch(), 1, "dh_epoch sabit 0 döndürülüyor olabilir");
    assert_eq!(o1.full_pn(), 7);
}
