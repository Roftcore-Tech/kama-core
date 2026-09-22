//! **mock-fixture** — sahte modül consumer'ı yanıltmıyor mu.
//!
//! Google SWE ch.13'ün kuralı: *"A fake must have its own tests to ensure that it conforms to the
//! API of its corresponding real implementation."* Sahtenin sözleşmeye UYDUĞU conformance-core'da
//! ölçülür; burada ölçülen ayrı bir şeydir: sahtenin **yapamadığı şeyi yapıyormuş gibi
//! göstermemesi**. Bu testlerin gerçek hedefte karşılığı yoktur, o yüzden ayrı ikilide dururlar.
//!
//! Bugünkü sahte ([`harness::target::NullMockTransport`]) KM-SEAM-AT-001 §7'nin `MockTransport`'u
//! DEĞİLDİR (kayıp/sırasız/duplicate enjeksiyonu yok; o ayrı teslimat). Dolayısıyla buradaki
//! sadakat iddiaları da dar: determinizm, teslim uydurmama, kapasite sayılarının beyanlı olması.

#[path = "../common/seam/harness.rs"]
mod harness;

use harness::api::{
    CompressionPolicy, DeliveryPolicy, KamaTransport, PeerRef, PreparedUnit, SessionHandle,
    TransportError,
};
use harness::target::{
    MockTarget, SeamTarget, TargetKind, STUB_MAX_PLAINTEXT_UNIT_BYTES,
    STUB_PREFERRED_PLAINTEXT_UNIT_BYTES,
};

fn unit(bytes: &[u8]) -> PreparedUnit {
    PreparedUnit {
        bytes: bytes.to_vec(),
        compression_policy: CompressionPolicy::Never,
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
fn f2_mock_refusals_are_deterministic() {
    let mut a = MockTarget::build().expect("mock hedefi kurulmalı");
    let mut b = MockTarget::build().expect("mock hedefi kurulmalı");
    let peer = PeerRef::from_raw(1);

    let r1 = a.connect(peer, None);
    let r2 = a.connect(peer, None);
    let r3 = b.connect(peer, None);
    assert_eq!(r1, r2, "aynı örnekte iki çağrı ayrıştı");
    assert_eq!(r1, r3, "iki ayrı örnek ayrıştı — sahte gizli durum taşıyor");
    assert_eq!(r1, Err(TransportError::NotConnected));
}

/// Sahte, YAPAMADIĞI teslimi uydurmaz: reddedilen `send` sonrası `recv()` boş kalır.
///
/// Consumer'ı yanıltmanın en pahalı biçimi budur — app, transport'un kuyruğa aldığını sanıp
/// (K8: `Ok(())` = kuyruğa alındı) kendi retry'sini yazmaz ve birim sessizce kaybolur.
#[test]
fn f3_mock_does_not_fabricate_delivery() {
    let mut t = MockTarget::build().expect("mock hedefi kurulmalı");
    let sent = t.send(
        SessionHandle::from_raw(7),
        unit(b"kama"),
        DeliveryPolicy::Reliable,
    );
    assert_eq!(
        sent,
        Err(TransportError::NotConnected),
        "kurulu oturum yokken send kabul edildi"
    );
    assert!(
        t.recv().is_none(),
        "reddedilen send'den sonra teslim uydurdu"
    );
    assert!(
        t.poll_event().is_none(),
        "reddedilen send'den sonra olay uydurdu"
    );
}

/// Sahtenin kapasite sayıları BEYANLI yer tutuculardır; sessizce "gerçek değer" hâline gelemezler.
///
/// Sabitler değişirse bu pin kırılır ve değiştiren kişi `target.rs`'teki "normatif değil" notunu
/// okumak zorunda kalır. Gerçek değerler K5 zincirinden (`conservative_limits`) türeyecektir.
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

/// Sahte, uygulamadığı §6 satırını uygulamış gibi ilan etmez.
///
/// `IMPLEMENTED_CONTRACT_ROWS` bugün boştur; runner bunu `seam-coverage.json` beyanıyla çapraz
/// kontrol eder. Buradaki sınama o zincirin Rust ucunu yerinde tutar.
#[test]
fn f5_mock_claims_no_unimplemented_contract_rows() {
    assert!(
        harness::contract::IMPLEMENTED_CONTRACT_ROWS.is_empty(),
        "kodda §6 satırı uygulanmış görünüyor; seam-coverage.json AYNI commit'te güncellendi mi?"
    );
}
