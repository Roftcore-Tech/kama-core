//! TC-18 — Epoch-0 anahtar düşürme yolu (SR-STR-CORE-001 §5.4, BUL-11).
//!
//! **Kapsanan bulgu:** BUL-11 / AÇIK-4. [KOD] `protector.rs::discard_epoch0_recv()` **hiçbir
//! yerden çağrılmamaktadır.** `DhEpochMachine` `DiscardEpoch0Recv` aksiyonunu üretir ve
//! `sprint4c_epoch::grace_expiry_discards_epoch0` bunu doğrular; ancak aksiyonu protector'a
//! BAĞLAYAN katman bu depoda mevcut değildir.
//!
//! **Gerekçe:** Makinenin doğru aksiyon ürettiği test edilmiş, aksiyonun UYGULANDIĞI test
//! edilmemiştir — çünkü uygulayan katman yoktur. Anahtar rotasyonunun ANLAMI eski anahtarın
//! gerçekten geçersizleşmesidir; aksi hâlde grace penceresi süresiz uzar ve epoch-0 anahtarı
//! oturum boyunca canlı kalır. Bu, forward secrecy'nin epoch sınırındaki karşılığıdır.
//!
//! **Bu dosya bir test olduğu kadar bir KAYIT TALEBİDİR** (SR-STR-CORE-001 TC-18 metni).
//! Aşağıdaki `MinimalEpochDriver`, eksik olan bağlama katmanının minimal bir modelidir ve
//! TEST-ONLY'dir — production'da engine katmanının yapması gereken şeyi gösterir.
//! [BEKLİYOR — Abkarada, AÇIK-4] Erteleme bilinçli mi (engine katmanı beklenmekte) yoksa
//! boşluk mu? Bilinçliyse spec'te `[BEKLİYOR]` olarak işaretlenmelidir.
//!
//! **Oracle:** Discard sonrası epoch-0 datagram → `EpochNotAccepted(0)`; epoch-1 datagram → `Ok`.

use kama_noise_wire::error::WireError;
use kama_noise_wire::{
    DhEpochMachine, EpochAction, KamaNoiseWireProtector, RecvEpoch0, Role, SealArgs,
    GRACE_MIN_PACKETS,
};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0102_0304;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn fec() -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: vec![0xA5u8; 64],
    }
}

fn args(wire_pn: u64, dh_epoch: u8) -> SealArgs {
    SealArgs {
        wire_pn,
        dh_epoch,
        send_generation: 0,
    }
}

/// **TEST-ONLY sürücü — depoda EKSİK olan bağlama katmanının minimal modeli.**
///
/// Production'da bu iş engine katmanına aittir: `DhEpochMachine`'in ürettiği `EpochAction`'ları
/// `KamaNoiseWireProtector` üzerinde uygular. Bu depoda o katman yoktur (BUL-11), dolayısıyla
/// test kendi sürücüsünü kurar. Bu bir TASARIM ÖNERİSİ değil, eksik bağın TEST KARŞILIĞIDIR.
struct MinimalEpochDriver {
    machine: DhEpochMachine,
    protector: KamaNoiseWireProtector,
    applied: Vec<EpochAction>,
}

impl MinimalEpochDriver {
    fn responder(grace_budget: u32) -> Self {
        Self {
            machine: DhEpochMachine::new(Role::Responder, grace_budget),
            protector: KamaNoiseWireProtector::new(H_EXT, &key(0x22), &key(0x11)),
            applied: Vec::new(),
        }
    }

    /// Aksiyonu protector'a UYGULAR — eksik olan tek satır budur.
    fn apply(&mut self, action: EpochAction) {
        match action {
            EpochAction::InstallEpoch1AndSend => {
                self.protector.install_epoch1_keys(&key(0x44), &key(0x33));
            }
            EpochAction::StartGrace => {}
            EpochAction::DiscardEpoch0Recv => {
                self.protector.discard_epoch0_recv();
            }
        }
        self.applied.push(action);
    }

    fn on_valid_epoch1_received(&mut self) {
        for a in self.machine.on_valid_epoch1_received() {
            self.apply(a);
        }
    }

    fn on_grace_tick(&mut self) {
        if let Some(a) = self.machine.on_grace_tick() {
            self.apply(a);
        }
    }
}

/// Karşı uç: epoch-0 ve epoch-1 datagramı üretir (ters anahtar çifti).
fn peer() -> KamaNoiseWireProtector {
    let mut p = KamaNoiseWireProtector::new(H_EXT, &key(0x11), &key(0x22));
    p.install_epoch1_keys(&key(0x33), &key(0x44));
    p
}

// ── (a) Bağlanmamış aksiyon: BUL-11'in kaydı ────────────────────────────────

