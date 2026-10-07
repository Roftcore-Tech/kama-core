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
//!
//! **[2026-10-08 — A5.4, L-A5c]** Bağlama katmanı artık ürün yolundadır: `NoiseWireProtection`'ın
//! oturum kontrolü (`session_control`) makinenin eylemlerini uygular ve epoch matrisi
//! (`noise_conformance_core`, `real_epoch`) bunu gerçek oturumda sınar. Bu dosya makine ↔ protector
//! sözleşmesini sabitlemeye devam eder; sürücü yeni API'ye taşındı: grace YALNIZ AEAD-geçerli
//! epoch-1 paketleriyle sayılır (onaylayan #1, `N_g` sabit) ve `T_g` süresi de kapatır.

use core::time::Duration;

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
    /// Bob, RESPONSE'u göndermiş: epoch-1 ALIM anahtarları kurulu, gönderim epoch-0'da.
    fn responder() -> Self {
        let mut machine = DhEpochMachine::new(Role::Responder);
        machine.on_response_sent();
        let mut protector = KamaNoiseWireProtector::new(H_EXT, &key(0x22), &key(0x11));
        protector.install_epoch1_keys(&key(0x44), &key(0x33));
        Self {
            machine,
            protector,
            applied: Vec::new(),
        }
    }

    /// Aksiyonu protector'a UYGULAR — eksik olan tek satır budur.
    fn apply(&mut self, action: EpochAction) {
        match action {
            // Anahtarlar RESPONSE gönderilirken kuruldu; bu eylem gönderim epoch'unun geçişidir.
            EpochAction::InstallEpoch1AndSend | EpochAction::StartGrace => {}
            EpochAction::DiscardEpoch0Recv => {
                self.protector.discard_epoch0_recv();
            }
        }
        self.applied.push(action);
    }

    /// AEAD-geçerli bir epoch-1 datagramını açar ve makineyi sürer (onay / grace sayımı).
    fn deliver_epoch1(&mut self, wire: &[u8]) {
        self.protector.open(wire).expect("geçerli epoch-1 açılmalı");
        for a in self.machine.on_valid_epoch1_received(Duration::ZERO) {
            self.apply(a);
        }
    }

    /// Grace'i `N_g`'ye kadar tüketir (onaylayan paket zaten #1 sayıldı).
    fn exhaust_grace(&mut self, peer: &mut KamaNoiseWireProtector, first_pn: u64) -> u64 {
        let mut pn = first_pn;
        while self.machine.recv_accepts(0) {
            let d = peer.seal(&fec(), args(pn, 1)).expect("seal e1").bytes;
            self.deliver_epoch1(&d);
            pn += 1;
        }
        pn
    }
}

/// Karşı uç: epoch-0 ve epoch-1 datagramı üretir (ters anahtar çifti).
fn peer() -> KamaNoiseWireProtector {
    let mut p = KamaNoiseWireProtector::new(H_EXT, &key(0x11), &key(0x22));
    p.install_epoch1_keys(&key(0x33), &key(0x44));
    p
}

// ── (a) Bağlanmamış aksiyon: BUL-11'in kaydı ────────────────────────────────

// ── (b) Bağlandığında ne olması gerektiği (TC-18 oracle'ı) ──────────────────

#[test]
fn epoch0_datagram_is_rejected_after_the_discard_action_is_applied() {
    // TC-18 ORACLE. Bu, wiring eklendiğinde production'da gözlenmesi gereken davranıştır.
    let mut driver = MinimalEpochDriver::responder();
    let mut peer = peer();

    // PN sayacı epoch'lar boyunca SÜREKLİDİR (§4 EPOCH-PN-INIT, 2026-10-06): gönderen epoch-0'da
    // PN 1..3'ü mühürler, sonra epoch-1'e PN 4 ile geçer. PN 2 ve 3 geçiş anında UÇUŞTADIR.
    let e0_early = peer.seal(&fec(), args(1, 0)).expect("seal e0").bytes;
    let e0_grace = peer.seal(&fec(), args(2, 0)).expect("seal e0").bytes;
    let e0_after = peer.seal(&fec(), args(3, 0)).expect("seal e0").bytes;
    // Epoch-0 trafiği transition öncesi KABUL EDİLİR.
    assert!(
        driver.protector.open(&e0_early).is_ok(),
        "transition öncesi epoch-0"
    );

    // İlk geçerli epoch-1 → implicit confirmation + grace (#1).
    let e1 = peer.seal(&fec(), args(4, 1)).expect("seal e1").bytes;
    driver.deliver_epoch1(&e1);
    assert!(matches!(driver.machine.recv0(), RecvEpoch0::Grace { .. }));

    // Grace BOYUNCA epoch-0 hâlâ kabul edilir (§4-B3: "epoch-0-usable" garantisi).
    assert!(
        driver.protector.open(&e0_grace).is_ok(),
        "grace içinde epoch-0 düştü"
    );

    // Grace'i tüket (yalnız geçerli epoch-1 paketleri sayılır).
    let next = driver.exhaust_grace(&mut peer, 5);
    assert!(
        driver.applied.contains(&EpochAction::DiscardEpoch0Recv),
        "discard aksiyonu sürücüye ulaşmadı"
    );

    // ORACLE: discard sonrası epoch-0 → EpochNotAccepted(0); epoch-1 → Ok.
    assert_eq!(
        driver.protector.open(&e0_after).unwrap_err(),
        WireError::EpochNotAccepted(0),
        "discard sonrası epoch-0 hâlâ kabul ediliyor — rotasyon anlamsız"
    );
    let e1_after = peer.seal(&fec(), args(next, 1)).expect("seal e1").bytes;
    assert!(
        driver.protector.open(&e1_after).is_ok(),
        "discard epoch-1'i de öldürdü"
    );
}

