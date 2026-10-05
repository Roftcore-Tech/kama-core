//! Sözleşme süiti — **tek yerde yazılır, her hedefte ayrı koşar**.
//!
//! Doktrin (Google SWE ch.13 "contract tests" / Fowler "ContractTest"): sahtenin doğruluğu,
//! gerçek implementasyonla AYNI test kodunun ikisine de koşturulmasıyla kanıtlanır. Burada test
//! gövdeleri generic `fn`'lerdir; [`seam_mechanism_suite!`] / [`seam_contract_suite!`] makroları
//! onları hedef başına ayrı `#[test]` olarak sarar. Böylece:
//!
//! * gövde TEK kopyadır (sahte ile gerçek ayrışamaz — drift imkânsız),
//! * `cargo test` çıktısında her hedef AYRI isimle görünür (`mock::m3_…`, `real::m3_…`),
//!   yani "hangi hedef koştu" çıktıdan okunur; çalışma-zamanı `if target == …` dallanması
//!   bilerek REDDEDİLDİ (tek test adı iki davranış gizler).
//!
//! # İki katman, karıştırılmaz
//!
//! * **mekanizma** ([`mechanism`]) — harness'in kendisinin çalıştığını kanıtlar (hedef kaydı,
//!   typed kurulum, taze hedefte oturum icat edilmemesi, kapasite tutarlılığı). Bunlar KM-SEAM-
//!   AT-001 §6 satırları DEĞİLDİR ve öyleymiş gibi sayılmazlar.
//!   Not: §6'nın 12 DAVRANIŞ satırı ortak koşum yüzeyinde (conformance-core) yaşar;
//!   `H-NO-CRYPTO-EDGE` ve `H-NO-PLANE1` spec'te STATİK kontrol olarak tanımlıdır ve koşum değil
//!   kapı-seviyesi denetimdir: `scripts/ci/layer_check.py` (beyanda durum `static`, kanıt
//!   `layer_check`; runner mock koşusunda o kapıyı da koşturur).
//! * **sözleşme** ([`seam_contract_suite!`]) — §6'nın 12 davranış satırı. **Bugün 12'sinin de
//!   gövdesi vardır**: yedisi `feature/app`'teki `conf_s1_seam.rs` satırlarıdır (Hayri —
//!   H-ROUNDTRIP, H-EVENTS, H-CUTOVER, H-CONNECT-IDEMP, H-EARLY-CONFLICT, H-CAPS,
//!   H-COMPRESS-BOUNDARY; H-ROUNDTRIP'in gövdesi bu süitte zaten vardı ve özgün testin iddialarını
//!   kapsar, diğer altısı oradan taşındı), dördü eksik kalan satırlardır (H-APP-RETRY-0, H-CANCEL,
//!   H-PROGRESS, H-SILENT-REJECT), H-PENDING-CAP G-S3 kanıt işinden gelir. On ikisi de
//!   `mock_only`'dir: yalnız sahtede koştukları için kanıt değerleri ikinci
//!   hedef gelince doğar. Bu ayrım `scripts/ci/seam-coverage.json` içinde satır satır beyan
//!   edilir; runner, Rust tarafının bildirdiği listeyi (aşağıdaki sentinel) o beyanla ve fiilen
//!   LİSTELENEN test adlarıyla ÇAPRAZ KONTROL eder ve tek hedef varken hiçbir satırın "kanıtlandı"
//!   sayılmasına izin vermez. Beyanla gerçeklik ayrışırsa kapı kırmızı.
//!
//! # Satır gövdeleri ve senaryo kancaları
//!
//! Bir satırın ihtiyaç duyduğu DIŞ koşul (karşı ucun yanıtı, link durumu, saat, kayıp) gövdede
//! üretilmez; [`crate::harness::target::SeamTarget`] kancalarından istenir. Sahte onları doğrudan
//! enjekte eder; gerçek hedef aynı kancaları gerçek bir karşı uç fikstürüyle dolduracaktır. Bir
//! satırın sahtede yeşil olması, kancanın sahtedeki karşılığı kadar güçlüdür: örn. sahtenin
//! "sahte RESPONSE"u hiçbir kripto çalıştırmaz, "recovery"si ARQ/FEC değil bir anahtardır. Bu
//! sınırlar satır satır `seam-coverage.json`'ın `needs` alanında yazılıdır.

#![allow(dead_code)]

/// Rust tarafının GERÇEKLİĞİ: süitin fiilen koşturduğu §6 satırlarının kimlikleri.
///
/// Burası tek doğruluk kaynağı DEĞİLDİR; §6 satır listesinin ve durumlarının otoritesi
/// `scripts/ci/seam-coverage.json`'dır (o da spec §6 tablosuna karşı `seam-api-drift.py` ile
/// denetlenir). Bu sabit yalnız "kodda fiilen ne koşuyor" der; runner ikisini karşılaştırır.
/// Yeni bir §6 satırı uygulandığında AYNI commit'te buraya ve JSON'a eklenir.
///
/// **Bir satırın burada olması "kanıtlandı" demek DEĞİLDİR.** Tek hedef (mock) kayıtlıyken bir
/// satır olsa olsa `mock_only`'dir: gövde hazırdır, doğrulama gücü ikinci hedef gelince doğar
/// (Google SWE ch.13 / Fowler ContractTest). Beyandaki durum ayrımını runner zorlar.
pub const CONTRACT_ROWS: &[&str] = &[
    "H-APP-RETRY-0",
    "H-ROUNDTRIP",
    "H-EVENTS",
    "H-CUTOVER",
    "H-CONNECT-IDEMP",
    "H-EARLY-CONFLICT",
    "H-CAPS",
    "H-PENDING-CAP",
    "H-CANCEL",
    "H-PROGRESS",
    "H-SILENT-REJECT",
    "H-COMPRESS-BOUNDARY",
];