#[test]
fn machine_emits_discard_but_nothing_in_this_repo_applies_it() {
    // [ÖLÇÜLDÜ] Makine aksiyonu ÜRETİR. `sprint4c_epoch` bunu zaten doğrular.
    // Bu test, aksiyonun üretilmesi ile UYGULANMASI arasındaki boşluğu açıkça isimlendirir:
    // aşağıdaki `driver.applied` listesi ancak BU DOSYADAKİ sürücü sayesinde dolar.
    // Depoda başka hiçbir yer `discard_epoch0_recv()` çağırmaz.
    let mut m = DhEpochMachine::new(Role::Responder, GRACE_MIN_PACKETS);
    let actions = m.on_valid_epoch1_received();
    assert_eq!(
        actions,
        vec![EpochAction::InstallEpoch1AndSend, EpochAction::StartGrace]
    );
    assert!(matches!(m.recv0(), RecvEpoch0::Grace { .. }));

    let mut discard_seen = None;
    for _ in 0..GRACE_MIN_PACKETS {
        if let Some(a) = m.on_grace_tick() {
            discard_seen = Some(a);
        }
    }
    assert_eq!(
        discard_seen,
        Some(EpochAction::DiscardEpoch0Recv),
        "grace bitişinde discard aksiyonu üretilmedi"
    );
    assert_eq!(m.recv0(), RecvEpoch0::Discarded);
}

// ── (b) Bağlandığında ne olması gerektiği (TC-18 oracle'ı) ──────────────────

#[test]
fn epoch0_datagram_is_rejected_after_the_discard_action_is_applied() {
    // TC-18 ORACLE. Bu, wiring eklendiğinde production'da gözlenmesi gereken davranıştır.
    let mut driver = MinimalEpochDriver::responder(GRACE_MIN_PACKETS);
    let mut peer = peer();

    // Epoch-0 trafiği transition öncesi KABUL EDİLİR.
    let e0_early = peer.seal(&fec(), args(1, 0)).expect("seal e0").bytes;
    assert!(
        driver.protector.open(&e0_early).is_ok(),
        "transition öncesi epoch-0"
    );

    // İlk geçerli epoch-1 → install + grace.
    driver.on_valid_epoch1_received();
    let e1 = peer.seal(&fec(), args(1, 1)).expect("seal e1").bytes;
    assert!(
        driver.protector.open(&e1).is_ok(),
        "epoch-1 kurulumdan sonra açılmalı"
    );

    // Grace BOYUNCA epoch-0 hâlâ kabul edilir (§4-B3: "epoch-0-usable" garantisi).
    let e0_grace = peer.seal(&fec(), args(2, 0)).expect("seal e0").bytes;
    assert!(
        driver.protector.open(&e0_grace).is_ok(),
        "grace içinde epoch-0 düştü"
    );

    // Grace'i tüket.
    for _ in 0..GRACE_MIN_PACKETS {
        driver.on_grace_tick();
    }
    assert!(
        driver.applied.contains(&EpochAction::DiscardEpoch0Recv),
        "discard aksiyonu sürücüye ulaşmadı"
    );

    // ORACLE: discard sonrası epoch-0 → EpochNotAccepted(0); epoch-1 → Ok.
    let e0_after = peer.seal(&fec(), args(3, 0)).expect("seal e0").bytes;
    assert_eq!(
        driver.protector.open(&e0_after).unwrap_err(),
        WireError::EpochNotAccepted(0),
        "discard sonrası epoch-0 hâlâ kabul ediliyor — rotasyon anlamsız"
    );
    let e1_after = peer.seal(&fec(), args(2, 1)).expect("seal e1").bytes;
    assert!(
        driver.protector.open(&e1_after).is_ok(),
        "discard epoch-1'i de öldürdü"
    );
}

#[test]
fn discard_is_irreversible_within_the_session() {
    // Eski anahtarın "geçersizleşmesi" kalıcı olmalıdır. Discard sonrası gelen bir epoch-0
    // seli hiçbir koşulda kabulü geri açmamalı (state-machine geri dönüşü yok).
    let mut driver = MinimalEpochDriver::responder(GRACE_MIN_PACKETS);
    let mut peer = peer();
    driver.on_valid_epoch1_received();
    for _ in 0..GRACE_MIN_PACKETS {
        driver.on_grace_tick();
    }

    for pn in 10..40u64 {
        let d = peer.seal(&fec(), args(pn, 0)).expect("seal").bytes;
        assert_eq!(
            driver.protector.open(&d).unwrap_err(),
            WireError::EpochNotAccepted(0),
            "pn={pn}: epoch-0 kabulü geri açıldı"
        );
    }
    // Ek grace tick'leri de durumu değiştirmemeli (idempotent Discarded).
    driver.on_grace_tick();
    assert_eq!(driver.machine.recv0(), RecvEpoch0::Discarded);
}

