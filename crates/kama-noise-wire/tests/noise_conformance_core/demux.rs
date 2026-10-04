//! H-DEMUX (A3.1) — KM-SEAM-TC-001 §6 / K7; sayı: ccr §35.15 KS-DOS-1.
//!
//! RESPONSE'un hangi bekleyen INIT'e ait olduğu wire'da yazmaz. Crypto pending adaylarını EN YENİ
//! ÖNCE dener; deneme sayısı `PENDING_RESPONSE_TRIAL_CAP` ile sınırlıdır; yanlış aday ile bozuk
//! paket dışarıdan ayırt edilemez.
//!
//! Gövdeler hedef-agnostiktir. Sahteye karşı geçmeleri yalnız SAHTENİN sözleşmeye uyduğunu
//! gösterir; kanıt değeri A5.5'te aynı gövdeler gerçek koda koşunca doğar.
//!
//! "Konum" = deneme sırasındaki yer (1 = en yeni pending).

use client_core::seam::{SessionCommand, SessionControl, SessionResult};

use crate::target::S3Target;

/// Pending etiketleri bu tabandan başlar (eski → yeni artan).
const TAG_BASE: u64 = 0x5000;

fn tag_at(i: usize) -> u64 {
    TAG_BASE + i as u64
}

/// `n` pending kaydeder (etiketler `tag_at(0)` en eski … `tag_at(n-1)` en yeni).
fn fill<T: S3Target>(c: &mut T::Crypto, n: usize) {
    for i in 0..n {
        T::add_pending(c, tag_at(i)).expect("tavanın altında pending kabul edilmeli");
    }
}

fn accept<T: S3Target>(c: &mut T::Crypto, wire: Vec<u8>) -> SessionResult {
    c.command(SessionCommand::AcceptResponse { wire })
}

/// Tavan kadar pending varken deneme sırasında `position`'daki aday için geçerli RESPONSE.
fn established_at<T: S3Target>(position: usize) {
    let cap = T::TRIAL_CAP;
    let mut c = T::build().expect("hedef kayıtlı olmalı");
    fill::<T>(&mut c, cap);
    let r = accept::<T>(&mut c, T::response_for(tag_at(cap - position)));
    assert!(
        matches!(r, SessionResult::Established { .. }),
        "konum {position}/{cap}: Established bekleniyordu, gelen {r:?}"
    );
    assert_eq!(T::last_trials(&c), position, "deneme sayısı = konum");
    assert_eq!(T::pending_len(&c), cap - 1, "eşleşen pending tüketilir");
}

/// 1 — doğru aday ilk konumda.
pub fn correct_candidate_first<T: S3Target>() {
    established_at::<T>(1);
}

/// 2 — doğru aday orta konumda.
pub fn correct_candidate_middle<T: S3Target>() {
    established_at::<T>(T::TRIAL_CAP.div_ceil(2));
}

/// 3 — doğru aday son (tavanıncı) konumda.
pub fn correct_candidate_last<T: S3Target>() {
    established_at::<T>(T::TRIAL_CAP);
}

/// 4 — tavanın bir ötesindeki aday → `Failed`.
///
/// ccr §35.15 zinciri (`PENDING_RESPONSE_TRIAL_CAP` = istemci pending INIT tavanı) gereği tavan+1
/// pending aynı anda VAR OLAMAZ: fazladan INIT kümeye alınmaz. Bu yüzden satır, o adayın kümeye
/// girmediğini ve RESPONSE'unun tavan kadar denemeden sonra `Failed` döndüğünü sınar
/// (sahip onayı 2026-10-06). Transport bu reddi Application'a `TransportError::Backpressure`
/// olarak yansıtır (KM-SEAM-AT-001 §3); o eşleme S1 tarafının işidir, bu süit S3 yüzeyini sınar.
pub fn candidate_beyond_cap_fails<T: S3Target>() {
    let cap = T::TRIAL_CAP;
    let mut c = T::build().expect("hedef kayıtlı olmalı");
    fill::<T>(&mut c, cap);
    let before = T::fingerprint(&c);
    assert!(
        T::add_pending(&mut c, tag_at(cap)).is_err(),
        "tavan+1'inci pending reddedilmeli"
    );
    assert_eq!(T::fingerprint(&c), before, "reddedilen kayıt iz bırakmaz");

    let r = accept::<T>(&mut c, T::response_for(tag_at(cap)));
    assert_eq!(r, SessionResult::Failed);
    assert_eq!(T::last_trials(&c), cap, "deneme tavanda durur");
    assert_eq!(T::pending_len(&c), cap, "pending kümesi değişmez");
    assert!(c.poll_event().is_none(), "başarısızlık olay üretmez");
}