/// Runner'ın okuduğu makine-satırı. `--nocapture` ile basılır.
pub const CONTRACT_ROWS_SENTINEL: &str = "SEAM-CONTRACT-ROWS";

/// Hedefe bakmaksızın sabit olan beyan tutarlılığı + sentinel emisyonu.
///
/// # Panics
/// Beyan listesi kendi içinde tutarsızsa (boş kimlik, tekrar eden kimlik).
pub fn emit_contract_row_sentinel(target_id: &str) {
    let rows = CONTRACT_ROWS;
    for (i, id) in rows.iter().enumerate() {
        assert!(!id.is_empty(), "boş §6 satır kimliği (index {i})");
        assert!(
            !rows[..i].contains(id),
            "§6 satır kimliği iki kez beyan edildi: {id}"
        );
    }
    println!(
        "{CONTRACT_ROWS_SENTINEL}: target={target_id} rows={}",
        rows.join(",")
    );
}

/// KM-SEAM-AT-001 §6 satır gövdeleri — her hedefte AYNI kod koşar.
pub mod rows {
    use crate::harness::target::SeamTarget;
    use std::time::Duration;

    use kama_contracts::handles::{CloseReason, PeerRef, SessionHandle};
    use kama_contracts::s1::{
        AbandonReason, CompressionPolicy, ConnectPhase, ConnectResult, ConnectTicket, Delivered,
        DeliveryPolicy, KamaTransport, PreparedUnit, SessionEvent, TransportError,
    };

    /// `Ready`'yi beklerken atılacak azami adım. Sınırsız döngü YOK: gerçek hedef ilerlemezse
    /// test asılı kalmaz, KIRMIZI olur.
    const PUMP_BUDGET: usize = 64;

    /// `connect` → (`Pending` →) `Ready` zincirini sürer ve aktif handle'ı döndürür.
    ///
    /// # Panics
    /// Bütçe içinde `Ready` gelmezse ya da olay başka peer taşırsa.
    fn establish<T: SeamTarget>(
        t: &mut T::Transport,
        peer: PeerRef,
        early: Option<PreparedUnit>,
    ) -> SessionHandle {
        match t.connect(peer, early).expect("connect reddedilmemeli") {
            ConnectResult::Ready(h) => h,
            ConnectResult::Pending(_) => {
                for _ in 0..PUMP_BUDGET {
                    T::pump(t);
                    match t.poll_event() {
                        Some(SessionEvent::Ready(h, p)) => {
                            assert_eq!(p, peer, "Ready olayı başka peer taşıdı");
                            return h;
                        }
                        Some(other) => panic!("Ready'den önce beklenmeyen olay: {other:?}"),
                        None => continue,
                    }
                }
                panic!("{} adımda Ready gelmedi (hedef {})", PUMP_BUDGET, T::ID);
            }
        }
    }

    /// **H-ROUNDTRIP** — §6: `send(PreparedUnit)` → `recv()` byte-identical; `early_unit`
    /// byte-identical.
    ///
    /// Ölçtüğü şey: Transport birimin **byte'larına dokunmaz**. Çerçeveleme sızmaz, kırpma yok,
    /// sıkıştırma yok (K3/K4), teslim uydurulmaz ve tekrarlanmaz. Bu iddia hedeften bağımsızdır;
    /// hedef sözleşmesi gereği peer echo yaptığı için aynı gövde gerçek adapter'da da koşar.
    ///
    /// # Panics
    /// Byte'lar değişirse, teslim gelmezse, oturum eşleşmezse ya da fazladan teslim üretilirse.
    pub fn h_roundtrip<T: SeamTarget>() {
        let mut t = T::build().unwrap_or_else(|e| {
            panic!(
                "hedef {} kurulamadı ({e}) — uygulanmış satır koşamayan hedefte olamaz",
                T::ID
            )
        });
        let caps = t.capabilities();
        let peer = crate::harness::test_peer(0xE0);

        // early_unit: opak, byte-özdeş geri gelmeli (K6).
        let early_bytes: Vec<u8> = b"AMP-early-unit-record".to_vec();
        assert!(early_bytes.len() <= caps.max_plaintext_unit_bytes as usize);
        let h = establish::<T>(
            &mut t,
            peer,
            Some(PreparedUnit {
                bytes: early_bytes.clone(),
                compression_policy: CompressionPolicy::Never,
            }),
        );

        let delivered = t.recv().expect("early_unit teslim edilmedi");
        assert_eq!(
            delivered.session, h,
            "early_unit başka oturuma teslim edildi"
        );
        assert_eq!(
            delivered.unit,
            early_bytes,
            "early_unit byte-özdeş gelmedi (hedef {})",
            T::ID
        );

        // send → recv: 0..=255 deseni, kapasite sınırına göre kırpılır.
        let n = core::cmp::min(256, caps.max_plaintext_unit_bytes as usize);
        let payload: Vec<u8> = (0..n).map(|i| (i % 256) as u8).collect();
        t.send(
            h,
            PreparedUnit {
                bytes: payload.clone(),
                compression_policy: CompressionPolicy::Never,
            },
            DeliveryPolicy::Reliable,
        )
        .expect("send kuyruğa alınmalı (K8: Ok = kuyruğa alındı)");

        let mut got = None;
        for _ in 0..PUMP_BUDGET {
            T::pump(&mut t);
            if let Some(d) = t.recv() {
                got = Some(d);
                break;
            }
        }
        let got = got.expect("send edilen birim teslim edilmedi");
        assert_eq!(got.session, h, "birim başka oturuma teslim edildi");
        assert_eq!(
            got.unit,
            payload,
            "birim byte-özdeş gelmedi (hedef {})",
            T::ID
        );

        // Fazladan teslim YOK (duplicate/uydurma).
        assert!(t.recv().is_none(), "hedef {} fazladan teslim üretti", T::ID);
    }

