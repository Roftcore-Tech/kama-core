//! H-PN-OWN — paket sayacının sahibi Crypto'dur (S3 K2).
//!
//! Transport'un yerel durumu sıfırlansa, boşta emekliye ayrılsa ya da kendi dar numarası sarsa
//! bile seam sayacı geri dönmez ve hiçbir PN tekrarlanmaz. Epoch 0→1 geçişinde de sayaç
//! sıfırlanmaz, +1 ile devam eder (noise-wire §4 EPOCH-PN-INIT, 2026-10-06).

use client_core::seam::PacketProtection;

use crate::mock_transport::PeerSlotSim;
use crate::s3_support::{established, session};
use crate::target::S3Target;

/// Her olay arasında mühürlenen paket sayısı.
const BURST: u64 = 5;

fn seal_burst<T: S3Target>(
    a: &mut T::Crypto,
    ctx: client_core::seam::ProtectionContext,
    slot: &mut PeerSlotSim,
    all: &mut Vec<u64>,
) {
    for _ in 0..BURST {
        let u = a.seal(ctx, b"veri").expect("seal");
        slot.on_sealed(u.meta.full_wire_pn).expect("slot kaydı");
        assert_eq!(
            slot.recorded().last(),
            Some(&u.meta.full_wire_pn),
            "Transport'un kaydettiği PN seam'in verdiğiyle özdeş değil"
        );
        all.push(u.meta.full_wire_pn);
    }
}

/// Ardışık `seal()` kesin +1; Transport-yerel olaylar seam sayacını etkilemez.
pub fn pn_survives_transport_local_events<T: S3Target>() {
    let mut p = established::<T>();
    let ctx = session(p.h);
    let mut slot = PeerSlotSim::new();
    let mut all = Vec::new();

    seal_burst::<T>(&mut p.a, ctx, &mut slot, &mut all);
    slot.reset(); // PeerSlot sıfırlandı
    assert_eq!(slot.tx_seq, 0);
    seal_burst::<T>(&mut p.a, ctx, &mut slot, &mut all);
    slot.retire_idle(); // boşta emeklilik
    seal_burst::<T>(&mut p.a, ctx, &mut slot, &mut all);
    slot.force_tx_seq_to_wrap_edge(); // Transport'un dar numarası sarar
    seal_burst::<T>(&mut p.a, ctx, &mut slot, &mut all);
    assert!(slot.tx_seq < BURST as u16, "yerel numara sarmış olmalı");

    let first = all[0];
    for (i, pn) in all.iter().enumerate() {
        assert_eq!(*pn, first + i as u64, "{i}. mühürde PN kesin +1 değil");
    }
    assert_eq!(all.len() as u64, 4 * BURST);
}

/// Epoch 0→1 geçişinde `meta.full_wire_pn` sıfırlanmaz; +1 ile devam eder.
pub fn pn_continues_across_epoch_transition<T: S3Target>() {
    let mut p = established::<T>();
    let ctx = session(p.h);
    let before = p.a.seal(ctx, b"epoch sifir").expect("seal");
    assert_eq!(before.meta.dh_epoch, 0);

    assert!(
        T::enter_epoch1(&mut p.a, p.h),
        "hedef epoch-1'e geçebilmeli"
    );
    let after = p.a.seal(ctx, b"epoch bir").expect("seal");
    assert_eq!(after.meta.dh_epoch, 1, "geçiş meta'ya yansımalı");
    assert_eq!(
        after.meta.full_wire_pn,
        before.meta.full_wire_pn + 1,
        "sayaç epoch geçişinde sıfırlandı ya da atladı"
    );
}
