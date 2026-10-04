//! **mock-fixture** (S3 / A4.3) — sahte Transport, Crypto'yu yanıltmıyor mu.
//!
//! `MockTransport`'un kendi iddialarına sadakati: yeniden gönderim byte-for-byte, arızalar yalnız
//! enjekte edildiğinde, komut türü yalnız `envelope_type`'tan, kuyruklar sınırlı.

#[path = "common/mock_transport.rs"]
mod mock_transport;

use kama_contracts::s3::SessionCommand;
use mock_transport::{
    EnvelopeId, Fault, MockTransport, MockTransportError, ENVELOPE_INIT, ENVELOPE_INIT_RESPONSE,
    MOCK_MAX_CACHED, MOCK_MAX_IN_FLIGHT,
};

fn wire_of(cmd: SessionCommand) -> Vec<u8> {
    match cmd {
        SessionCommand::AcceptInit { wire } | SessionCommand::AcceptResponse { wire } => wire,
        other => panic!("zarf taşıyan komut bekleniyordu: {other:?}"),
    }
}

/// Arıza yokken: N zarf girdi → N komut, gönderim sırasında, tam bir kez, byte-özdeş.
#[test]
fn f1_no_fault_means_no_loss_reorder_or_duplication() {
    let mut t = MockTransport::new();
    let sent: Vec<Vec<u8>> = (0u8..6).map(|i| vec![i; usize::from(i) + 1]).collect();
    for e in &sent {
        t.submit(ENVELOPE_INIT, e).expect("submit");
    }
    for (i, e) in sent.iter().enumerate() {
        let cmd = t.deliver().expect("teslim").expect("bilinen tür");
        assert_eq!(&wire_of(cmd), e, "{i}. zarf sırası/içeriği bozuldu");
    }
    assert!(t.deliver().is_none(), "fazladan teslim üretti");
    assert_eq!((t.sent, t.dropped), (6, 0));
}

/// K6: komut türü YALNIZ `envelope_type`'tan seçilir; aynı byte'lar farklı türle farklı komut olur.
#[test]
fn f2_command_kind_comes_from_envelope_type_not_content() {
    let mut t = MockTransport::new();
    let bytes = b"SSI1-gibi-gorunen-ama-RESPONSE-olarak-etiketlenen".to_vec();
    t.submit(ENVELOPE_INIT, &bytes).unwrap();
    t.submit(ENVELOPE_INIT_RESPONSE, &bytes).unwrap();
    assert_eq!(
        t.deliver().unwrap().unwrap(),
        SessionCommand::AcceptInit {
            wire: bytes.clone()
        }
    );
    assert_eq!(
        t.deliver().unwrap().unwrap(),
        SessionCommand::AcceptResponse {
            wire: bytes.clone()
        }
    );
    // Tanımsız tür → typed DROP; içerik "INIT'e benziyor" diye kurtarılmaz.
    t.submit(0x7F, &bytes).unwrap();
    assert_eq!(
        t.deliver().unwrap(),
        Err(MockTransportError::UnknownEnvelopeType)
    );
}

/// INV-INIT-KEY-1: yeniden gönderim saklanan zarfın BYTE-FOR-BYTE kopyasıdır; çağıranın elindeki
/// tampon sonradan değişse bile.
#[test]
fn f3_retry_is_byte_identical_to_the_cached_envelope() {
    let mut t = MockTransport::new();
    let mut caller_buf = vec![0xAB; 121];
    let id = t.submit(ENVELOPE_INIT, &caller_buf).unwrap();
    let first = wire_of(t.deliver().unwrap().unwrap());
    caller_buf.fill(0xFF); // çağıran tamponunu yeniden kullandı
    for _ in 0..3 {
        t.retry(id).unwrap();
        assert_eq!(
            wire_of(t.deliver().unwrap().unwrap()),
            first,
            "retry byte-özdeş değil"
        );
    }
    assert_eq!(first, vec![0xAB; 121]);
    t.forget(id);
    assert_eq!(t.retry(id), Err(MockTransportError::UnknownEnvelope));
}