    /// **H-PENDING-CAP** — §6 / §3: istemci pending INIT tavanı kadar bekleyen `connect` varken yeni
    /// bir eşe `connect()` → `TransportError::Backpressure`; pending kümesi DEĞİŞMEZ.
    ///
    /// Tavan hedefin adlandırılmış sabitinden okunur ve ccr §35.15'teki sayıyla karşılaştırılır
    /// (KS-DOS-1; tutarlılık zinciri: `PENDING_RESPONSE_TRIAL_CAP` = istemci pending INIT tavanı).
    /// Satır hedefin `connect`'leri PENDING tutabilmesini ister: olaylar okunmaz, hedef sürülmez.
    ///
    /// # Panics
    /// Tavanın altında `connect` reddedilirse, tavanda kabul edilirse, ret bir state bırakırsa ya da
    /// hedefin tavanı spec'teki değerden farklıysa.
    pub fn h_pending_cap<T: SeamTarget>() {
        const SPEC_NAME: &str = "İstemci pending INIT tavanı = ";
        /// Bu satırın eşleri bu tabandan numaralanır.
        const PEER_BASE: u64 = 0x0050_0000;

        let cap = T::PENDING_INIT_CAP;
        let spec = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../specs/ccr-format-spec.md"
        ))
        .expect("ccr-format-spec.md okunmalı");
        let spec_values: Vec<usize> = spec
            .lines()
            .filter(|l| !l.contains("~~"))
            .flat_map(|l| {
                l.match_indices(SPEC_NAME)
                    .map(move |(i, _)| &l[i + SPEC_NAME.len()..])
            })
            .filter_map(|rest| {
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().ok()
            })
            .collect();
        assert!(
            !spec_values.is_empty(),
            "ccr §35.15'te sayısal pending INIT tavanı bulunamadı"
        );
        assert!(
            spec_values.iter().all(|v| *v == cap),
            "spec {spec_values:?} ≠ hedef sabiti {cap}"
        );

        let mut t = T::build().unwrap_or_else(|e| {
            panic!(
                "hedef {} kurulamadı ({e}) — uygulanmış satır koşamayan hedefte olamaz",
                T::ID
            )
        });
        let peer = |i: usize| crate::harness::test_peer(PEER_BASE + i as u64);

        // Tavan kadar niyet açılır ve PENDING kalır.
        let mut tickets = Vec::with_capacity(cap);
        for i in 0..cap {
            match t.connect(peer(i), None) {
                Ok(ConnectResult::Pending(ticket)) => tickets.push(ticket),
                other => panic!("tavanın altında {i}. connect Pending vermedi: {other:?}"),
            }
        }

        // Tavan dolu: yeni eşe connect → Backpressure. early_unit'li de aynı; birim hiçbir yola girmez.
        let extra = peer(cap);
        assert_eq!(t.connect(extra, None), Err(TransportError::Backpressure));
        let early = PreparedUnit {
            bytes: b"tavanda reddedilen early_unit".to_vec(),
            compression_policy: CompressionPolicy::Never,
        };
        assert_eq!(
            t.connect(extra, Some(early)),
            Err(TransportError::Backpressure)
        );
        assert!(
            t.recv().is_none(),
            "reddedilen early_unit teslim yoluna girdi"
        );

        // Pending kümesi değişmedi: bekleyen her eşe connect idempotent ve AYNI ticket'ı verir;
        // reddedilen eş pending'e girmedi (hâlâ Backpressure).
        for (i, ticket) in tickets.iter().enumerate() {
            assert_eq!(
                t.connect(peer(i), None),
                Ok(ConnectResult::Pending(*ticket)),
                "{i}. bekleyen niyetin ticket'ı değişti"
            );
        }
        assert_eq!(t.connect(extra, None), Err(TransportError::Backpressure));

        // Ret kalıcı bir durum değildir: bir niyet iptal edilince yer açılır ve aynı eş kabul edilir.
        t.cancel(tickets[0]);
        assert!(
            matches!(t.connect(extra, None), Ok(ConnectResult::Pending(_))),
            "yer açıldıktan sonra connect hâlâ reddediliyor"
        );
    }

    // ── ortak yardımcılar ────────────────────────────────────────────────────────────────────

    fn fresh<T: SeamTarget>() -> T::Transport {
        T::build().unwrap_or_else(|e| {
            panic!(
                "hedef {} kurulamadı ({e}) — uygulanmış satır koşamayan hedefte olamaz",
                T::ID
            )
        })
    }

    fn unit(bytes: &[u8]) -> PreparedUnit {
        PreparedUnit {
            bytes: bytes.to_vec(),
            compression_policy: CompressionPolicy::Never,
        }
    }

    /// Bir sonraki olayı bekler (hedefi sürerek). Bütçe dolarsa `None`.
    fn next_event<T: SeamTarget>(t: &mut T::Transport) -> Option<SessionEvent> {
        for _ in 0..PUMP_BUDGET {
            if let Some(ev) = t.poll_event() {
                return Some(ev);
            }
            T::pump(t);
        }
        None
    }

    /// Bir sonraki teslimi bekler (hedefi sürerek). Bütçe dolarsa `None`.
    fn next_delivery<T: SeamTarget>(t: &mut T::Transport) -> Option<Delivered> {
        for _ in 0..PUMP_BUDGET {
            if let Some(d) = t.recv() {
                return Some(d);
            }
            T::pump(t);
        }
        None
    }

    /// Hedef ne kadar sürülürse sürülsün başka olay ÇIKMADIĞINI doğrular.
    fn assert_no_event<T: SeamTarget>(t: &mut T::Transport, why: &str) {
        for _ in 0..PUMP_BUDGET {
            if let Some(ev) = t.poll_event() {
                panic!("{why}: beklenmeyen olay {ev:?}");
            }
            T::pump(t);
        }
    }

    /// `Pending(ticket)` bekler.
    fn pending_ticket(r: Result<ConnectResult, TransportError>, what: &str) -> ConnectTicket {
        match r {
            Ok(ConnectResult::Pending(ticket)) => ticket,
            other => panic!("{what}: Pending bekleniyordu, gelen {other:?}"),
        }
    }

    // ── Hayri'nin yedi satırı (ported from feature/app; güncel S1'e göre) ────────────────────

    /// **H-EVENTS** — §6: `Ready → RenewRequired → Ready(h') → Closed(h, Superseded)` sırası;
    /// `Closed` sonrası `send(h)` → `UnknownSession`.
    ///
    /// "`Closed` sonrası" burada `Closed` olayının Application'a TESLİMİNDEN sonrası olarak okunur
    /// (K7 aynı dili kullanır: "`Ready` gelmeden `send(h)` → `UnknownSession`"). `Ready(h')` ile
    /// `Closed` arasındaki pencere H-CUTOVER'ındır (`SessionSuperseded`). Hayri'nin özgün testi
    /// `UnknownSession`'ı `close(h)` çağrısından sonra bekliyordu; gövde iki biçimi de sınar.
    ///
    /// # Panics
    /// Olay sırası farklıysa, fazladan olay çıkarsa ya da `Closed` sonrası `send(h)` başka bir şey
    /// dönerse.
    pub fn h_events<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xE1);
        let h = establish::<T>(&mut t, peer, None); // Ready(h, peer)

        T::require_renew(&mut t, h);
        assert_eq!(
            next_event::<T>(&mut t),
            Some(SessionEvent::RenewRequired(h))
        );

        T::complete_renew(&mut t, h);
        let h2 = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h2, p)) => {
                assert_eq!(p, peer, "Ready(h') başka peer taşıdı");
                assert_ne!(h2, h, "yenileme aynı tutamacı verdi");
                h2
            }
            other => panic!("RenewRequired'dan sonra Ready(h') bekleniyordu, gelen {other:?}"),
        };
        assert_eq!(
            next_event::<T>(&mut t),
            Some(SessionEvent::Closed(h, CloseReason::Superseded))
        );
        assert_no_event::<T>(&mut t, "Closed sonrası");

        // `Closed` Application'a ulaştı: eski tutamaç artık BİLİNMİYOR. Satırın yazılı hâli budur
        // ("`Closed` sonrası `send(h)` → `UnknownSession`"); `close()` çağrısı İSTEMEZ.
        assert_eq!(
            t.send(h, unit(b"gec"), DeliveryPolicy::Reliable),
            Err(TransportError::UnknownSession)
        );
        // Hayri'nin özgün testindeki biçim: Application `Closed`'ı işleyip eski tutamacı kapatır
        // (kaynak bırakma). Sonuç değişmez ve kapatma YENİ oturuma dokunmaz.
        t.close(h);
        assert_eq!(
            t.send(h, unit(b"gec"), DeliveryPolicy::Reliable),
            Err(TransportError::UnknownSession)
        );
        assert_eq!(t.send(h2, unit(b"yeni"), DeliveryPolicy::Reliable), Ok(()));
    }

    /// **H-CUTOVER** — §6 (yenileme kolu): cutover öncesi `send(h)` kabul; cutover sonrası `send(h)`
    /// → `SessionSuperseded`; `h`'nin outstanding'i drain olur; `h'` üzerinden sıralama korunur.
    ///
    /// KAPSAM DIŞI (satırın geri kalanı; sahte modellemiyor, gerçek hedefte yazılacak): provisional
    /// supersession (bilinen eşten yeni-sayaçlı INIT), bayat INIT'in etkisizliği, duplicate INIT,
    /// glare, davranışın Crypto cap değerinden bağımsızlığı.
    ///
    /// # Panics
    /// Yukarıdaki dört iddiadan biri tutmazsa.
    pub fn h_cutover<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xE2);
        let h = establish::<T>(&mut t, peer, None);

        // Cutover öncesi ve yenileme SÜRERKEN h kabul eder.
        assert_eq!(t.send(h, unit(b"once-1"), DeliveryPolicy::Reliable), Ok(()));
        T::require_renew(&mut t, h);
        assert_eq!(
            next_event::<T>(&mut t),
            Some(SessionEvent::RenewRequired(h))
        );
        assert_eq!(t.send(h, unit(b"once-2"), DeliveryPolicy::Reliable), Ok(()));

        // Cutover: Ready(h') geldi, Closed(h) henüz TESLİM EDİLMEDİ → h supersede edilmiş.
        T::complete_renew(&mut t, h);
        let h2 = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h2, p)) if p == peer => h2,
            other => panic!("Ready(h') bekleniyordu, gelen {other:?}"),
        };
        assert_eq!(
            t.send(h, unit(b"sonra"), DeliveryPolicy::Reliable),
            Err(TransportError::SessionSuperseded)
        );

        // h'nin outstanding'i drain olur: cutover'dan ÖNCE kabul edilen iki birim teslim edilir.
        for want in [&b"once-1"[..], &b"once-2"[..]] {
            let d = next_delivery::<T>(&mut t).expect("outstanding birim drain olmadı");
            assert_eq!((d.session, d.unit.as_slice()), (h, want));
        }

        // h' üzerinden gönderim çalışır ve sıra korunur.
        for body in [&b"yeni-1"[..], &b"yeni-2"[..], &b"yeni-3"[..]] {
            assert_eq!(t.send(h2, unit(body), DeliveryPolicy::Reliable), Ok(()));
        }
        for want in [&b"yeni-1"[..], &b"yeni-2"[..], &b"yeni-3"[..]] {
            let d = next_delivery::<T>(&mut t).expect("h' üzerinden teslim gelmedi");
            assert_eq!(
                (d.session, d.unit.as_slice()),
                (h2, want),
                "h' sırası bozuldu"
            );
        }
        assert!(
            t.recv().is_none(),
            "fazladan teslim (reddedilen birim sızdı)"
        );

        assert_eq!(
            next_event::<T>(&mut t),
            Some(SessionEvent::Closed(h, CloseReason::Superseded))
        );
    }

    /// **H-CONNECT-IDEMP** — §6: aktif peer → `Ready(h)` döner, olay YOK; pending → aynı ticket;
    /// olay kuyruğunda duplicate `Ready` yok.
    ///
    /// # Panics
    /// İkinci `connect` farklı ticket/tutamaç verirse ya da fazladan olay üretirse.
    pub fn h_connect_idemp<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xE3);

        let ticket = pending_ticket(t.connect(peer, None), "ilk connect");
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Pending(ticket)));

        let h = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h, p)) if p == peer => h,
            other => panic!("Ready bekleniyordu, gelen {other:?}"),
        };
        for _ in 0..3 {
            assert_eq!(t.connect(peer, None), Ok(ConnectResult::Ready(h)));
        }
        assert_no_event::<T>(&mut t, "idempotent connect");
    }

    /// **H-EARLY-CONFLICT** — §6: aktif/pending peer'a `connect(peer, Some(X))` →
    /// `EarlyUnitConflict`; X hiçbir yola girmez; `connect(peer, None)` idempotent.
    ///
    /// # Panics
    /// Çakışma kabul edilirse ya da reddedilen birim teslim yoluna girerse.
    pub fn h_early_conflict<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xE4);
        let conflicting = || Some(unit(b"cakisan early_unit"));

        // Pending durumunda çakışma.
        let ticket = pending_ticket(t.connect(peer, None), "ilk connect");
        assert_eq!(
            t.connect(peer, conflicting()),
            Err(TransportError::EarlyUnitConflict)
        );
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Pending(ticket)));

        // Aktif durumunda çakışma.
        let h = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h, p)) if p == peer => h,
            other => panic!("Ready bekleniyordu, gelen {other:?}"),
        };
        assert_eq!(
            t.connect(peer, conflicting()),
            Err(TransportError::EarlyUnitConflict)
        );
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Ready(h)));

        // Reddedilen birimler hiçbir yola girmedi.
        for _ in 0..PUMP_BUDGET {
            T::pump(&mut t);
            assert!(t.recv().is_none(), "reddedilen early_unit teslim edildi");
        }
    }

    /// **H-CAPS** — §6: `len == max` kabul, `len == max+1` `UnitTooLarge`; değer Transport'tan.
    ///
    /// Sınır hedefin kendi `capabilities()`'inden okunur; gövdede sayı yoktur. Satırın "app'te sabit
    /// yok" cümlesi statik bir iddiadır ve burada SINANMAZ (`kama-app`'te bugün ürün kodu yok).
    ///
    /// # Panics
    /// Sınırda ret ya da sınırın bir üstünde kabul olursa.
    pub fn h_caps<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let h = establish::<T>(&mut t, crate::harness::test_peer(0xE5), None);
        let max = t.capabilities().max_plaintext_unit_bytes as usize;

        assert_eq!(
            t.send(h, unit(&vec![0xA5; max]), DeliveryPolicy::Reliable),
            Ok(()),
            "tam sınırda kabul edilmeli"
        );
        let d = next_delivery::<T>(&mut t).expect("sınırdaki birim teslim edilmedi");
        assert_eq!(d.unit.len(), max);

        assert_eq!(
            t.send(h, unit(&vec![0xA5; max + 1]), DeliveryPolicy::Reliable),
            Err(TransportError::UnitTooLarge)
        );
        assert!(t.recv().is_none(), "reddedilen birim teslim edildi");
    }

    /// **H-COMPRESS-BOUNDARY** — §6: `CompressionPolicy` Transport davranışını değiştirmez;
    /// decompress yalnız Application'da.
    ///
    /// # Panics
    /// Aynı byte'lar iki politika altında farklı teslim edilirse.
    pub fn h_compress_boundary<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let h = establish::<T>(&mut t, crate::harness::test_peer(0xE6), None);
        let bytes = b"ayni baytlar, iki politika".to_vec();

        let mut delivered = Vec::new();
        for policy in [CompressionPolicy::Never, CompressionPolicy::AppCompressed] {
            let u = PreparedUnit {
                bytes: bytes.clone(),
                compression_policy: policy,
            };
            assert_eq!(t.send(h, u, DeliveryPolicy::Reliable), Ok(()));
            delivered.push(next_delivery::<T>(&mut t).expect("birim teslim edilmedi"));
        }
        assert_eq!(delivered[0].unit, bytes, "Never: byte'lar değişti");
        assert_eq!(
            delivered[1].unit, bytes,
            "AppCompressed: Transport byte'lara dokundu (açtı ya da sıkıştırdı)"
        );
    }

    // ── Eksik dört satır ─────────────────────────────────────────────────────────────────────

    /// **H-APP-RETRY-0** — §6: deterministik kayıp altında `recovery ON` → teslim; `recovery OFF` →
    /// kasıtlı düşürülen birim **kurtarılmaz**; Application yeniden GÖNDERMEZ.
    ///
    /// Application'ın yerinde bu test durur ve her birimi TEK kez `send` eder. Recovery açıkken
    /// teslimi sağlayan Transport'tur; kapalıyken kayıp kalıcıdır — yani "kurtarma" Application'da
    /// değil Transport'tadır. ("app kodunda retransmit yolu yok" cümlesi statik bir iddiadır;
    /// `kama-app`'te bugün mesaj mantığı bulunmadığından davranışla sınanacak bir şey yoktur.)
    ///
    /// # Panics
    /// Recovery açıkken birim teslim edilmezse ya da sıra bozulursa; kapalıyken kaybolan birim
    /// teslim edilirse.
    pub fn h_app_retry_0<T: SeamTarget>() {
        // recovery ON: kaybolan birim Transport'un içinde kurtarılır; sıra korunur.
        let mut t = fresh::<T>();
        let h = establish::<T>(&mut t, crate::harness::test_peer(0xE7), None);
        T::set_recovery(&mut t, true);
        T::drop_next_units(&mut t, 1);
        assert_eq!(
            t.send(h, unit(b"kaybolan"), DeliveryPolicy::Reliable),
            Ok(())
        );
        assert_eq!(
            t.send(h, unit(b"sonraki"), DeliveryPolicy::Reliable),
            Ok(())
        );
        for want in [&b"kaybolan"[..], &b"sonraki"[..]] {
            let d = next_delivery::<T>(&mut t).expect("recovery ON: birim teslim edilmedi");
            assert_eq!(d.unit.as_slice(), want, "recovery ON: sıra bozuldu");
        }
        assert!(
            t.recv().is_none(),
            "recovery ON: fazladan teslim (duplicate)"
        );

        // recovery OFF: aynı kayıp KURTARILMAZ; sonraki birim etkilenmez.
        let mut t = fresh::<T>();
        let h = establish::<T>(&mut t, crate::harness::test_peer(0xE8), None);
        T::set_recovery(&mut t, false);
        T::drop_next_units(&mut t, 1);
        assert_eq!(
            t.send(h, unit(b"kaybolan"), DeliveryPolicy::Reliable),
            Ok(())
        );
        assert_eq!(
            t.send(h, unit(b"sonraki"), DeliveryPolicy::Reliable),
            Ok(())
        );
        let d = next_delivery::<T>(&mut t).expect("recovery OFF: kaybolmayan birim de gelmedi");
        assert_eq!(d.unit.as_slice(), b"sonraki");
        for _ in 0..PUMP_BUDGET {
            T::pump(&mut t);
            assert!(
                t.recv().is_none(),
                "recovery OFF: düşürülen birim kurtarıldı — kurtarma Transport dışında bir yerde"
            );
        }
    }

    /// **H-CANCEL** — §6: `Pending(ticket)` → `cancel(ticket)` → `ConnectAbandoned(ticket,
    /// Cancelled)`; `early_unit` hiçbir yola girmez; ticket geçersiz, sonraki `connect(peer)` yeni
    /// ticket verir. Saat ne kadar ilerlerse ilerlesin niyet KENDİLİĞİNDEN sonlanmaz.
    ///
    /// KAPSAM DIŞI: satırın "pending kripto state kapanır" cümlesi S1 yüzeyinden gözlenemez
    /// (Application Crypto'yu görmez) ve sahtede Crypto yoktur; gerçek hedefte, Transport'un
    /// altındaki Crypto'nun pending kaydına bakılarak sınanacak iştir.
    ///
    /// # Panics
    /// Niyet zamanla sonlanırsa, iptal başka olay üretirse, `early_unit` teslim edilirse ya da
    /// iptal edilen ticket yeniden verilirse.
    pub fn h_cancel<T: SeamTarget>() {
        /// "Ne kadar ilerletilirse ilerletilsin": T-18c ephemeral ömrünün (7 gün) çok ötesi.
        const A_VERY_LONG_TIME: Duration = Duration::from_secs(90 * 24 * 60 * 60);

        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xE9);
        let ticket = pending_ticket(
            t.connect(peer, Some(unit(b"iptal edilecek early_unit"))),
            "ilk connect",
        );

        // INV-NO-IDLE-TEARDOWN: zaman niyeti sonlandırmaz. (Hedef SÜRÜLMEZ: yanıt gelmiyor.)
        T::advance_clock(&mut t, A_VERY_LONG_TIME);
        assert!(
            t.poll_event().is_none(),
            "zaman geçti diye olay çıktı (niyet kendiliğinden sonlandı?)"
        );
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Pending(ticket)));

        // Tek çıkış: cancel → tam bir olay.
        t.cancel(ticket);
        assert_eq!(
            t.poll_event(),
            Some(SessionEvent::ConnectAbandoned(
                ticket,
                AbandonReason::Cancelled
            ))
        );
        assert!(t.poll_event().is_none(), "iptal birden fazla olay üretti");
        t.cancel(ticket); // tekrar iptal: etkisiz
        assert!(t.poll_event().is_none(), "ikinci cancel olay üretti");

        // Ticket geçersiz: aynı eşe connect YENİ ticket verir; eski niyet dirilmez.
        let again = pending_ticket(t.connect(peer, None), "iptalden sonra connect");
        assert_ne!(again, ticket, "iptal edilen ticket yeniden verildi");
        match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(_, p)) => assert_eq!(p, peer),
            other => panic!("yeni niyet Ready'ye çözülmedi: {other:?}"),
        }
        // early_unit iptalle birlikte düştü: yeni oturumda teslim EDİLMEZ.
        for _ in 0..PUMP_BUDGET {
            T::pump(&mut t);
            assert!(
                t.recv().is_none(),
                "iptal edilen niyetin early_unit'i teslim edildi"
            );
        }
    }

    /// **H-PROGRESS** — §6: link yok → `ConnectProgress(Suspended)` ve 0 prob emisyonu; link gelir →
    /// `Measuring` + anında prob; `ConnectProgress` hiçbir durumda eşin canlılığını bildirmez.
    ///
    /// # Panics
    /// Link yokken prob çıkarsa, link gelince prob çıkmazsa ya da link olayları niyeti çözerse.
    pub fn h_progress<T: SeamTarget>() {
        /// Kadanstan bağımsız "uzun" süre: herhangi bir prob aralığının çok üstünde.
        const LONG: Duration = Duration::from_secs(60 * 60);

        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xEA);
        let ticket = pending_ticket(t.connect(peer, None), "ilk connect");

        // Karşı-kontrol: link AÇIKKEN saat ilerleyince prob çıkar. (Çıkmasaydı aşağıdaki "0 prob"
        // iddiası her hedefte kendiliğinden doğru olurdu.)
        let before = T::probe_emissions(&t);
        T::advance_clock(&mut t, LONG);
        assert!(
            T::probe_emissions(&t) > before,
            "link açıkken hiç prob çıkmadı — '0 prob' iddiası bu hedefte ölçülemez"
        );

        // Link düşer: Suspended; prob DURUR.
        T::set_link(&mut t, false);
        assert_eq!(
            t.poll_event(),
            Some(SessionEvent::ConnectProgress(
                ticket,
                ConnectPhase::Suspended
            ))
        );
        let suspended_at = T::probe_emissions(&t);
        T::advance_clock(&mut t, LONG);
        assert_eq!(
            T::probe_emissions(&t),
            suspended_at,
            "link yokken prob emisyonu oldu"
        );
        assert!(t.poll_event().is_none(), "link yokken fazladan olay");

        // Link gelir: Measuring + ANINDA prob (saat ilerletilmeden).
        T::set_link(&mut t, true);
        assert_eq!(
            t.poll_event(),
            Some(SessionEvent::ConnectProgress(
                ticket,
                ConnectPhase::Measuring
            ))
        );
        assert_eq!(
            T::probe_emissions(&t),
            suspended_at + 1,
            "link gelince anında prob çıkmadı"
        );

        // Bu olaylar HUB YOLUNUN durumudur: niyeti çözmezler, eş hakkında bir şey söylemezler.
        assert!(t.poll_event().is_none(), "link olayı Ready üretti");
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Pending(ticket)));
    }

    /// **H-SILENT-REJECT** — §6: sahte/geçersiz RESPONSE hiçbir olay üretmez, pending yaşar: ticket
    /// geçerli kalır, `ConnectAbandoned` çıkmaz, sonradan gelen geçerli RESPONSE aynı niyeti
    /// `Ready`'ye çözer; kripto nedeni hiçbir olayda görünmez (K9).
    ///
    /// # Panics
    /// Sahte RESPONSE bir olay üretirse, niyeti düşürürse ya da sonraki geçerli yanıt çözülmezse.
    pub fn h_silent_reject<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xEB);
        let ticket = pending_ticket(t.connect(peer, None), "ilk connect");

        for _ in 0..3 {
            T::inject_invalid_response(&mut t, peer);
            assert!(
                t.poll_event().is_none(),
                "sahte RESPONSE olay üretti (kripto nedeni Application'a sızdı)"
            );
            assert_eq!(
                t.connect(peer, None),
                Ok(ConnectResult::Pending(ticket)),
                "sahte RESPONSE niyeti düşürdü ya da ticket'ı değiştirdi"
            );
        }

        // Geçerli RESPONSE gelir: AYNI niyet Ready'ye çözülür.
        let h = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h, p)) => {
                assert_eq!(p, peer);
                h
            }
            other => panic!("geçerli yanıt niyeti çözmedi: {other:?}"),
        };
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Ready(h)));
        assert_no_event::<T>(&mut t, "çözülmeden sonra");
    }
}

