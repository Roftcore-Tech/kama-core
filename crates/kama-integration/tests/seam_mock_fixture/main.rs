//! **mock-fixture** — sahte modül consumer'ı yanıltmıyor mu.
//!
//! Google SWE ch.13'ün kuralı: *"A fake must have its own tests to ensure that it conforms to the
//! API of its corresponding real implementation."* Sahtenin sözleşmeye UYDUĞU conformance-core'da
//! ölçülür; burada ölçülen ayrı bir şeydir: sahtenin **yapamadığı şeyi yapıyormuş gibi
//! göstermemesi** ve kendi iddialarına sadık kalması. Bu testlerin gerçek hedefte karşılığı
//! yoktur (gerçek transport'un `MAX_INBOX`'ı, kayıp ENJEKSİYONU ya da gözlem sayacı yoktur), o
//! yüzden ayrı ikilide dururlar.
//!
//! Sınanan sahte `crates/kama-app/tests/common/mock_transport.rs` içindeki `MockTransport`'tur
//! (ported from feature/app — Hayri): Application'ın geliştirme sahtesiyle AYNI dosya, buraya
//! `#[path]` ile alınır. KM-SEAM-AT-001 §7 sahtesinin yokluğu (`GAP-SEAM-MOCK-7`) bu taşımayla
//! DARALDI, kapanmadı: §7'nin saydığı sırasız/duplicate enjeksiyonu ve supersession senaryosu
//! sahtede hâlâ yok. Sahtenin MODELLEMEDİKLERİ o dosyanın başında sayılıdır ve burada iddia edilmez.
//!
//! §6 satırı olan davranışlar (iptal, link olayları, sahte RESPONSE, cutover…) burada TEKRAR
//! sınanmaz — onlar conformance-core'dadır ve gerçek hedefte de koşacaktır.

#[path = "../common/seam/harness.rs"]
mod harness;

use std::time::Duration;

use harness::target::app_mock::{
    MockStats, MAX_EVENTS, MAX_INBOX, MAX_SESSIONS, PENDING_INIT_CAP, PROBE_INTERVAL,
};
use harness::target::{
    MockTarget, MockTransport, SeamTarget, TargetKind, STUB_MAX_PLAINTEXT_UNIT_BYTES,
    STUB_PREFERRED_PLAINTEXT_UNIT_BYTES,
};
use kama_contracts::handles::{PeerRef, SessionHandle};
use kama_contracts::s1::{
    AbandonReason, CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered,
    DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent, TransportError,
};

fn unit(bytes: &[u8]) -> PreparedUnit {
    PreparedUnit {
        bytes: bytes.to_vec(),
        compression_policy: CompressionPolicy::Never,
    }
}

fn build() -> MockTransport {
    MockTarget::build().expect("mock hedefi kurulmalı")
}

/// `connect` → `Pending` → (sahteyi sür) → `Ready` olayını okuyarak aktif handle üretir.
fn ready(t: &mut MockTransport, peer: PeerRef) -> SessionHandle {
    match t.connect(peer, None).expect("connect reddedilmemeli") {
        ConnectResult::Ready(h) => h,
        ConnectResult::Pending(_) => {
            t.pump();
            match t.poll_event() {
                Some(SessionEvent::Ready(h, p)) => {
                    assert_eq!(p, peer, "Ready olayı başka peer taşıdı");
                    h
                }
                other => panic!("Ready olayı bekleniyordu, gelen: {other:?}"),
            }
        }
    }
}

fn pending(t: &mut MockTransport, peer: PeerRef, early: Option<PreparedUnit>) -> ConnectTicket {
    match t.connect(peer, early).expect("connect reddedilmemeli") {
        ConnectResult::Pending(ticket) => ticket,
        ConnectResult::Ready(_) => panic!("taze peer için Pending bekleniyordu"),
    }
}

fn drain_events(t: &mut MockTransport) -> Vec<SessionEvent> {
    std::iter::from_fn(|| t.poll_event()).collect()
}

fn drain_deliveries(t: &mut MockTransport) -> Vec<Delivered> {
    std::iter::from_fn(|| t.recv()).collect()
}

/// Sahte, ailesini doğru beyan eder — "mock" diye koşan şey gerçekte gerçek adapter olamaz.
#[test]
fn f1_mock_declares_itself_as_mock() {
    assert_eq!(MockTarget::KIND, TargetKind::Mock);
    assert_eq!(MockTarget::ID, "mock");
}

