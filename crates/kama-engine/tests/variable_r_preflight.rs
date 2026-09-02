//! VARIABLE-R PREFLIGHT (Commit-E; §6) — DR-1a-öncesi receiver/codec variable-r-sözleşme-KANITI.
//! PRODUCTION-ACTUATION-KODU-YOK; mevcut-yüzeyler (protector/admit_and_decode/codec) kullanılır.
//! Kanıt-hedefi: alıcı EXACT-R-beklemez; kaç-repair-geldiğini-dinamik-kabul-eder; r=0-leak-üretmez;
//! ardışık-bloklarda-farklı-r decoder'ı-bozmaz; wire/profil-değişikliği-GEREKMEZ.

mod common;

use kama_engine::{admit_and_decode, EngineError};
use kama_fec_codec::by_id;
use kama_noise_wire::{KamaNoiseWireProtector, OpenedTransportShard};

const PID: u8 = 0x03; // K8 R4 E64 (Rmax=4)

fn app(n_units: usize) -> Vec<u8> {
    vec![0x33u8; common::PAYLOAD_CAP * n_units]
}

/// Bir bloğu emit_r-repair'le seal + seçilen-source'ları-düşür + (ops.) repair-alt-kümesi + sıra-düzeni.
#[allow(clippy::too_many_arguments)]
fn run_block(
    tx: &mut KamaNoiseWireProtector,
    rx: &mut KamaNoiseWireProtector,
    block_id: u32,
    k_units: usize,
    base_pn: u64,
    emit_r: usize,
    drop_source_esi: &[u16],
    take_repair: Option<usize>, // None=hepsi; Some(n)=ilk-n-repair
    reverse_order: bool,
) -> Result<Vec<u8>, EngineError> {
    let (sealed, k_actual) = common::encode_seal_block_with_r(
        tx,
        &by_id(PID).unwrap(),
        &app(k_units),
        block_id,
        0x7000 + block_id,
        base_pn,
        emit_r,
    );
    let mut dgs: Vec<&common::SealedShard> = sealed
        .iter()
        .filter(|s| s.is_repair || !drop_source_esi.contains(&s.esi))
        .collect();
    if let Some(n) = take_repair {
        let mut kept = 0usize;
        dgs.retain(|s| {
            if s.is_repair {
                kept += 1;
                kept <= n
            } else {
                true
            }
        });
    }
    if reverse_order {
        dgs.reverse(); // repair-önce-source-sonra (out-of-order)
    }
    let opened: Vec<OpenedTransportShard> = dgs
        .iter()
        .map(|s| rx.open(&s.datagram).expect("authenticated"))
        .collect();
    admit_and_decode(opened, PID, k_actual)
}

// ── §6.1: ardışık-bloklarda farklı-r (A:r=0, B:r=1, C:r=Rmax=4, D:r=2) — exact-R-beklentisi-YOK ──
#[test]
fn consecutive_blocks_variable_r_no_exact_expectation() {
    let (mut tx, mut rx) = common::pair(0x0800_0001);
    let expect = app(8);
    // A: r=0 kayıpsız (source-only-tamamlama).
    assert_eq!(
        run_block(&mut tx, &mut rx, 1, 8, 0, 0, &[], None, false).unwrap(),
        expect
    );
    // B: r=1, 1-kayıp → recoverable.
    assert_eq!(
        run_block(&mut tx, &mut rx, 2, 8, 100, 1, &[3], None, false).unwrap(),
        expect
    );
    // C: r=Rmax=4, 2-kayıp → recoverable.
    assert_eq!(
        run_block(&mut tx, &mut rx, 3, 8, 200, 4, &[1, 5], None, false).unwrap(),
        expect
    );
    // D: 0<r<Rmax (r=2), kayıpsız.
    assert_eq!(
        run_block(&mut tx, &mut rx, 4, 8, 300, 2, &[], None, false).unwrap(),
        expect
    );
    // Ardışık-r-değişimi decoder-state'ini-bozmadı (aynı-rx; her-blok-bağımsız-decode).
}