/// Mekanizma gövdeleri — her hedefte AYNI kod koşar.
pub mod mechanism {
    use super::super::target::{SeamTarget, TargetUnavailable, REGISTERED_TARGETS};
    use kama_contracts::handles::SessionHandle;
    use kama_contracts::s1::KamaTransport;

    /// Hedef kaydı ile hedefin kendi beyanı tutarlı mı.
    ///
    /// # Panics
    /// Hedef `REGISTERED_TARGETS` içinde yoksa ya da ailesi (`KIND`) kayıtla uyuşmuyorsa.
    pub fn target_registered<T: SeamTarget>() {
        let hit = REGISTERED_TARGETS.iter().find(|(id, _)| *id == T::ID);
        let (_, kind) = hit.unwrap_or_else(|| {
            panic!(
                "hedef '{}' REGISTERED_TARGETS içinde kayıtlı değil — süit kayıtsız bir hedefte koşuyor",
                T::ID
            )
        });
        assert_eq!(
            *kind,
            T::KIND,
            "hedef '{}' kayıtta {:?}, kendi beyanında {:?}",
            T::ID,
            kind,
            T::KIND
        );
    }

    /// Kurulum typed'dır: ya `Ok`, ya boşluk kimliği taşıyan typed bir red. Panik/`todo!()` yok.
    ///
    /// # Panics
    /// Red boş bir boşluk kimliği taşıyorsa (beyansız boşluk = sessiz atlama).
    pub fn build_is_typed<T: SeamTarget>() {
        match T::build() {
            Ok(_) => {}
            Err(TargetUnavailable::NotImplemented { gap }) => {
                assert!(
                    !gap.is_empty(),
                    "hedef '{}' kimliksiz bir boşlukla reddetti — beyan edilmemiş boşluk",
                    T::ID
                );
            }
        }
    }

