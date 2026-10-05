//! H-RETRY — yeniden gönderim Transport'un önbelleğindeki zarfın byte-özdeş kopyasıdır; Crypto
//! yeniden ÜRETMEZ (INV-INIT-KEY-1, ccr INV-RESP-KEY-1, noise-wire §0.7-B).
//!
//! * Alice: `Initiate` çıktısı önbellekten byte-özdeş yeniden gönderilir ve Bob'da idempotenttir;
//!   ikinci `Initiate` farklı byte üretir (yeni `init_counter` / yeni geçici anahtar).
//! * Bob: aynı INIT tekrar gelirse mevcut oturuma eşlenir ve ÖNBELLEKTEKİ RESPONSE byte-özdeş
//!   döner; yeni oturum, yeni yanıtlayan geçici anahtarı YOK.

use kama_contracts::s3::{SessionCommand, SessionControl, SessionEvent, SessionResult};

use crate::mock_transport::MockTransport;
use crate::s3_support::{initiate, next_command as deliver, PEER};
use crate::target::S3Target;

/// Alice yarısı.
pub fn alice_retransmits_cached_init_and_new_initiate_differs<T: S3Target>() {
    let mut a = T::build().expect("hedef kayıtlı olmalı");
    let mut b = T::build().expect("hedef kayıtlı olmalı");
    let mut net = MockTransport::new();

    let (ty, init) = initiate::<T>(&mut a, PEER);
    let id = net.submit(ty, &init).expect("INIT yola çıkar");
    let first = b.command(deliver(&mut net));
    assert!(matches!(first, SessionResult::EstablishedAndReply { .. }));
    let after_first = T::state_fingerprint(&b);

    // Yeniden gönderim: Crypto çağrılmadan, önbellekten. Bob'da idempotent.
    net.retry(id).expect("önbellekten yeniden gönderim");
    let cmd = deliver(&mut net);
    assert_eq!(
        cmd,
        SessionCommand::AcceptInit { wire: init.clone() },
        "yeniden gönderim byte-özdeş değil"
    );
    assert_eq!(
        b.command(cmd),
        first,
        "aynı INIT ikinci kez farklı sonuç verdi"
    );
    assert_eq!(
        T::state_fingerprint(&b),
        after_first,
        "duplicate INIT state değiştirdi"
    );

    // İkinci Initiate: yeni deneme, farklı byte.
    let (_, second) = initiate::<T>(&mut a, PEER);
    assert_ne!(second, init, "ikinci Initiate aynı zarfı üretti");
}

/// Bob yarısı.
pub fn bob_maps_duplicate_init_to_same_session_and_cached_response<T: S3Target>() {
    let mut a = T::build().expect("hedef kayıtlı olmalı");
    let mut b = T::build().expect("hedef kayıtlı olmalı");
    let (_, init) = initiate::<T>(&mut a, PEER);
    let accept = |b: &mut T::Crypto, wire: &[u8]| {
        b.command(SessionCommand::AcceptInit {
            wire: wire.to_vec(),
        })
    };

    let SessionResult::EstablishedAndReply {
        session,
        bytes: ssr1,
        ..
    } = accept(&mut b, &init)
    else {
        panic!("AcceptInit oturum + RESPONSE üretmeli");
    };
    assert_eq!(b.poll_event(), Some(SessionEvent::Ready(session)));
    assert_eq!(b.poll_event(), None);
    let after_first = T::state_fingerprint(&b);

    for _ in 0..3 {
        let SessionResult::EstablishedAndReply {
            session: again,
            bytes: ssr1_again,
            ..
        } = accept(&mut b, &init)
        else {
            panic!("duplicate INIT de oturum + RESPONSE döndürmeli");
        };
        assert_eq!(again, session, "duplicate INIT ikinci bir oturum doğurdu");
        assert_eq!(
            ssr1_again, ssr1,
            "RESPONSE önbellekten gelmedi (byte farkı)"
        );
        assert_eq!(b.poll_event(), None, "duplicate INIT olay üretti");
        assert_eq!(T::state_fingerprint(&b), after_first);
    }

    // Karşılaştırmanın boş olmadığının kanıtı: FARKLI bir INIT'in RESPONSE'u farklıdır.
    let (_, other_init) = initiate::<T>(&mut a, PEER);
    let SessionResult::EstablishedAndReply {
        bytes: other_ssr1, ..
    } = accept(&mut b, &other_init)
    else {
        panic!("yeni INIT de RESPONSE üretmeli");
    };
    assert_ne!(other_ssr1, ssr1, "farklı INIT'e aynı RESPONSE döndü");

    // Alice önbellekli RESPONSE'la oturumu tamamlar.
    assert_eq!(
        a.command(SessionCommand::AcceptResponse { wire: ssr1 }),
        SessionResult::Established { session }
    );
}
