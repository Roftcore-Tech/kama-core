//! **mock-fixture** — sahte modül consumer'ı yanıltmıyor mu.
//!
//! Google SWE ch.13'ün kuralı: *"A fake must have its own tests to ensure that it conforms to the
//! API of its corresponding real implementation."* Sahtenin sözleşmeye UYDUĞU conformance-core'da
//! ölçülür; burada ölçülen ayrı bir şeydir: sahtenin **yapamadığı şeyi yapıyormuş gibi
//! göstermemesi** ve kendi iddialarına sadık kalması. Bu testlerin gerçek hedefte karşılığı
//! yoktur (gerçek transport'un `MOCK_MAX_INBOX`'ı yoktur), o yüzden ayrı ikilide dururlar.
//!
//! Bugünkü sahte KM-SEAM-AT-001 §7'nin tam `MockTransport`'u DEĞİLDİR (`GAP-SEAM-MOCK-7`:
//! kayıp/sırasız/duplicate enjeksiyonu yok). Dolayısıyla buradaki sadakat iddiaları da dar:
//! determinizm, teslim uydurmama, sınırlılık, beyan dürüstlüğü.

#[path = "../common/seam/harness.rs"]
mod harness;

use harness::api::{
    AbandonReason, CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PeerRef,
    PreparedUnit, SessionEvent, SessionHandle, TransportError,
};
use harness::target::{
    EchoMockTransport, MockTarget, SeamTarget, TargetKind, MOCK_MAX_INBOX, MOCK_MAX_SESSIONS,
    STUB_MAX_PLAINTEXT_UNIT_BYTES, STUB_PREFERRED_PLAINTEXT_UNIT_BYTES,
};

fn unit(bytes: &[u8]) -> PreparedUnit {
    PreparedUnit {
        bytes: bytes.to_vec(),
        compression_policy: CompressionPolicy::Never,
    }
}

/// `connect` → `Pending` → `Ready` olayını sürerek aktif handle üretir.
fn ready(t: &mut EchoMockTransport, peer: PeerRef) -> SessionHandle {
    match t.connect(peer, None).expect("connect reddedilmemeli") {
        ConnectResult::Ready(h) => h,
        ConnectResult::Pending(_) => match t.poll_event() {
            Some(SessionEvent::Ready(h, p)) => {
                assert_eq!(p, peer, "Ready olayı başka peer taşıdı");
                h
            }
            other => panic!("Ready olayı bekleniyordu, gelen: {other:?}"),
        },
    }
}

/// Sahte, ailesini doğru beyan eder — "mock" diye koşan şey gerçekte gerçek adapter olamaz.
#[test]
fn f1_mock_declares_itself_as_mock() {
    assert_eq!(MockTarget::KIND, TargetKind::Mock);
    assert_eq!(MockTarget::ID, "mock");
}

/// ADR-17 determinizmi sahte için de geçerli: aynı girdi → aynı çıktı.
///
/// Deterministik olmayan bir sahte, consumer'a var olmayan bir davranış çeşitliliği gösterir ve
/// "bende çalışıyordu" sınıfı hatalar üretir.
#[test]
fn f2_mock_is_deterministic_across_instances() {
    let mut a = MockTarget::build().expect("mock hedefi kurulmalı");
    let mut b = MockTarget::build().expect("mock hedefi kurulmalı");
    let peer = PeerRef::from_raw(1);

    let r_a = a.connect(peer, None);
    let r_b = b.connect(peer, None);
    assert_eq!(
        r_a, r_b,
        "iki taze örnek ayrıştı — sahte gizli durum taşıyor"
    );

    // Aynı peer'a ikinci çağrı yeni bilet üretmez (KARAR-1 idempotent; satır olarak İDDİA
    // EDİLMİYOR — burada ölçülen yalnız sahtenin kendi kendisiyle tutarlılığı).
    let again = a.connect(peer, None);
    assert_eq!(again, r_a, "ikinci connect farklı bilet/ sonuç üretti");
}