    /// Taze hedef oturum İCAT ETMEZ: `recv()`/`poll_event()` boş ve gözlem yan etkisiz.
    ///
    /// Mekanizma seviyesidir; §6 `H-EVENTS` satırının yerine GEÇMEZ (o, `Ready → RenewRequired →
    /// Ready(h') → Closed(h, Superseded)` sırasını ister).
    ///
    /// # Panics
    /// Hiç `connect` çağrılmadan teslim ya da olay üretilirse.
    pub fn fresh_target_has_no_sessions<T: SeamTarget>() {
        let Ok(mut t) = T::build() else {
            return; // boşluk beyanı build_is_typed'ın işi; burada sessizce geçilir
        };
        assert!(
            t.recv().is_none(),
            "hedef '{}' connect edilmeden teslim üretti",
            T::ID
        );
        assert!(
            t.poll_event().is_none(),
            "hedef '{}' connect edilmeden olay üretti",
            T::ID
        );
        // Gözlem yan etkisiz: ikinci çağrı da boş.
        assert!(t.recv().is_none(), "ikinci recv() farklı davrandı");
        assert!(
            t.poll_event().is_none(),
            "ikinci poll_event() farklı davrandı"
        );
        // §1: `close` hata kanalı taşımaz; bilinmeyen handle no-op olmalı, panik değil.
        t.close(SessionHandle::from_raw(u64::MAX));
    }

