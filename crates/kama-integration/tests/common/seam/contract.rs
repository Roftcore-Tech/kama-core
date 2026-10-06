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
//!
//! # Yeni niyetin İLK olayı (S1 §4, F0.2(ii) — 2026-10-06'da KAPANDI)
//!
//! `connect()` `Pending(ticket)` döndüğü anda `ConnectProgress(ticket, Measuring)` kuyruğa girer;
//! faz değişince yeni olay; aynı faz tekrar yayınlanmaz. Gövdeler bu olayı YARDIMCIDA ATLAMAZ:
//! `establish` ve her satır gövdesi ilk olayı AÇIKÇA okur ve `Measuring` olduğunu iddia eder
//! (sessiz atlama kanıtı zayıflatırdı). Reddedilen `connect` (`Backpressure`, `UnitTooLarge`,
//! `EarlyUnitConflict`) `Pending` dönmediği için olay üretmez; idempotent `connect` de üretmez
//! (`Measuring` yalnız ilkinde). `Suspended`/`LongOutage`'ın gerçek tetikleri bu sprintte KAPSAM
//! DIŞIDIR (PARK P-4); sahte `Suspended`'ı link enjeksiyonuyla modellemeye devam eder (H-PROGRESS).

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
    /// `Pending` dönerse ilk olay AÇIKÇA okunur ve `ConnectProgress(ticket, Measuring)` olmak
    /// zorundadır (F0.2(ii)); yardımcı onu atlamaz.
    ///
    /// # Panics
    /// İlk olay `Measuring` değilse, bütçe içinde `Ready` gelmezse ya da olay başka peer taşırsa.
    fn establish<T: SeamTarget>(
        t: &mut T::Transport,
        peer: PeerRef,
        early: Option<PreparedUnit>,
    ) -> SessionHandle {
        match t.connect(peer, early).expect("connect reddedilmemeli") {
            ConnectResult::Ready(h) => h,
            ConnectResult::Pending(ticket) => {
                expect_measuring::<T>(t, ticket, "establish");
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
    ///
    /// İki evre. (1) Tavan ve ret iddiaları: hedef SÜRÜLMEZ; yalnız her kabul edilen niyetin ilk
    /// olayı (`ConnectProgress(ticket, Measuring)`, F0.2(ii)) okunur — satır hedefin `connect`'leri
    /// PENDING tutabilmesini ister (karşı uç bu evrede yanıtsız). Reddedilen `connect`
    /// (`Backpressure`) `Pending` dönmediği için olay ÜRETMEZ. (2) Yerin açılması: iptal olayı
    /// okunur, sonra hedef sürülür ve bekleyen her niyetin `Ready`'ye çözülmesi beklenir (karşı uç
    /// bu evrede yanıt verir).
    ///
    /// # Panics
    /// Tavanın altında `connect` reddedilirse, tavanda kabul edilirse, ret bir state bırakırsa,
    /// hedefin tavanı spec'teki değerden farklıysa; kabul edilen bir niyetin ilk olayı `Measuring`
    /// değilse, iptalden önce başka bir olay çıkmışsa (reddedilen connect olay üretmişse), yer
    /// açıldığı hâlde (iptalle ya da `Ready` ile) önceden reddedilen eş kabul edilmezse, çözülme
    /// sırasında `Ready` dışında olay gelirse ya da reddedilen `early_unit` sonradan teslim edilirse.
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

        // Tavan kadar niyet açılır ve PENDING kalır. Her kabul edilen niyetin ilk olayı Measuring
        // (hedef sürülmeden, kuyruktan doğrudan).
        let mut tickets = Vec::with_capacity(cap);
        for i in 0..cap {
            match t.connect(peer(i), None) {
                Ok(ConnectResult::Pending(ticket)) => tickets.push(ticket),
                other => panic!("tavanın altında {i}. connect Pending vermedi: {other:?}"),
            }
            expect_measuring::<T>(&mut t, tickets[i], "tavanın altındaki connect");
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
        // Kabul edilen niyetlerin Measuring'inden beri hiçbir olay okunmadı; sıradaki olay iptalin
        // kendisidir — reddedilen connect'ler olay ÜRETMEMİŞTİR.
        t.cancel(tickets[0]);
        assert_eq!(
            t.poll_event(),
            Some(SessionEvent::ConnectAbandoned(
                tickets[0],
                AbandonReason::Cancelled
            )),
            "iptalden önce başka bir olay çıkmış (reddedilen connect olay üretti?)"
        );
        let extra_ticket = pending_ticket(t.connect(extra, None), "yer açıldıktan sonra connect");
        expect_measuring::<T>(&mut t, extra_ticket, "yer açılınca kabul edilen niyet");

        // `Ready` kolu: tavan yeniden dolu (yeni bir eş reddedilir); bekleyen niyetler `Ready`'ye
        // çözülünce yer açılır ve o eş kabul edilir.
        let late = peer(cap + 1);
        assert_eq!(t.connect(late, None), Err(TransportError::Backpressure));
        for resolved in 0..cap {
            match next_event::<T>(&mut t) {
                Some(SessionEvent::Ready(..)) => {}
                other => panic!("Ready bekleniyordu ({resolved}/{cap} çözüldü), gelen {other:?}"),
            }
        }
        // Kabul edilen niyetlerin hiçbiri early_unit taşımıyordu: çözülen oturumlarda teslim
        // edilecek bir şey YOKTUR. Tavanda reddedilen early_unit bir yere saklandıysa burada çıkar.
        for _ in 0..PUMP_BUDGET {
            T::pump(&mut t);
            assert!(
                t.recv().is_none(),
                "tavanda reddedilen early_unit sonradan teslim edildi"
            );
        }
        let late_ticket = pending_ticket(
            t.connect(late, None),
            "niyetler Ready'ye çözüldükten sonra connect hâlâ reddediliyor",
        );
        expect_measuring::<T>(&mut t, late_ticket, "Ready ile açılan yere gelen niyet");
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

    /// F0.2(ii): `Pending(ticket)` döndükten HEMEN sonra ilk olay `ConnectProgress(ticket,
    /// Measuring)` olmalıdır. Hedef SÜRÜLMEZ (sürülse niyet çözülebilirdi); olay kuyruktan
    /// doğrudan okunur — sessizce atlanmaz.
    fn expect_measuring<T: SeamTarget>(t: &mut T::Transport, ticket: ConnectTicket, what: &str) {
        assert_eq!(
            t.poll_event(),
            Some(SessionEvent::ConnectProgress(
                ticket,
                ConnectPhase::Measuring
            )),
            "{what}: Pending döndüğünde ilk olay ConnectProgress(ticket, Measuring) olmalı"
        );
    }

    // ── Hayri'nin yedi satırı (ported from feature/app; güncel S1'e göre) ────────────────────

    /// **H-EVENTS** — §6: `Ready → RenewRequired → Ready(h') → Closed(h, Superseded)` sırası;
    /// `Closed` sonrası `send(h)` → `UnknownSession`.
    ///
    /// "`Closed` sonrası" = `Closed` olayının `poll_event` ile OKUNMASINDAN sonrası (S1 §3
    /// F0.2(i), 2026-10-06; K7 aynı dili kullanır). `Ready(h')` ile `Closed` arasındaki pencere
    /// H-CUTOVER'ındır (`SessionSuperseded`). Application'ın `close(h)` çağırması GEREKMEZ;
    /// çağırırsa (`Closed` okunmuş tutamaç) olay üretmez. Hayri'nin özgün testi `UnknownSession`'ı
    /// `close(h)` çağrısından sonra bekliyordu; gövde iki biçimi de sınar.
    ///
    /// F0.2(iii): aktif `h'`ye `close(h')` tam BİR `Closed(h', UserClosed)` üretir; sonra
    /// `send(h')` → `UnknownSession`; ikinci `close(h')` olay üretmez — hem `Closed` KUYRUKTAYKEN
    /// (art arda iki `close`, arada okuma yok) hem `Closed` OKUNDUKTAN sonra.
    ///
    /// # Panics
    /// Olay sırası farklıysa, fazladan olay çıkarsa, `Closed` sonrası `send(h)` başka bir şey
    /// dönerse, `close` olay üretmesi gereken yerde üretmez ya da üretmemesi gerekirken üretirse.
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
        // (kaynak bırakma). Sonuç değişmez, kapatma YENİ oturuma dokunmaz ve `Closed`'ı okunmuş
        // tutamaca `close(h)` olay ÜRETMEZ (F0.2(iii): her oturum sonu tam BİR Closed).
        t.close(h);
        assert_no_event::<T>(&mut t, "Closed'ı okunmuş tutamaca close(h)");
        assert_eq!(
            t.send(h, unit(b"gec"), DeliveryPolicy::Reliable),
            Err(TransportError::UnknownSession)
        );
        assert_eq!(t.send(h2, unit(b"yeni"), DeliveryPolicy::Reliable), Ok(()));

        // F0.2(iii): aktif tutamacı kapatmak tam BİR `Closed(h', UserClosed)` üretir; sonra
        // tutamaç bilinmez; ikinci `close` olay üretmez. İki kol: (a) `Closed` KUYRUKTAYKEN (henüz
        // okunmadan) ikinci `close(h')` → yine tam BİR olay; (b) `Closed` OKUNDUKTAN sonra
        // `close(h')` → olay yok.
        t.close(h2);
        t.close(h2); // (a) Closed kuyrukta, okunmadı
        assert_eq!(
            next_event::<T>(&mut t),
            Some(SessionEvent::Closed(h2, CloseReason::UserClosed))
        );
        assert_no_event::<T>(&mut t, "Closed kuyruktayken ikinci close(h')");
        assert_eq!(
            t.send(h2, unit(b"gec"), DeliveryPolicy::Reliable),
            Err(TransportError::UnknownSession)
        );
        t.close(h2); // (b) Closed okundu
        assert_no_event::<T>(&mut t, "Closed'ı okunmuş tutamaca ikinci close(h')");
    }

    /// **H-CUTOVER** — §6, İKİ vaka tek gövdede (test adı `h_cutover` değişmez; runner adı arar).
    ///
    /// Vaka 1 (yenileme kolu): cutover öncesi `send(h)` kabul; cutover sonrası `send(h)` →
    /// `SessionSuperseded`; `h`'nin outstanding'i drain olur; `h'` üzerinden sıralama korunur.
    ///
    /// Vaka 2 (peer supersession, F0.4 — 2026-10-06): bilinen eşten ONAYLANMIŞ yeni INIT —
    /// `RenewRequired` OLMADAN `Ready(h')` → `Closed(h, Superseded)`; `Ready(h')` anından itibaren
    /// `send(h)` → `SessionSuperseded`; `h`'nin outstanding'i drain olur; `h'` çalışır; `Closed(h)`
    /// okunduktan sonra `send(h)` → `UnknownSession` (F0.2(i) eski tutamaç semantiği).
    ///
    /// KAPSAM DIŞI (S3 real hedefte, L-A5c; sahte onay anından ÖNCESİNİ modellemiyor): provisional
    /// onay-öncesi olaysızlık (`send(h)` kabul, olay yok), N_g/T_g yalnız-alım drain, bayat INIT'in
    /// etkisizliği, duplicate INIT, glare → tek `Ready`, davranışın Crypto cap değerinden
    /// bağımsızlığı.
    ///
    /// İki vakada da `Closed(h, Superseded)` okunduktan sonra kuyruk BOŞTUR (geç `RenewRequired`,
    /// ikinci `Closed`, `ConnectProgress` yok).
    ///
    /// # Panics
    /// İki vakanın iddialarından biri tutmazsa (vaka 2'de `Ready(h')`'den önce `RenewRequired` ya
    /// da başka bir olay çıkarsa, ya da `Closed` sonrası olay çıkarsa dahil).
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
        // 'once-2' cutover anında gerçekten YOLDA olsun: ilk iletimi kaybolur, Transport onu
        // kurtaracaktır. Böylece "outstanding drain" yalnız teslim kuyruğunda duran bir birimle
        // değil, Transport'un içinde bekleyen bir birimle de ölçülür.
        T::set_recovery(&mut t, true);
        T::drop_next_units(&mut t, 1);
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
        assert_no_event::<T>(&mut t, "vaka 1: Closed sonrası");

        // ── Vaka 2: peer supersession — RenewRequired OLMADAN (F0.4) ──
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xED);
        let h = establish::<T>(&mut t, peer, None);
        assert_eq!(t.send(h, unit(b"pre"), DeliveryPolicy::Reliable), Ok(()));

        // Onay: ilk olay doğrudan Ready(h') — öncesinde RenewRequired YOK.
        T::inject_peer_supersession(&mut t, h);
        let h2 = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h2, p)) if p == peer => h2,
            other => panic!("RenewRequired OLMADAN Ready(h') bekleniyordu, gelen {other:?}"),
        };
        assert_ne!(h2, h, "peer supersession aynı tutamacı verdi");
        assert_eq!(
            t.send(h, unit(b"sonra"), DeliveryPolicy::Reliable),
            Err(TransportError::SessionSuperseded)
        );

        // h'nin outstanding'i drain olur; h' çalışır; reddedilen birim sızmaz.
        let d = next_delivery::<T>(&mut t).expect("outstanding birim drain olmadı (vaka 2)");
        assert_eq!((d.session, d.unit.as_slice()), (h, &b"pre"[..]));
        assert_eq!(t.send(h2, unit(b"yeni"), DeliveryPolicy::Reliable), Ok(()));
        let d = next_delivery::<T>(&mut t).expect("h' üzerinden teslim gelmedi (vaka 2)");
        assert_eq!((d.session, d.unit.as_slice()), (h2, &b"yeni"[..]));
        assert!(
            t.recv().is_none(),
            "fazladan teslim (vaka 2: reddedilen birim sızdı)"
        );

        // Closed(h, Superseded) okunur; okunduktan sonra eski tutamaç BİLİNMEZ (F0.2(i)).
        assert_eq!(
            next_event::<T>(&mut t),
            Some(SessionEvent::Closed(h, CloseReason::Superseded))
        );
        assert_eq!(
            t.send(h, unit(b"gec"), DeliveryPolicy::Reliable),
            Err(TransportError::UnknownSession)
        );
        assert_eq!(
            t.send(h2, unit(b"yeni-2"), DeliveryPolicy::Reliable),
            Ok(())
        );
        // Closed sonrası kuyruk BOŞ: geç RenewRequired(h), ikinci Closed ya da ConnectProgress yok.
        assert_no_event::<T>(&mut t, "vaka 2: Closed sonrası");
    }

    /// **H-CONNECT-IDEMP** — §6: aktif peer → `Ready(h)` döner, olay YOK; pending → aynı ticket;
    /// olay kuyruğunda duplicate `Ready` yok. F0.2(ii): `Measuring` yalnız ilk `connect`'te;
    /// pending'e ikinci `connect` YENİ olay üretmez.
    ///
    /// # Panics
    /// İkinci `connect` farklı ticket/tutamaç verirse ya da fazladan olay üretirse.
    pub fn h_connect_idemp<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xE3);

        let ticket = pending_ticket(t.connect(peer, None), "ilk connect");
        expect_measuring::<T>(&mut t, ticket, "ilk connect");
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Pending(ticket)));
        // Hedef SÜRÜLMEZ: pending'e ikinci connect'in olay üretmediği kuyruktan doğrudan okunur.
        assert!(
            t.poll_event().is_none(),
            "pending'e ikinci connect olay üretti (Measuring yalnız ilkinde olmalı)"
        );

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
        expect_measuring::<T>(&mut t, ticket, "ilk connect");
        assert_eq!(
            t.connect(peer, conflicting()),
            Err(TransportError::EarlyUnitConflict)
        );
        assert_eq!(t.connect(peer, None), Ok(ConnectResult::Pending(ticket)));
        // Reddedilen ve idempotent connect'ler olay üretmez (hedef sürülmeden okunur).
        assert!(
            t.poll_event().is_none(),
            "reddedilen ya da idempotent connect olay üretti"
        );

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
    /// F0.2(iv) (2026-10-06): `early_unit` sınırı AYRI alandır (`max_early_unit_bytes`, K6):
    /// `len == max_early` kabul ve `Ready` sonrası teslim; `len == max_early+1` → `connect()`
    /// `UnitTooLarge`, olay yok ve pending OLUŞMAZ — doğrudan kanıt: aynı eşe sonraki
    /// `connect(peer, Some(küçük))` `Pending` verir (pending sızmış olsaydı `EarlyUnitConflict`
    /// dönerdi); niyet `Ready`'ye sürülür, tam BİR teslim (küçük birim) gelir, büyük birim
    /// `PUMP_BUDGET` boyunca ASLA gelmez. `reliable` `capabilities()`'ten okunur ve hedefin
    /// adlandırılmış sabitine (`T::RELIABLE`) eşittir.
    ///
    /// # Panics
    /// Sınırda ret ya da sınırın bir üstünde kabul olursa (iki sınır için de); reddedilen
    /// `early_unit` pending bırakır ya da teslim edilirse; `reliable` hedefin beyanından farklıysa.
    pub fn h_caps<T: SeamTarget>() {
        let mut t = fresh::<T>();
        let h = establish::<T>(&mut t, crate::harness::test_peer(0xE5), None);
        let caps = t.capabilities();
        let max = caps.max_plaintext_unit_bytes as usize;

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

        // early_unit sınırı: tam max_early kabul (Ready sonrası teslim).
        let max_early = caps.max_early_unit_bytes as usize;
        let h_early = establish::<T>(
            &mut t,
            crate::harness::test_peer(0xEE),
            Some(unit(&vec![0x5A; max_early])),
        );
        let d = next_delivery::<T>(&mut t).expect("sınırdaki early_unit teslim edilmedi");
        assert_eq!((d.session, d.unit.len()), (h_early, max_early));

        // max_early+1 → connect() UnitTooLarge; pending OLUŞMAZ; birim hiçbir yola girmez.
        let peer_big = crate::harness::test_peer(0xEF);
        assert_eq!(
            t.connect(peer_big, Some(unit(&vec![0x5A; max_early + 1]))),
            Err(TransportError::UnitTooLarge)
        );
        assert!(t.poll_event().is_none(), "reddedilen connect olay üretti");
        // Doğrudan kanıt: aynı eşe early_unit'Lİ connect Pending verir — pending sızmış olsaydı
        // `EarlyUnitConflict` dönerdi.
        let ticket = pending_ticket(
            t.connect(peer_big, Some(unit(b"kucuk"))),
            "reddedilen early_unit pending bıraktı (yeni Pending bekleniyordu)",
        );
        expect_measuring::<T>(&mut t, ticket, "reddedilen early_unit'ten sonra connect");
        // Niyet Ready'ye sürülür: tam BİR teslim (küçük birim) gelir; büyük birim ASLA gelmez.
        let h_big = match next_event::<T>(&mut t) {
            Some(SessionEvent::Ready(h, p)) if p == peer_big => h,
            other => panic!("reddedilen early_unit'ten sonra Ready bekleniyordu, gelen {other:?}"),
        };
        let d = next_delivery::<T>(&mut t).expect("kabul edilen küçük early_unit teslim edilmedi");
        assert_eq!((d.session, d.unit.as_slice()), (h_big, &b"kucuk"[..]));
        for _ in 0..PUMP_BUDGET {
            T::pump(&mut t);
            assert!(
                t.recv().is_none(),
                "reddedilen (max_early+1) early_unit teslim edildi"
            );
        }

        // reliable: değer Transport'tan; hedefin adlandırılmış sabitine eşit.
        assert_eq!(
            caps.reliable,
            T::RELIABLE,
            "hedef '{}' reliable beyanı sabitinden farklı",
            T::ID
        );
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
    /// teslim edilirse ya da kayıpsız (başka) oturumun birimi gelmezse.
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

        // recovery OFF: aynı kayıp KURTARILMAZ.
        //
        // Karşı-kontrol (hedef hiçbir şey teslim etmiyor olsaydı "düşürülen birim gelmedi" iddiası
        // kendiliğinden doğru çıkardı) BAŞKA bir oturumda gönderilir. Aynı oturumda, kalıcı bir
        // `Reliable` boşluğunun ARKASINDAKİ birimin teslim edilip edilmeyeceğini S1 söylemez (§4
        // yalnız sıra garantisi verir); başka oturumun akışında boşluk yoktur.
        let mut t = fresh::<T>();
        let h = establish::<T>(&mut t, crate::harness::test_peer(0xE8), None);
        let h_control = establish::<T>(&mut t, crate::harness::test_peer(0xEC), None);
        T::set_recovery(&mut t, false);
        T::drop_next_units(&mut t, 1);
        assert_eq!(
            t.send(h, unit(b"kaybolan"), DeliveryPolicy::Reliable),
            Ok(())
        );
        assert_eq!(
            t.send(h_control, unit(b"kontrol"), DeliveryPolicy::Reliable),
            Ok(())
        );
        let d = next_delivery::<T>(&mut t)
            .expect("recovery OFF: kayıpsız oturumun birimi de gelmedi (hedef teslim etmiyor)");
        assert_eq!((d.session, d.unit.as_slice()), (h_control, &b"kontrol"[..]));
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
        expect_measuring::<T>(&mut t, ticket, "ilk connect");

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
        expect_measuring::<T>(&mut t, again, "iptalden sonra connect");
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

    /// **H-PROGRESS** — §6: `Pending` döndüğünde ilk olay `ConnectProgress(ticket, Measuring)`
    /// (F0.2(ii)); link yok → `ConnectProgress(Suspended)` ve 0 prob emisyonu; link gelir →
    /// `Measuring` + anında prob; `ConnectProgress` hiçbir durumda eşin canlılığını bildirmez.
    ///
    /// `Suspended`'ın gerçek tetiği bu sprintte KAPSAM DIŞIDIR (PARK P-4); sahte onu link
    /// enjeksiyonuyla modeller — satır bu yüzden gerçek hedefte link kancasına bağlıdır.
    ///
    /// # Panics
    /// İlk olay `Measuring` değilse, link yokken prob çıkarsa, link gelince prob çıkmazsa ya da
    /// link olayları niyeti çözerse.
    pub fn h_progress<T: SeamTarget>() {
        /// Kadanstan bağımsız "uzun" süre: herhangi bir prob aralığının çok üstünde.
        const LONG: Duration = Duration::from_secs(60 * 60);

        let mut t = fresh::<T>();
        let peer = crate::harness::test_peer(0xEA);
        let ticket = pending_ticket(t.connect(peer, None), "ilk connect");
        expect_measuring::<T>(&mut t, ticket, "ilk connect");

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
        expect_measuring::<T>(&mut t, ticket, "ilk connect");

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
        // §1: `close` hata kanalı taşımaz; bilinmeyen handle no-op olmalı, panik değil ve
        // (F0.2(iii)) olay ÜRETMEZ.
        t.close(SessionHandle::from_raw(u64::MAX));
        assert!(
            t.poll_event().is_none(),
            "hedef '{}' bilinmeyen tutamaca close() olay üretti",
            T::ID
        );
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
