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
//!   Not: §6'nın 11 satırı ortak koşum yüzeyinde (conformance-core) yaşar; `H-NO-CRYPTO-EDGE` ve
//!   `H-NO-PLANE1` spec'te STATİK kontrol olarak tanımlıdır (derleme bağımlılığı denetimi) —
//!   sırası geldiğinde koşum değil, kapı-seviyesi denetim olarak uygulanmaları gerekir.
//! * **sözleşme** ([`seam_contract_suite!`]) — §6'nın 11 satırı. **Bugün 1'i uygulanmıştır**
//!   (H-ROUNDTRIP), ve o satır `mock_only`'dir: yalnız sahtede koştuğu için kanıt değeri ikinci
//!   hedef gelince doğar. Kalan 10'u ve bu ayrım `scripts/ci/seam-coverage.json` içinde satır
//!   satır beyan edilir; runner, Rust tarafının bildirdiği listeyi (aşağıdaki sentinel) o
//!   beyanla ÇAPRAZ KONTROL eder ve tek hedef varken hiçbir satırın "kanıtlandı" sayılmasına
//!   izin vermez. Beyanla gerçeklik ayrışırsa kapı kırmızı.

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
pub const CONTRACT_ROWS: &[&str] = &["H-ROUNDTRIP", "H-PENDING-CAP"];

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
    use kama_contracts::handles::{PeerRef, SessionHandle};
    use kama_contracts::s1::{
        CompressionPolicy, ConnectResult, DeliveryPolicy, KamaTransport, PreparedUnit,
        SessionEvent, TransportError,
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
/// **Bugün 1/11 satır uygulanmıştır** (H-ROUNDTRIP; walking-skeleton'un taşıdığı ince dilim).
/// Yeni satır eklerken: gövde [`rows`] içine generic `fn` olarak yazılır, buraya `#[test]`
/// sarmalayıcısı eklenir, kimliği [`CONTRACT_ROWS`] ve `scripts/ci/seam-coverage.json`'a AYNI
/// commit'te işlenir (durum: tek hedef varken `mock_only`).
#[macro_export]
macro_rules! seam_contract_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            use super::*;
            use $crate::harness::contract;
            use $crate::harness::target::SeamTarget;

            type Target = $target;

            // ── §6 satırları ── (kalanlar seam-coverage.json'da beyanlı)

            #[test]
            fn h_roundtrip() {
                contract::rows::h_roundtrip::<Target>();
            }

            #[test]
            fn h_pending_cap() {
                contract::rows::h_pending_cap::<Target>();
            }

            #[test]
            fn c0_declared_contract_rows_are_consistent() {
                contract::emit_contract_row_sentinel(<Target as SeamTarget>::ID);
            }
        }
    };
}