    /// `capabilities()` oturum GEREKTİRMEZ (K5: global, session öncesi) ve kararlıdır.
    ///
    /// `preferred ≤ max` kontrolü spec'te yazılı DEĞİLDİR; türetilmiştir: app record'larını
    /// `preferred`'a göre boyutlandırırsa (K5 S) ve `preferred > max` olsaydı ipucu doğrudan
    /// `UnitTooLarge`'a (K3 S) götürürdü.
    ///
    /// # Panics
    /// İki çağrı farklı değer döndürürse ya da `preferred > max` ise.
    pub fn capabilities_queryable_without_session<T: SeamTarget>() {
        let Ok(t) = T::build() else {
            return;
        };
        let a = t.capabilities();
        let b = t.capabilities();
        assert_eq!(a, b, "hedef '{}' kapasiteyi kararsız döndürdü", T::ID);
        assert!(
            a.preferred_plaintext_unit_bytes <= a.max_plaintext_unit_bytes,
            "hedef '{}': preferred({}) > max({}) — ipucu kullanılamaz",
            T::ID,
            a.preferred_plaintext_unit_bytes,
            a.max_plaintext_unit_bytes
        );
    }
}

/// Mekanizma süitini bir hedef için örnekler.
///
/// Çağıranın kök modülünde `mod harness;` bulunmalıdır (repo idiomu: `#[path]` ile dahil edilir).
/// Hedef yolu ÇAĞRI YERİNDEN geçer, örn. `harness::target::MockTarget`.
#[macro_export]
macro_rules! seam_mechanism_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            use super::*;
            use $crate::harness::contract::mechanism;

            type Target = $target;

            #[test]
            fn m1_target_registered() {
                mechanism::target_registered::<Target>();
            }

            #[test]
            fn m2_build_is_typed() {
                mechanism::build_is_typed::<Target>();
            }

            #[test]
            fn m3_fresh_target_has_no_sessions() {
                mechanism::fresh_target_has_no_sessions::<Target>();
            }

            #[test]
            fn m4_capabilities_queryable_without_session() {
                mechanism::capabilities_queryable_without_session::<Target>();
            }
        }
    };
}