/// Sahte, YAPAMADIĞI teslimi uydurmaz.
///
/// Consumer'ı yanıltmanın en pahalı biçimi budur: app, transport'un kuyruğa aldığını sanıp
/// (K8: `Ok(())` = kuyruğa alındı) kendi retry'sini yazmaz ve birim sessizce kaybolur.
#[test]
fn f3_mock_does_not_fabricate_delivery() {
    let mut t = MockTarget::build().expect("mock hedefi kurulmalı");

    // Hiç connect edilmemiş handle.
    assert_eq!(
        t.send(
            SessionHandle::from_raw(7),
            unit(b"kama"),
            DeliveryPolicy::Reliable
        ),
        Err(TransportError::UnknownSession)
    );

    // Pending oturum da aktif DEĞİLDİR: Ready gelmeden send kabul edilmez (§K7).
    let peer = PeerRef::from_raw(9);
    let ConnectResult::Pending(_) = t.connect(peer, None).expect("connect") else {
        panic!("taze peer için Pending bekleniyordu");
    };
    let pending_handle = SessionHandle::from_raw(1);
    assert_eq!(
        t.send(pending_handle, unit(b"erken"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession),
        "Ready gelmeden send kabul edildi"
    );
    assert!(
        t.recv().is_none(),
        "reddedilen send'den sonra teslim uydurdu"
    );
}

/// Sahtenin kapasite sayıları BEYANLI yer tutuculardır; sessizce "gerçek değer" hâline gelemezler.
#[test]
fn f4_mock_capabilities_are_declared_placeholders() {
    let t = MockTarget::build().expect("mock hedefi kurulmalı");
    let caps = t.capabilities();
    assert_eq!(caps.max_plaintext_unit_bytes, STUB_MAX_PLAINTEXT_UNIT_BYTES);
    assert_eq!(
        caps.preferred_plaintext_unit_bytes,
        STUB_PREFERRED_PLAINTEXT_UNIT_BYTES
    );
}

/// Sahte, uygulanmamış §6 satırını uygulanmış gibi ilan etmez; beyan iyi biçimlidir.
///
/// Kimliklerin `seam-coverage.json` ile eşleşmesi runner'ın çapraz kontrolüdür; buradaki sınama
/// o zincirin Rust ucunu (biçim + tekrarsızlık) yerinde tutar.
#[test]
fn f5_mock_contract_row_claims_are_well_formed() {
    let rows = harness::contract::CONTRACT_ROWS;
    for (i, id) in rows.iter().enumerate() {
        assert!(
            id.starts_with("H-"),
            "§6 satır kimliği 'H-' ile başlamalı: {id}"
        );
        assert!(
            !rows[..i].contains(id),
            "§6 satır kimliği iki kez beyan edildi: {id}"
        );
    }
}

/// Sahte SINIRLIDIR (SR-AUDIT-001): kuyruklar sınırsız büyümez, dolunca typed `Backpressure`.
///
/// Sınırsız bir sahte, consumer'a gerçek transport'ta bulunmayan bir "sonsuz kuyruk" davranışı
/// gösterir; app backpressure yolunu hiç yazmaz ve gerçek hedefte ilk kez orada patlar.
#[test]
fn f6_mock_queues_are_bounded() {
    let mut t = MockTarget::build().expect("mock hedefi kurulmalı");
    let h = ready(&mut t, PeerRef::from_raw(3));

    for i in 0..MOCK_MAX_INBOX {
        assert_eq!(
            t.send(h, unit(b"x"), DeliveryPolicy::Reliable),
            Ok(()),
            "kuyruk dolmadan {i}. send reddedildi"
        );
    }
    assert_eq!(
        t.send(h, unit(b"x"), DeliveryPolicy::Reliable),
        Err(TransportError::Backpressure),
        "kuyruk tavanında sessizce büyüdü"
    );

    // Oturum tavanı da sınırlı.
    let mut t2 = MockTarget::build().expect("mock hedefi kurulmalı");
    for i in 0..MOCK_MAX_SESSIONS {
        assert!(
            t2.connect(PeerRef::from_raw(100 + i as u64), None).is_ok(),
            "tavan altında {i}. connect reddedildi"
        );
    }
    assert_eq!(
        t2.connect(PeerRef::from_raw(999), None),
        Err(TransportError::Backpressure),
        "oturum tavanında sessizce büyüdü"
    );
}

/// Sahte kayıp/sıra bozma/duplicate ENJEKTE ETMEZ ve bunu iddia da etmez.
///
/// `GAP-SEAM-MOCK-7` kapanana kadar sahtenin davranışı bu: N birim girdi → N birim, gönderim
/// sırasında, tam olarak bir kez. Sessizce kayıp veren bir sahte, H-APP-RETRY-0'ın ölçmek
/// istediği farkı ölçülemez hâle getirirdi.
#[test]
fn f7_mock_injects_no_loss_reorder_or_duplication() {
    let mut t = MockTarget::build().expect("mock hedefi kurulmalı");
    let h = ready(&mut t, PeerRef::from_raw(4));

    let payloads: Vec<Vec<u8>> = (0u8..8).map(|i| vec![i; (i as usize) + 1]).collect();
    for p in &payloads {
        t.send(h, unit(p), DeliveryPolicy::Reliable).expect("send");
    }
    for (i, p) in payloads.iter().enumerate() {
        let d = t.recv().unwrap_or_else(|| panic!("{i}. birim kayıp"));
        assert_eq!(&d.unit, p, "{i}. birim sırası/içeriği bozuldu");
        assert_eq!(d.session, h);
    }
    assert!(t.recv().is_none(), "fazladan (duplicate) teslim üretti");
}

/// DR-CONNECT-1 H9 — sahtenin `cancel` yolu sözleşmenin dediğini yapar ve fazlasını uydurmaz.
///
/// Bekleyen niyet iptal edilince: tek olay `ConnectAbandoned(ticket, Cancelled)`; `Ready` ASLA
/// gelmez; `early_unit` hiçbir yola girmez; aynı peer'a yeniden `connect` YENİ bilet verir.
/// (H-CANCEL'in "niyet kendiliğinden sonlanmaz" kolu sanal saat ister — `GAP-SEAM-MOCK-7`.)
#[test]
fn f8_mock_cancel_abandons_pending_without_inventing_ready() {
    let mut t = MockTarget::build().expect("mock hedefi kurulmalı");
    let peer = PeerRef::from_raw(5);

    let ticket = match t.connect(peer, Some(unit(b"early"))).expect("connect") {
        ConnectResult::Pending(ticket) => ticket,
        ConnectResult::Ready(_) => panic!("taze peer için Pending bekleniyordu"),
    };
    t.cancel(ticket);

    assert_eq!(
        t.poll_event(),
        Some(SessionEvent::ConnectAbandoned(
            ticket,
            AbandonReason::Cancelled
        )),
        "cancel tek olay üretmeli: ConnectAbandoned(ticket, Cancelled)"
    );
    assert!(
        t.poll_event().is_none(),
        "iptal edilen niyet için Ready geldi"
    );
    assert!(
        t.recv().is_none(),
        "iptal edilen niyetin early_unit'i teslim edildi"
    );

    // İkinci cancel etkisizdir (bilet geçersizleşti).
    t.cancel(ticket);
    assert!(
        t.poll_event().is_none(),
        "geçersiz bilete cancel olay üretti"
    );

    match t.connect(peer, None).expect("yeniden connect") {
        ConnectResult::Pending(again) => {
            assert_ne!(again, ticket, "iptal sonrası aynı bilet geri verildi");
        }
        ConnectResult::Ready(_) => panic!("iptal sonrası oturum hâlâ aktif görünüyor"),
    }
}