/// Kayıp: zarf yola çıkmaz ama önbellekte KALIR → retry ile kurtarılır.
#[test]
fn f4_injected_drop_loses_the_transmission_not_the_envelope() {
    let mut t = MockTransport::new();
    t.inject(Fault::Drop);
    let id = t.submit(ENVELOPE_INIT, b"init").unwrap();
    assert!(t.deliver().is_none(), "düşürülen gönderim teslim edildi");
    assert_eq!((t.sent, t.dropped), (1, 1));
    t.retry(id).unwrap();
    assert_eq!(wire_of(t.deliver().unwrap().unwrap()), b"init");
}

/// Duplicate: aynı zarf iki kez, byte-özdeş (Crypto'nun idempotent `AcceptInit`'ini sürer).
#[test]
fn f5_injected_duplicate_delivers_twice() {
    let mut t = MockTransport::new();
    t.inject(Fault::Duplicate);
    t.submit(ENVELOPE_INIT, b"dup").unwrap();
    let a = wire_of(t.deliver().unwrap().unwrap());
    let b = wire_of(t.deliver().unwrap().unwrap());
    assert_eq!((a.as_slice(), b.as_slice()), (&b"dup"[..], &b"dup"[..]));
    assert!(t.deliver().is_none());
}

/// Reorder: işaretli zarf bir öncekinin önüne geçer; başka hiçbir şey yer değiştirmez.
#[test]
fn f6_injected_reorder_swaps_exactly_one_pair() {
    let mut t = MockTransport::new();
    t.submit(ENVELOPE_INIT, b"1").unwrap();
    t.submit(ENVELOPE_INIT, b"2").unwrap();
    t.inject(Fault::Reorder);
    t.submit(ENVELOPE_INIT, b"3").unwrap();
    t.submit(ENVELOPE_INIT, b"4").unwrap();
    let order: Vec<Vec<u8>> =
        std::iter::from_fn(|| t.deliver().map(|c| wire_of(c.unwrap()))).collect();
    assert_eq!(
        order,
        [b"1".to_vec(), b"3".to_vec(), b"2".to_vec(), b"4".to_vec()]
    );
}

/// Sınırlar: önbellek ve taşıma kuyruğu sınırsız büyümez; dolunca typed `Backpressure`.
#[test]
fn f7_queues_are_bounded_with_typed_backpressure() {
    let mut t = MockTransport::new();
    let mut ids: Vec<EnvelopeId> = Vec::new();
    for i in 0..MOCK_MAX_CACHED {
        ids.push(
            t.submit(ENVELOPE_INIT, &[i as u8])
                .expect("kapasite içinde"),
        );
        t.deliver(); // taşıma kuyruğunu boşalt; önbellek dolmaya devam eder
    }
    assert_eq!(
        t.submit(ENVELOPE_INIT, b"x"),
        Err(MockTransportError::Backpressure),
        "önbellek dolu"
    );
    t.forget(ids[0]);
    t.submit(ENVELOPE_INIT, b"x").expect("yer açıldı");

    let mut t = MockTransport::new();
    let id = t.submit(ENVELOPE_INIT, b"r").unwrap();
    for _ in 1..MOCK_MAX_IN_FLIGHT {
        t.retry(id).unwrap();
    }
    assert_eq!(t.in_flight_len(), MOCK_MAX_IN_FLIGHT);
    assert_eq!(
        t.retry(id),
        Err(MockTransportError::Backpressure),
        "taşıma kuyruğu dolu"
    );
    assert_eq!(
        t.in_flight_len(),
        MOCK_MAX_IN_FLIGHT,
        "reddedilen gönderim kuyruğa girmedi"
    );
}

/// INV-NEIGHBOR-1: sahte Transport Application'a ait hiçbir şeyi import etmez.
#[test]
fn f8_mock_transport_imports_nothing_from_the_application_layer() {
    let src = include_str!("common/mock_transport.rs");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for banned in [
        "kama_app",
        "kama_contracts::s1",
        "KamaTransport",
        "AppMessageRecord",
        "PreparedUnit",
    ] {
        assert!(
            !code.contains(banned),
            "sahte Transport Application sembolü içeriyor: {banned}"
        );
    }
}