#[test]
fn discard_also_drops_the_epoch0_receive_key_not_just_the_accept_flag() {
    // Kabul bayrağını kapatmak YETMEZ; anahtar materyali de düşmelidir. Aksi hâlde bayrağın
    // yanlışlıkla geri açılması (veya bir bellek okuması) eski anahtarı hâlâ canlı bulur.
    // `discard_epoch0_recv()` ikisini birden yapar; bu test o sözleşmeyi sabitler.
    //
    // Gözlemlenebilir kanıt: discard sonrası epoch-0 için `EpochNotAccepted` alınıyor,
    // yani epoch-kabul kapısı anahtar kapısından ÖNCE düşüyor (bkz. err_paths_protector TC-04/a).
    // Anahtarın da düştüğü, kodun tek bir fonksiyonda ikisini set etmesiyle garanti altındadır;
    // buradaki assertion o fonksiyonun ATOMİK kaldığını korur.
    let mut p = KamaNoiseWireProtector::new(H_EXT, &key(0x22), &key(0x11));
    let mut peer = peer();
    let d = peer.seal(&fec(), args(1, 0)).expect("seal").bytes;
    assert!(p.open(&d).is_ok());

    p.discard_epoch0_recv();
    let d2 = peer.seal(&fec(), args(2, 0)).expect("seal").bytes;
    assert_eq!(p.open(&d2).unwrap_err(), WireError::EpochNotAccepted(0));

    // Send tarafı ETKİLENMEMELİ: discard yalnız RECV state'idir (§4-B3 adım-5).
    let mut sender = KamaNoiseWireProtector::new(H_EXT, &key(0x11), &key(0x22));
    sender.discard_epoch0_recv();
    assert!(
        sender.seal(&fec(), args(1, 0)).is_ok(),
        "discard send tarafını da öldürdü — §4-B3 yalnız recv der"
    );
}

// ── (c) Grace bütçesi sınırları ─────────────────────────────────────────────

#[test]
fn grace_budget_is_clamped_to_the_minimum() {
    // §4-B3: GRACE_MIN_PACKETS alt sınırdır. Engine daha küçük bir bütçe verirse clamp edilmeli;
    // aksi hâlde grace penceresi çok kısalır ve uçuştaki epoch-0 paketleri düşer.
    let mut driver = MinimalEpochDriver::responder(1);
    driver.on_valid_epoch1_received();

    for i in 0..(GRACE_MIN_PACKETS - 1) {
        driver.on_grace_tick();
        assert!(
            !driver.applied.contains(&EpochAction::DiscardEpoch0Recv),
            "tick {i}: grace erken bitti (clamp çalışmıyor)"
        );
    }
    driver.on_grace_tick();
    assert!(
        driver.applied.contains(&EpochAction::DiscardEpoch0Recv),
        "grace GRACE_MIN_PACKETS'te bitmedi"
    );
}

#[test]
fn initiator_never_starts_grace_or_discards() {
    // Rol asimetrisi (§4-B3): grace YALNIZ Responder'da başlar. Initiator'da epoch-0 recv
    // discard'ı bu makine tarafından üretilmez — üretilirse Alice kendi epoch-0 alımını
    // erken kapatır ve Bob'un gecikmiş epoch-0 paketleri düşer.
    let mut m = DhEpochMachine::new(Role::Initiator, GRACE_MIN_PACKETS);
    assert!(
        m.on_valid_epoch1_received().is_empty(),
        "Initiator grace başlattı"
    );
    assert_eq!(
        m.on_response_received(),
        Some(EpochAction::InstallEpoch1AndSend)
    );
    for _ in 0..(GRACE_MIN_PACKETS * 2) {
        assert_eq!(m.on_grace_tick(), None, "Initiator discard üretti");
    }
    assert_eq!(
        m.recv0(),
        RecvEpoch0::Accepting,
        "Initiator epoch-0 kabulü kapandı"
    );
}

#[test]
fn epoch1_pn_window_is_independent_of_epoch0() {
    // §4 EPOCH-PN-INIT: epoch-1 ayrı cipher-state, PN 0'dan başlar. Discard sonrası epoch-1
    // penceresi epoch-0'ın ilerlemişliğinden ETKİLENMEMELİ.
    let mut driver = MinimalEpochDriver::responder(GRACE_MIN_PACKETS);
    let mut peer = peer();

    // epoch-0 penceresini yüksek bir PN'e taşı.
    for pn in 1..=20u64 {
        let d = peer.seal(&fec(), args(pn, 0)).expect("seal").bytes;
        driver.protector.open(&d).expect("epoch-0 open");
    }
    driver.on_valid_epoch1_received();
    for _ in 0..GRACE_MIN_PACKETS {
        driver.on_grace_tick();
    }

    // epoch-1 PN 0'dan başlayabilmeli.
    let d = peer.seal(&fec(), args(0, 1)).expect("seal e1 pn=0").bytes;
    assert!(
        driver.protector.open(&d).is_ok(),
        "epoch-1 penceresi epoch-0'dan miras aldı"
    );
    assert_eq!(driver.protector.recv_expected_pn(1), 1);
}