/// Sözleşme süitini (KM-SEAM-AT-001 §6) bir hedef için örnekler.
///
/// **Bugün 12/12 davranış satırının gövdesi vardır** (iki statik satır koşum değildir; bkz. modül
/// başlığı). Yeni satır eklerken: gövde [`rows`] içine generic `fn` olarak yazılır, buraya
/// `#[test]` sarmalayıcısı eklenir (ad = satır kimliğinin küçük harfli, `-` yerine `_` konmuş hâli
/// — runner bu adı LİSTEDE arar), kimliği [`CONTRACT_ROWS`] ve `scripts/ci/seam-coverage.json`'a
/// AYNI commit'te işlenir (durum: tek hedef varken `mock_only`).
#[macro_export]
macro_rules! seam_contract_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            use super::*;
            use $crate::harness::contract;
            use $crate::harness::target::SeamTarget;

            type Target = $target;

            // ── §6 davranış satırları (spec tablosu sırasıyla) ──

            #[test]
            fn h_app_retry_0() {
                contract::rows::h_app_retry_0::<Target>();
            }

            #[test]
            fn h_roundtrip() {
                contract::rows::h_roundtrip::<Target>();
            }

            #[test]
            fn h_events() {
                contract::rows::h_events::<Target>();
            }

            #[test]
            fn h_cutover() {
                contract::rows::h_cutover::<Target>();
            }

            #[test]
            fn h_connect_idemp() {
                contract::rows::h_connect_idemp::<Target>();
            }

            #[test]
            fn h_early_conflict() {
                contract::rows::h_early_conflict::<Target>();
            }

            #[test]
            fn h_caps() {
                contract::rows::h_caps::<Target>();
            }

            #[test]
            fn h_pending_cap() {
                contract::rows::h_pending_cap::<Target>();
            }

            #[test]
            fn h_cancel() {
                contract::rows::h_cancel::<Target>();
            }

            #[test]
            fn h_progress() {
                contract::rows::h_progress::<Target>();
            }

            #[test]
            fn h_silent_reject() {
                contract::rows::h_silent_reject::<Target>();
            }

            #[test]
            fn h_compress_boundary() {
                contract::rows::h_compress_boundary::<Target>();
            }

            #[test]
            fn c0_declared_contract_rows_are_consistent() {
                contract::emit_contract_row_sentinel(<Target as SeamTarget>::ID);
            }
        }
    };
}