/// Sahtenin enjeksiyon yüzeyinin tamamına dokunan sabit bir senaryonun gözlenen her çıktısı.
#[derive(Debug, PartialEq)]
struct Trace {
    connects: Vec<Result<ConnectResult, TransportError>>,
    sends: Vec<Result<(), TransportError>>,
    events: Vec<SessionEvent>,
    deliveries: Vec<Delivered>,
    stats: MockStats,
}

fn scripted_trace() -> Trace {
    let mut t = build();
    let (a, b) = (harness::test_peer(1), harness::test_peer(2));
    let mut connects = vec![
        t.connect(a, Some(unit(b"erken"))),
        t.connect(b, None),
        t.connect(a, None),
    ];
    t.inject_invalid_response(&b);
    t.set_link(false);
    t.advance_clock(PROBE_INTERVAL);
    t.set_link(true);
    t.advance_clock(PROBE_INTERVAL);
    t.pump();
    let mut events = drain_events(&mut t);
    let ready_handles: Vec<SessionHandle> = events
        .iter()
        .filter_map(|ev| match ev {
            SessionEvent::Ready(h, _) => Some(*h),
            _ => None,
        })
        .collect();
    let h = *ready_handles
        .first()
        .expect("senaryoda en az bir Ready olmalı");

    t.drop_next_units(1);
    let mut sends = vec![
        t.send(h, unit(b"bir"), DeliveryPolicy::Reliable),
        t.send(h, unit(b"iki"), DeliveryPolicy::BestEffort),
    ];
    t.pump();
    t.request_renew(h);
    let h2 = t.complete_renew(h);
    sends.push(t.send(h, unit(b"eski"), DeliveryPolicy::Reliable));
    sends.push(t.send(h2, unit(b"yeni"), DeliveryPolicy::Reliable));
    connects.push(t.connect(a, None));
    events.extend(drain_events(&mut t));
    Trace {
        connects,
        sends,
        events,
        deliveries: drain_deliveries(&mut t),
        stats: t.stats(),
    }
}

/// ADR-17 determinizmi sahte için de geçerli: aynı girdi → aynı çıktı.
///
/// Deterministik olmayan bir sahte, consumer'a var olmayan bir davranış çeşitliliği gösterir ve
/// "bende çalışıyordu" sınıfı hatalar üretir. Karşılaştırılan şey yalnız ilk `connect` değil,
/// senaryonun BÜTÜN izi: bilet/tutamaç numaraları, olay sırası, teslim sırası, sayaçlar.
#[test]
fn f2_mock_is_deterministic_across_instances() {
    let (first, second) = (scripted_trace(), scripted_trace());
    assert_eq!(
        first, second,
        "iki taze örnek ayrıştı — sahte gizli durum taşıyor"
    );
    // İz boş değil: senaryo gerçekten bir şey gözledi (karşılaştırma vakum değil).
    assert!(!first.events.is_empty() && !first.deliveries.is_empty());
}