/// 5 — yanlış aday ile bozuk paket: aynı sonuç, aynı yan etki.
pub fn wrong_candidate_indistinguishable_from_corrupt<T: S3Target>() {
    let n = T::TRIAL_CAP.div_ceil(2);
    let mut c = T::build().expect("hedef kayıtlı olmalı");
    fill::<T>(&mut c, n);

    // Hiç kaydedilmemiş bir INIT'in geçerli biçimli RESPONSE'u = yanlış aday.
    let wrong = T::response_for(tag_at(T::TRIAL_CAP + 1));
    // Kayıtlı bir INIT'in RESPONSE'unun doğrulama alanı bozulmuş kopyası.
    let corrupt = T::corrupt(&T::response_for(tag_at(0)));

    let r_wrong = accept::<T>(&mut c, wrong.clone());
    let after_wrong = T::fingerprint(&c);
    let r_corrupt = accept::<T>(&mut c, corrupt);
    let after_corrupt = T::fingerprint(&c);
    let r_again = accept::<T>(&mut c, wrong);
    let after_again = T::fingerprint(&c);

    assert_eq!(r_wrong, SessionResult::Failed);
    assert_eq!(r_corrupt, r_wrong, "hata varyantı aynı");
    assert_eq!(r_again, r_wrong);
    assert_eq!(after_corrupt, after_wrong, "state parmak izi byte-özdeş");
    assert_eq!(after_again, after_wrong);
    assert_eq!(T::pending_len(&c), n, "pending kümesi değişmez");
    assert!(c.poll_event().is_none(), "başarısızlık olay üretmez");

    // Başarısızlıklar hiçbir adayı tüketmedi: kayıtlı adayın geçerli RESPONSE'u hâlâ kabul edilir.
    let ok = accept::<T>(&mut c, T::response_for(tag_at(0)));
    assert!(matches!(ok, SessionResult::Established { .. }));
}

/// 6 — deneme sırası en-yeni-önce (deneme sayacıyla gözlenir).
pub fn trial_order_newest_first<T: S3Target>() {
    let n = T::TRIAL_CAP.div_ceil(2);
    for position in 1..=n {
        let mut c = T::build().expect("hedef kayıtlı olmalı");
        fill::<T>(&mut c, n);
        let r = accept::<T>(&mut c, T::response_for(tag_at(n - position)));
        assert!(matches!(r, SessionResult::Established { .. }));
        assert_eq!(
            T::last_trials(&c),
            position,
            "en yeniden {position}. aday {position} denemede bulunmalı"
        );
    }
}

/// 7 — tavan adlandırılmış sabitten gelir, spec'teki değere eşittir, bu süitte literal olarak geçmez.
pub fn cap_is_named_and_matches_spec<T: S3Target>() {
    const NAME: &str = "PENDING_RESPONSE_TRIAL_CAP = ";
    let spec = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../specs/ccr-format-spec.md"
    ))
    .expect("ccr-format-spec.md okunmalı");
    // Üstü çizili (eski) satırlar sayılmaz; sayıyla başlayan her atama aynı değeri vermeli.
    let values: Vec<usize> = spec
        .lines()
        .filter(|l| !l.contains("~~"))
        .flat_map(|l| {
            l.match_indices(NAME)
                .map(move |(i, _)| &l[i + NAME.len()..])
        })
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().ok()
        })
        .collect();
    assert!(
        !values.is_empty(),
        "spec'te sayısal tavan ataması bulunamadı"
    );
    assert!(
        values.iter().all(|v| *v == T::TRIAL_CAP),
        "spec {values:?} ≠ hedef sabiti {}",
        T::TRIAL_CAP
    );

    // Literal denetimi: tavanın ondalık yazımı bu süitin kaynaklarında bağımsız belirteç olarak geçmez.
    let needle = T::TRIAL_CAP.to_string();
    for (name, src) in [
        ("demux.rs", include_str!("demux.rs")),
        ("target.rs", include_str!("target.rs")),
        ("main.rs", include_str!("main.rs")),
    ] {
        let hit = src
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|tok| tok == needle);
        assert!(!hit, "{name}: tavan değeri literal olarak yazılmış");
    }
}

/// Satır gövdelerini hedef başına ayrı `#[test]` olarak sarar.
#[macro_export]
macro_rules! noise_demux_suite {
    ($suite:ident, $target:ty) => {
        mod $suite {
            type Target = $target;

            #[test]
            fn h_demux_1_correct_candidate_first() {
                $crate::demux::correct_candidate_first::<Target>();
            }
            #[test]
            fn h_demux_2_correct_candidate_middle() {
                $crate::demux::correct_candidate_middle::<Target>();
            }
            #[test]
            fn h_demux_3_correct_candidate_last() {
                $crate::demux::correct_candidate_last::<Target>();
            }
            #[test]
            fn h_demux_4_candidate_beyond_cap_fails() {
                $crate::demux::candidate_beyond_cap_fails::<Target>();
            }
            #[test]
            fn h_demux_5_wrong_candidate_indistinguishable_from_corrupt() {
                $crate::demux::wrong_candidate_indistinguishable_from_corrupt::<Target>();
            }
            #[test]
            fn h_demux_6_trial_order_newest_first() {
                $crate::demux::trial_order_newest_first::<Target>();
            }
            #[test]
            fn h_demux_7_cap_is_named_and_matches_spec() {
                $crate::demux::cap_is_named_and_matches_spec::<Target>();
            }
        }
    };
}