// ── §6.2: senaryolar ──
#[test]
fn scenarios_per_r() {
    let (mut tx, mut rx) = common::pair(0x0800_0002);
    let expect = app(8);
    // r=1: 2-kayıp → UNRECOVERABLE (fail-closed; typed).
    assert!(matches!(
        run_block(&mut tx, &mut rx, 1, 8, 0, 1, &[2, 6], None, false),
        Err(EngineError::FecDecode(_))
    ));
    // r=4 ama yalnız-2-repair-ULAŞTI (eksik-repair) + 2-kayıp → yine-recoverable (dinamik-kabul).
    assert_eq!(
        run_block(&mut tx, &mut rx, 2, 8, 100, 4, &[0, 7], Some(2), false).unwrap(),
        expect
    );
    // Out-of-order: repair-ÖNCE source-SONRA → decode-doğru.
    assert_eq!(
        run_block(&mut tx, &mut rx, 3, 8, 200, 4, &[4], None, true).unwrap(),
        expect
    );
    // Duplicate-repair: aynı-repair-datagram-iki-kez → İKİNCİ-open PnReject (wire-replay);
    // logical-dup-admission zaten DuplicateEsi-fail-closed (pipeline.rs-kanıtı) — burada wire-katmanı-reddi.
    let (sealed, k_actual) =
        common::encode_seal_block_with_r(&mut tx, &by_id(PID).unwrap(), &expect, 4, 0x9999, 300, 2);
    let rep = sealed.iter().find(|s| s.is_repair).unwrap();
    let mut opened = Vec::new();
    for s in &sealed {
        opened.push(rx.open(&s.datagram).expect("authenticated"));
    }
    assert!(
        rx.open(&rep.datagram).is_err(),
        "duplicate-repair wire-replay-reddi"
    );
    assert_eq!(admit_and_decode(opened, PID, k_actual).unwrap(), expect);
}

// ── §6.3: r=0-semantiği — repair-üretilmez; source-only; leak-yok; sonraki-r>0-etkilenmez ──
#[test]
fn r0_semantics_no_leak() {
    let (mut tx, mut rx) = common::pair(0x0800_0003);
    let (sealed, k_actual) =
        common::encode_seal_block_with_r(&mut tx, &by_id(PID).unwrap(), &app(8), 1, 0x1111, 0, 0);
    assert!(
        sealed.iter().all(|s| !s.is_repair),
        "r=0: sender-repair-ÜRETMEZ"
    );
    assert_eq!(sealed.len(), 8);
    let opened: Vec<_> = sealed
        .iter()
        .map(|s| rx.open(&s.datagram).unwrap())
        .collect();
    assert_eq!(
        admit_and_decode(opened, PID, k_actual).unwrap(),
        app(8),
        "source-only-tamamlama"
    );
    // r=0 + 1-kayıp → unrecoverable (fail-closed; stuck-state-değil-typed-hata).
    assert!(matches!(
        run_block(&mut tx, &mut rx, 2, 8, 100, 0, &[3], None, false),
        Err(EngineError::FecDecode(_) | EngineError::Reassembly(_))
    ));
    // Sonraki-blok r=4 NORMAL (önceki-r=0/hata-bloğundan-etkilenme-YOK; leak-yok — admit_and_decode
    // stateless-fn: blok-state'i-çağrı-yereli, kalıcı-decoder-state-yapısal-olarak-yok).
    assert_eq!(
        run_block(&mut tx, &mut rx, 3, 8, 200, 4, &[1, 2], None, false).unwrap(),
        app(8)
    );
}

// ── §6.4: partial-block (k_actual<K) × variable-r ──
#[test]
fn partial_block_variable_r() {
    let (mut tx, mut rx) = common::pair(0x0800_0004);
    let expect = app(5); // k_actual=5 < K=8 (final-partial-kalıbı; known_zero=3)
                         // r=2, 1-kayıp → recoverable.
    assert_eq!(
        run_block(&mut tx, &mut rx, 1, 5, 0, 2, &[2], None, false).unwrap(),
        expect
    );
    // r=0 partial, kayıpsız → source-only.
    assert_eq!(
        run_block(&mut tx, &mut rx, 2, 5, 100, 0, &[], None, false).unwrap(),
        expect
    );
}

// ── §6.2-ek: interleaved-block-IDs (A/B-shard'ları-karışık-sırada; blok-başına-decode) ──
#[test]
fn interleaved_blocks_decode_independently() {
    let (mut tx, mut rx) = common::pair(0x0800_0005);
    let p = by_id(PID).unwrap();
    let (sa, ka) = common::encode_seal_block_with_r(&mut tx, &p, &app(8), 10, 0xAAAA, 0, 2);
    let (sb, kb) = common::encode_seal_block_with_r(&mut tx, &p, &app(8), 11, 0xBBBB, 100, 1);
    // Karışık-open-sırası (A0,B0,A1,B1,...): tek-rx; blok-ayrımı block_id-ile.
    let mut oa = Vec::new();
    let mut ob = Vec::new();
    let (mut ia, mut ib) = (sa.iter(), sb.iter());
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => break,
            (x, y) => {
                if let Some(s) = x {
                    oa.push(rx.open(&s.datagram).unwrap());
                }
                if let Some(s) = y {
                    ob.push(rx.open(&s.datagram).unwrap());
                }
            }
        }
    }
    assert_eq!(admit_and_decode(oa, PID, ka).unwrap(), app(8));
    assert_eq!(admit_and_decode(ob, PID, kb).unwrap(), app(8));
}