/// Sahte, YAPAMADIĞI teslimi uydurmaz.
///
/// Consumer'ı yanıltmanın en pahalı biçimi budur: app, transport'un kuyruğa aldığını sanıp
/// (K8: `Ok(())` = kuyruğa alındı) kendi retry'sini yazmaz ve birim sessizce kaybolur.
#[test]
fn f3_mock_does_not_fabricate_delivery() {
    let mut t = build();

    // Hiç connect edilmemiş handle.
    assert_eq!(
        t.send(
            SessionHandle::from_raw(7),
            unit(b"kama"),
            DeliveryPolicy::Reliable
        ),
        Err(TransportError::UnknownSession)
    );

    // Pending oturum da aktif DEĞİLDİR: Ready gelmeden send kabul edilmez (§K7). Sahte bilet ve
    // tutamaç numaralarını tek sayaçtan basar; ilk biletin numarasıyla kurulan bir tutamaç da
    // (bilet tutamaç yerine geçemez) reddedilir.
    let _ticket = pending(&mut t, harness::test_peer(9), None);
    let first_issued_id = SessionHandle::from_raw(1);
    assert_eq!(
        t.send(first_issued_id, unit(b"erken"), DeliveryPolicy::Reliable),
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
    let t = build();
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
    // Pending tavanı tablo tavanının ALTINDA olmalı: yoksa pending tavanına hiç ulaşılamaz ve
    // H-PENDING-CAP tablo tavanını ölçerdi.
    const { assert!(PENDING_INIT_CAP < MAX_SESSIONS) };

    // Teslim kuyruğu.
    let mut t = build();
    let h = ready(&mut t, harness::test_peer(3));
    for i in 0..MAX_INBOX {
        assert_eq!(
            t.send(h, unit(b"x"), DeliveryPolicy::Reliable),
            Ok(()),
            "kuyruk dolmadan {i}. send reddedildi"
        );
    }
    assert_eq!(
        t.send(h, unit(b"x"), DeliveryPolicy::Reliable),
        Err(TransportError::Backpressure),
        "teslim kuyruğu tavanında sessizce büyüdü"
    );

    // Oturum tablosu. Her oturum `Ready`'ye sürülür: pending birikseydi önce pending INIT
    // tavanına (H-PENDING-CAP) takılırdık ve ölçülen şey tablo tavanı olmazdı.
    let mut t = build();
    for i in 0..MAX_SESSIONS {
        ready(&mut t, harness::test_peer(100 + i as u64));
    }
    assert_eq!(
        t.connect(harness::test_peer(999), None),
        Err(TransportError::Backpressure),
        "oturum tavanında sessizce büyüdü"
    );

    // Olay kuyruğu: okunmayan olaylar tavanın üstüne BİRİKMEZ.
    const OVERFLOW: usize = 5;
    let mut t = build();
    let h = ready(&mut t, harness::test_peer(4));
    for _ in 0..MAX_EVENTS + OVERFLOW {
        t.request_renew(h);
    }
    let queued = drain_events(&mut t);
    assert_eq!(queued.len(), MAX_EVENTS, "olay kuyruğu tavanını aştı");
    assert_eq!(
        t.stats().events_dropped,
        OVERFLOW as u64,
        "düşen olaylar sayılmadı (düşme sessiz kaldı)"
    );
    assert!(queued
        .iter()
        .all(|ev| *ev == SessionEvent::RenewRequired(h)));
}

/// Kayıp YALNIZ kurulduğunda ve TAM kurulduğu kadar enjekte edilir; sayaçlar gerçeği söyler.
///
/// Sessizce kayıp veren (ya da kurulduğundan fazla/az kaybeden) bir sahte, H-APP-RETRY-0'ın
/// ölçmek istediği farkı ölçülemez hâle getirirdi. `send()` üç durumda da `Ok(())` döner:
/// kabul teslim değildir (K8).
#[test]
fn f7_loss_is_injected_only_when_armed_and_counted_exactly() {
    const UNITS: u8 = 8;
    const DROPS: usize = 3;
    let payloads: Vec<Vec<u8>> = (0..UNITS).map(|i| vec![i; usize::from(i) + 1]).collect();
    let send_all = |t: &mut MockTransport, h: SessionHandle| {
        for p in &payloads {
            assert_eq!(t.send(h, unit(p), DeliveryPolicy::Reliable), Ok(()));
        }
    };
    let bodies = |ds: Vec<Delivered>, h: SessionHandle| -> Vec<Vec<u8>> {
        ds.into_iter()
            .map(|d| {
                assert_eq!(d.session, h, "birim başka oturuma teslim edildi");
                d.unit
            })
            .collect()
    };

    // (a) Kurulmamışken: N birim girdi → N birim, gönderim sırasında, tam olarak bir kez.
    let mut t = build();
    let h = ready(&mut t, harness::test_peer(5));
    send_all(&mut t, h);
    assert_eq!(bodies(drain_deliveries(&mut t), h), payloads);
    assert_eq!(
        t.stats().units_dropped,
        0,
        "kurulmadan kayıp enjekte edildi"
    );

    // (b) Recovery AÇIK: ilk DROPS birim kaybolur; Transport sürülmeden hiçbiri (ve sıra
    // bozulmasın diye arkalarındakiler de) teslim edilmez; sürülünce hepsi, sırayla, bir kez.
    let mut t = build();
    let h = ready(&mut t, harness::test_peer(6));
    t.set_recovery(true);
    t.drop_next_units(DROPS);
    send_all(&mut t, h);
    assert!(
        t.recv().is_none(),
        "kaybolan birim kurtarılmadan teslim edildi"
    );
    t.pump();
    assert_eq!(bodies(drain_deliveries(&mut t), h), payloads);
    t.pump();
    assert!(t.recv().is_none(), "kurtarma ikinci kez teslim etti");
    let stats = t.stats();
    assert_eq!(stats.units_dropped, DROPS as u64);
    assert_eq!(
        stats.units_recovered, DROPS as u64,
        "kurtarılan sayısı kaybolan sayısı değil (sıra için bekleyenler de sayıldı?)"
    );

    // (c) Recovery KAPALI: aynı DROPS birim KALICI kaybolur; kalanı sırasıyla gelir.
    let mut t = build();
    let h = ready(&mut t, harness::test_peer(7));
    t.set_recovery(false);
    t.drop_next_units(DROPS);
    send_all(&mut t, h);
    t.pump();
    assert_eq!(bodies(drain_deliveries(&mut t), h), payloads[DROPS..]);
    let stats = t.stats();
    assert_eq!(stats.units_dropped, DROPS as u64);
    assert_eq!(
        stats.units_recovered, 0,
        "recovery kapalıyken birim kurtarıldı"
    );
}

/// `abandon_connect` yalnız verilen KAPALI nedeni yayınlar ve fazlasını uydurmaz.
///
/// Kullanıcı iptali (`Cancelled`) `cancel(ticket)` yoludur ve conformance-core'da H-CANCEL olarak
/// sınanır; buradaki üç neden (§4 (b)–(d)) Application'ın elinde olmayan sonlanmalardır ve yalnız
/// sahtenin enjeksiyonuyla üretilebilir.
#[test]
fn f8_abandon_emits_exactly_the_injected_closed_reason() {
    for (i, reason) in [
        AbandonReason::PeerClosed,
        AbandonReason::KeyHygiene,
        AbandonReason::LocalPermanent,
    ]
    .into_iter()
    .enumerate()
    {
        let mut t = build();
        let peer = harness::test_peer(50 + i as u64);
        let ticket = pending(&mut t, peer, Some(unit(b"erken")));

        t.abandon_connect(&peer, reason);
        assert_eq!(
            drain_events(&mut t),
            vec![SessionEvent::ConnectAbandoned(ticket, reason)],
            "sonlandırma tek olay üretmeli: ConnectAbandoned(ticket, {reason:?})"
        );

        // Sonlanan niyet dirilmez: sürülse de `Ready` gelmez, `early_unit` teslim edilmez.
        t.pump();
        assert!(
            t.poll_event().is_none(),
            "sonlandırılan niyet için olay geldi"
        );
        assert!(
            t.recv().is_none(),
            "sonlandırılan niyetin early_unit'i teslim edildi"
        );

        // Bilet geçersiz: aynı eşe yeniden `connect` YENİ bilet verir.
        assert_ne!(
            pending(&mut t, peer, None),
            ticket,
            "sonlandırma sonrası aynı bilet geri verildi"
        );
    }
}

/// `close()` yalnız kapatılan tutamacı unutturur.
///
/// Supersede edilmiş ESKİ tutamacı kapatmak, eşin YENİ aktif oturumunu silmemelidir; yoksa
/// Application cutover'dan sonra eski tutamacı temizlerken yeni oturumunu kaybeder. `close()`'un
/// bir `Closed` olayı üretip üretmediği burada SINANMAZ: S1 bunu yazmıyor (işaretli açık nokta).
#[test]
fn f9_close_forgets_only_the_closed_handle() {
    let mut t = build();
    let peer = harness::test_peer(8);
    let old = ready(&mut t, peer);
    t.request_renew(old);
    let new = t.complete_renew(old);

    t.close(old);
    assert_eq!(
        t.send(old, unit(b"eski"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession)
    );
    assert_eq!(
        t.connect(peer, None),
        Ok(ConnectResult::Ready(new)),
        "eski tutamacı kapatmak eşin yeni aktif oturumunu sildi"
    );
    assert_eq!(t.send(new, unit(b"yeni"), DeliveryPolicy::Reliable), Ok(()));

    // Aktif tutamacı kapatmak onu unutturur ve eşi yeniden bağlanılabilir kılar.
    t.close(new);
    assert_eq!(
        t.send(new, unit(b"gec"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession)
    );
    assert!(
        matches!(t.connect(peer, None), Ok(ConnectResult::Pending(_))),
        "kapatılan oturumun eşi hâlâ aktif görünüyor"
    );
}

/// Link YOKKEN açılan niyet doğduğu anda `Suspended` bildirilir ve prob üretmez.
///
/// H-PROGRESS bekleyen niyetin link GEÇİŞLERİNİ sınar; bu sınama sahtenin ek iddiasını yerinde
/// tutar: link yokken `connect` eden Application "gönderiliyor" sanmaz. S1 §4 yeni bir niyetin
/// İLK `ConnectProgress`'inin ne zaman çıkacağını yazmıyor (işaretli açık nokta); sahte, link
/// varken doğan niyet için olay ÜRETMEZ.
#[test]
fn f10_intent_born_without_link_is_reported_suspended() {
    const LONG: Duration = Duration::from_secs(60 * 60);

    let mut t = build();
    t.set_link(false);
    let probes_before = t.stats().probes;
    let ticket = pending(&mut t, harness::test_peer(10), None);
    assert_eq!(
        drain_events(&mut t),
        vec![SessionEvent::ConnectProgress(
            ticket,
            ConnectPhase::Suspended
        )]
    );
    t.advance_clock(LONG);
    assert_eq!(t.stats().probes, probes_before, "link yokken prob çıktı");

    // Karşı durum: link varken doğan niyet için olay yok.
    let mut t = build();
    let _ticket = pending(&mut t, harness::test_peer(11), None);
    assert!(t.poll_event().is_none());
}

/// Kabul edilmiş `early_unit`, teslim kuyruğu dolu diye DÜŞÜRÜLMEZ.
///
/// `connect(peer, Some(X))` X'i kabul etti; handshake kuyruk doluyken tamamlanırsa X bekler ve yer
/// açılınca teslim edilir. Sessizce düşen bir `early_unit`, Application'ın "ilk mesajım gitti"
/// varsayımını sahtede sınanamaz kılardı.
#[test]
fn f11_accepted_early_unit_survives_a_full_inbox() {
    let mut t = build();
    let filler = ready(&mut t, harness::test_peer(12));
    for _ in 0..MAX_INBOX {
        assert_eq!(t.send(filler, unit(b"x"), DeliveryPolicy::Reliable), Ok(()));
    }

    let peer = harness::test_peer(13);
    let _ticket = pending(&mut t, peer, Some(unit(b"erken")));
    t.pump(); // handshake tamamlanır; teslim kuyruğu hâlâ dolu
    let h = match t.poll_event() {
        Some(SessionEvent::Ready(h, p)) if p == peer => h,
        other => panic!("Ready bekleniyordu, gelen: {other:?}"),
    };

    assert_eq!(drain_deliveries(&mut t).len(), MAX_INBOX);
    t.pump();
    assert_eq!(
        drain_deliveries(&mut t),
        vec![Delivered {
            session: h,
            unit: b"erken".to_vec(),
        }],
        "kabul edilmiş early_unit düşürüldü ya da başka oturuma gitti"
    );
}

/// Düşen bir `Closed` olayı tutamacı "supersede" durumunda bırakmaz.
///
/// Olay kuyruğu doluyken yapılan cutover'ın `Ready(h')` ve `Closed(h)` olayları kuyruğa sığmaz.
/// `Closed` hiç okunamayacağı için eski tutamaç o anda unutulur (`UnknownSession`); yoksa
/// Application `close(h)` çağırmadıkça — ki onu buna yöneltecek olay hiç gelmeyecektir —
/// `send(h)` `SessionSuperseded` dönmeye devam eder ve supersede kümesi büyürdü.
#[test]
fn f12_dropped_closed_event_does_not_strand_the_handle() {
    /// Cutover'ın ürettiği olay sayısı: `Ready(h')` + `Closed(h, Superseded)`.
    const CUTOVER_EVENTS: u64 = 2;

    let mut t = build();
    let old = ready(&mut t, harness::test_peer(14));
    for _ in 0..MAX_EVENTS {
        t.request_renew(old); // kuyruk okunmadan dolar
    }
    let new = t.complete_renew(old);
    assert_eq!(t.stats().events_dropped, CUTOVER_EVENTS);

    assert_eq!(
        t.send(old, unit(b"eski"), DeliveryPolicy::Reliable),
        Err(TransportError::UnknownSession),
        "Closed olayı düşen tutamaç supersede durumunda takılı kaldı"
    );
    assert_eq!(t.send(new, unit(b"yeni"), DeliveryPolicy::Reliable), Ok(()));
}