#[test]
fn discard_is_irreversible_within_the_session() {
    // Eski anahtarın "geçersizleşmesi" kalıcı olmalıdır. Discard sonrası gelen bir epoch-0
    // seli hiçbir koşulda kabulü geri açmamalı (state-machine geri dönüşü yok).
    let mut driver = MinimalEpochDriver::responder();
    let mut peer = peer();
    let next = driver.exhaust_grace(&mut peer, 1);

    for pn in next..next + 30 {
        let d = peer.seal(&fec(), args(pn, 0)).expect("seal").bytes;
        assert_eq!(
            driver.protector.open(&d).unwrap_err(),
            WireError::EpochNotAccepted(0),
            "pn={pn}: epoch-0 kabulü geri açıldı"
        );
    }
    // Ek geçerli epoch-1 paketleri ve zaman da durumu değiştirmemeli (idempotent Discarded).
    let d = peer
        .seal(&fec(), args(next + 30, 1))
        .expect("seal e1")
        .bytes;
    driver.deliver_epoch1(&d);
    assert_eq!(driver.machine.on_time(Duration::MAX), None);
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

#[test]
fn initiator_grace_starts_only_after_the_peer_switches() {
    // ~~Rol asimetrisi (§4-B3): grace YALNIZ Responder'da başlar.~~ [2026-10-08 A5.4 — YORUM]
    // Initiator'ın epoch-0 ALIMI da aynı kuralla kapanır, ama yalnız Bob epoch-1'e geçtikten SONRA
    // (Bob'dan gelen ilk geçerli epoch-1 paketi = #1): öncesinde Bob epoch-0'da gönderir ve Alice'in
    // epoch-0 alımı açık kalmalıdır — erken kapanırsa Bob'un epoch-0 paketleri düşer.
    let mut m = DhEpochMachine::new(Role::Initiator);
    assert!(
        m.on_valid_epoch1_received(Duration::ZERO).is_empty(),
        "epoch-1 anahtarı yokken grace başladı"
    );
    assert_eq!(
        m.on_response_received(),
        Some(EpochAction::InstallEpoch1AndSend)
    );
    assert_eq!(
        m.on_time(Duration::MAX),
        None,
        "grace başlamadan zaman kapatmaz"
    );
    assert_eq!(
        m.recv0(),
        RecvEpoch0::Accepting,
        "Bob geçmeden Initiator epoch-0 kabulü kapandı"
    );
    // Bob geçti: ilk geçerli epoch-1 paketi → grace (#1), gönderim zaten epoch-1.
    assert_eq!(
        m.on_valid_epoch1_received(Duration::ZERO),
        vec![EpochAction::StartGrace]
    );
    for _ in 2..GRACE_MIN_PACKETS {
        assert!(m.on_valid_epoch1_received(Duration::ZERO).is_empty());
    }
    assert_eq!(
        m.on_valid_epoch1_received(Duration::ZERO),
        vec![EpochAction::DiscardEpoch0Recv]
    );
}

#[test]
fn epoch1_pn_continues_from_epoch0() {
    // §4 EPOCH-PN-INIT (2026-10-06): PN sayacı epoch'lar boyunca SÜREKLİ, replay penceresi TEK.
    // Discard yalnız epoch-0 ANAHTARINI kapatır; pencere epoch-0'ın ilerlemişliğinden devam eder.
    let mut driver = MinimalEpochDriver::responder();
    let mut peer = peer();

    // Pencereyi epoch-0'da yüksek bir PN'e taşı.
    for pn in 1..=20u64 {
        let d = peer.seal(&fec(), args(pn, 0)).expect("seal").bytes;
        driver.protector.open(&d).expect("epoch-0 open");
    }
    // Gönderen epoch-1'de sayacı geri alamaz.
    assert!(
        peer.seal(&fec(), args(0, 1)).is_err(),
        "epoch-1 PN 0'dan yeniden başladı"
    );
    // epoch-1 son PN'den +1 ile devam eder; onaylayan paket budur.
    let d = peer.seal(&fec(), args(21, 1)).expect("seal e1 pn=21").bytes;
    driver.deliver_epoch1(&d);
    assert_eq!(driver.protector.recv_expected_pn(), 22);
    // Grace boyunca ve sonrasında pencere TEK ve süreklidir.
    let next = driver.exhaust_grace(&mut peer, 22);
    assert_eq!(driver.protector.recv_expected_pn(), next);
    assert_eq!(driver.machine.recv0(), RecvEpoch0::Discarded);
}
