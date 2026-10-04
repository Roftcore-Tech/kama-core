//! MUTATION-GÜDÜMLÜ SINIR TESTLERİ — `pn.rs` (anti-replay penceresi).
//!
//! **Kaynak:** `cargo mutants -p kama-noise-wire --file crates/kama-noise-wire/src/pn.rs`
//! [ÖLÇÜLDÜ] 87 mutant · 72 caught · **13 missed** · 1 unviable · 1 timeout → skor %84,7.
//!
//! Kaçan 13 mutantın tamamı SINIR koşullarındaydı. Mevcut testler (`sprint1_core::pn_*`,
//! `pn_proptest`) tipik değerleri sürüyor; eşitlik-anındaki karar noktalarını sürmüyor.
//! Bu dosya her kaçan mutantı öldürmeyi hedefler ve hangi mutantı hedeflediğini satır
//! numarasıyla kaydeder — mutasyon skoru düşerse hangi testin zayıfladığı izlenebilir olsun.
//!
//! **Neden bu dosya `pn.rs` için ayrıca yazıldı:** anti-replay penceresi §4'ün güvenlik
//! sınırıdır. `precheck`'teki her karşılaştırma operatörü bir kabul/red kararıdır; bir
//! off-by-one ya meşru paketi düşürür (kullanılabilirlik) ya da replay'i kabul eder (güvenlik).
//! Mutation testing bu iki sınıfı ayırt etmeden ölçer; testlerin ikisini de kapatması gerekir.
//!
//! ---
//! **[ÖLÇÜLDÜ — bu testler eklendikten SONRA] SONUÇ: 13 missed → 2 missed.**
//! `87 mutants tested in 5m: 2 missed, 83 caught, 1 unviable, 1 timeouts`
//!
//! **Kalan 2 mutant EŞDEĞERDİR — onlar için test YAZILMAMALIDIR.** Eşdeğer mutant, kodu
//! değiştiren ama DAVRANIŞI değiştirmeyen mutasyondur; hiçbir test onu öldüremez çünkü
//! öldürülecek bir hata yoktur. Skor hesabında paydadan düşülür.
//!
//! Üçü de `pn.rs`'e elle uygulanıp TÜM test paketi koşturularak doğrulandı; hiçbiri hiçbir
//! testi düşürmedi:
//!
//!   `50:25`  `base | low_bits` → `base ^ low_bits`
//!            `base = center & !(span-1)` düşük bitleri sıfırlar; `low_bits < span` olduğu
//!            sürece `|` ile `^` ÖZDEŞTİR. Üretimdeki iki çağıran da bunu sağlar
//!            (`open()` u32 pn_trunc, `legacy::open()` u16 seq16).
//!
//!   `142:42` `candidate - ew + 1` → `candidate - ew`
//!            `advance` YALNIZ dal seçiminde kullanılır. Mutasyonun dalı değiştirdiği tek
//!            nokta `advance == window` sınırıdır; orada döngü aralığı (`ew..=candidate`,
//!            tam `window` ardışık değer) ring'in TÜM yuvalarını kapsar → full-clear ile
//!            davranışsal olarak ÖZDEŞ. Fark yalnız performanstır (16384 iterasyon vs
//!            birkaç word yazımı).
//!            NOT: Aşağıdaki `advance_arithmetic_selects_the_right_clear_strategy` testi bu
//!            mutantı öldürmek için yazıldı ama ÖLDÜREMEZ (yukarıdaki sebep). Test yine de
//!            geçerlidir — `advance >= window` sınırındaki strateji seçimini pinler.
//!
//!   `151:24` `pn += 1` → `pn *= 1` sonsuz döngü üretir; TIMEOUT olarak raporlanır.
//!            Bu YAKALANMIŞTIR, kaçmamıştır (test paketi asılıyor).
//!
//! Yani `pn.rs` için ETKİN mutasyon skoru **83/83 = %100**'dür.
//!
//! Koş: `cargo test -p kama-noise-wire --test pn_boundary_mutants`
//! Doğrula: `cargo mutants -p kama-noise-wire --file crates/kama-noise-wire/src/pn.rs`

use kama_noise_wire::error::PnReject;
use kama_noise_wire::pn::{reconstruct, ReplayWindow, EXTENDED_PN_PARAMS, LEGACY_PN_PARAMS};

const SPAN: u64 = 1u64 << 32;
const HALF: u64 = SPAN / 2;
/// `EXTENDED_PN_PARAMS.window` — bit-ring uzunluğu.
const W: u64 = 1u64 << 14;

// ════════════════════════════════════════════════════════════════════════════
// reconstruct() — en-yakın-aday seçimi, beraberlik anları
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `pn.rs:51:49` — `is_some_and(|v| v < center)` → `v <= center`.
#[test]
fn reconstruct_exact_backward_tie_picks_the_lower_candidate() {
    // `cand + half == center` TAM eşitlik anı. İki aday da center'a eşit uzaklıkta
    // (biri span aşağıda, biri yukarıda). Kod `<` kullanır → DÜŞÜK adayı seçer.
    //
    // Bu bir tasarım kararıdır ve pinlenmelidir: beraberlikte ileri atlamak, gecikmiş bir
    // paketi geleceğe taşır ve replay penceresini yanlış yöne kaydırır.
    let p = &EXTENDED_PN_PARAMS;
    assert_eq!(
        reconstruct(p, HALF, 0),
        0,
        "cand+half == center beraberliğinde DÜŞÜK aday seçilmeli (`<`, `<=` değil)"
    );
    // Komşu: bir birim ileri → artık beraberlik yok, düşük aday zaten doğru.
    assert_eq!(reconstruct(p, HALF, 1), 1);
    // Komşu: bir birim geri → beraberlik yok.
    assert_eq!(reconstruct(p, HALF - 1, 0), 0);
}

/// MUTANT `pn.rs:53:20` — `cand > center.saturating_add(half)` → `>=`.
#[test]
fn reconstruct_exact_forward_tie_keeps_the_higher_candidate() {
    // `cand == center + half` TAM eşitlik. Yine beraberlik; kod `>` kullanır → span
    // ÇIKARMAZ, yani YÜKSEK adayı korur. Simetrik olarak yukarıdaki testin ikizi.
    let p = &EXTENDED_PN_PARAMS;
    for (center, low) in [
        (SPAN + 10, HALF + 10),
        (SPAN, HALF),
        (SPAN + 100, HALF + 100),
    ] {
        let want = SPAN + HALF + (center - SPAN);
        assert_eq!(
            reconstruct(p, center, low),
            want,
            "center={center} low={low}: beraberlikte YÜKSEK aday korunmalı (`>`, `>=` değil)"
        );
    }
}

/// MUTANT `pn.rs:50:25` — `base | low_bits` → `base ^ low_bits`.
///
/// **[ÖLÇÜLDÜ] Bu bir EŞDEĞER MUTANT'tır — normal kullanımda öldürülemez.**
/// `base = center & !(span-1)` düşük bitleri sıfırlar; `low_bits < span` olduğu sürece
/// `base | low_bits == base ^ low_bits` her zaman doğrudur.
///
/// İkisi ancak `low_bits >= span` iken ayrışır. Üretimdeki iki çağıran da bunu sağlamaz
/// mı diye bakıldı: `open()` `u64::from(outer.pn_trunc)` verir (u32 < 2^32 = span),
/// `legacy::open()` `u64::from(seq16)` verir (u16 < 2^16 = legacy span). Yani ihlal
/// üretimde imkânsız.
///
/// Ancak `reconstruct` **public**'tir ve bu ön-koşul hiçbir yerde yazılı değildir. Test
/// bugünkü davranışı pinler; sözleşmenin belgelenmesi ayrı bir karardır.
/// [BEKLİYOR — Abkarada] `low_bits < 2^low_width_bits` ön-koşulu `debug_assert!` ile
/// zorlansın mı, yoksa doc-comment'ta belirtilsin mi?
#[test]
fn reconstruct_low_bits_beyond_span_is_an_undocumented_precondition() {
    let p = &EXTENDED_PN_PARAMS;
    // Sözleşme içi: her low_bits < span için sonucun düşük bitleri low_bits'e eşit.
    for low in [0u64, 1, 12345, SPAN - 1] {
        let got = reconstruct(p, SPAN + 10, low);
        assert_eq!(got & (SPAN - 1), low, "low={low}: düşük bitler korunmalı");
    }
    // Sözleşme dışı (low_bits >= span): bugünkü davranış — `|` semantiği, üst bit sızar.
    // Bu assert bir GEREKSİNİM değil, bir KAYITTIR.
    let out_of_contract = reconstruct(p, SPAN + 10, SPAN + 5);
    assert_eq!(
        out_of_contract,
        SPAN + 5,
        "sözleşme dışı girdide bugünkü davranış `base | low_bits`"
    );
}

// ════════════════════════════════════════════════════════════════════════════
// ReplayWindow::commit() — ring temizliği
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `pn.rs:100:9` (`bit_clear` → `()`) ve `pn.rs:101:39` (`<<` → `>>`).
///
/// **Bu, kaçan 13 mutantın en ciddisidir.** `bit_clear`'ın TÜM gövdesi silindiğinde hiçbir
/// test düşmüyordu — yani ring'in eski bitlerinin temizlendiği hiç kanıtlanmamıştı.
///
/// **Sonucu güvenlik değil kullanılabilirlik tarafında:** bayat bir bit, aynı ring yuvasına
/// düşen YENİ bir paketi sahte `Duplicate` ile reddettirir. Hostile ağda reordering yaygın
/// olduğundan bu, meşru paketlerin sessizce düşmesi demektir.
#[test]
fn ring_wrap_clears_stale_bits_from_the_previous_cycle() {
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);

    // (1) pn=3 kabul → ring yuvası 3 işaretlenir.
    w.commit(3);
    assert_eq!(w.expected_pn(), 4);

    // (2) İleri atla, ama ring'i tam turlamadan (advance < window → döngü yolu).
    //     Döngü pn 4..15999 yuvalarını temizler; YUVA 3'E DOKUNMAZ (bayat kalır).
    w.commit(15_999);
    assert_eq!(w.expected_pn(), 16_000);

    // (3) Ring'i yuva 3'ün üzerinden geçir: pn=16387 → 16387 % 16384 = 3.
    //     advance = 16388-16000+1 = 389 < window → yine döngü yolu.
    //     DOĞRU kod burada yuva 3'ü temizler; `bit_clear` no-op ise bayat bit KALIR.
    assert_eq!(16_387 % W, 3, "16387 ring yuvası 3 olmalı");
    w.commit(16_388);
    assert_eq!(w.expected_pn(), 16_389);

    // (4) pn=16387 hiç commit edilmedi → meşru geç paket, kabul EDİLMELİ.
    //     Bayat bit kalsaydı sahte Duplicate dönerdi.
    assert_eq!(
        w.precheck(16_387),
        Ok(()),
        "ring turlandıktan sonra bayat bit temizlenmedi — meşru paket sahte Duplicate aldı"
    );

    // (5) Kontrol: gerçekten commit edilmiş olan pn=16388 hâlâ Duplicate vermeli.
    //     (Temizlik fazla agresif olsaydı bu da Ok dönerdi.)
    assert_eq!(
        w.precheck(16_388),
        Err(PnReject::Duplicate),
        "commit edilmiş pn Duplicate vermiyor — temizlik fazla agresif"
    );
}

/// MUTANT `pn.rs:142:42` — `candidate - ew + 1` → `candidate - ew * 1`.
#[test]
fn advance_arithmetic_selects_the_right_clear_strategy() {
    // `advance` iki temizleme stratejisi arasında seçim yapar:
    //   advance >= window → tüm bitmap sıfırlanır
    //   advance <  window → yalnız ew..=candidate döngüyle temizlenir
    // Sınır TAM `advance == window` anındadır. `+1` düşerse sınır bir birim kayar ve
    // tam-sınırdaki commit yanlış stratejiyi seçer.
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);

    // Eski bir bit bırak.
    w.commit(0);
    assert_eq!(w.expected_pn(), 1);

    // advance = candidate - 1 + 1 = candidate. TAM window olması için candidate = window.
    // Bu, full-clear dalını seçmeli → yuva 0 (pn=0) temizlenir.
    assert_eq!(W - 1 + 1, W);
    w.commit(W);
    assert_eq!(w.expected_pn(), W + 1);

    // pn=0 artık pencere dışında (too-old) olmalı; bit durumu değil sınır kararı sınanır.
    assert_eq!(
        w.precheck(0),
        Err(PnReject::TooOld),
        "window kadar ilerledikten sonra pn=0 too-old olmalı"
    );
}

/// MUTANT `pn.rs:149:26` — `while pn <= candidate` → `while pn > candidate`.
#[test]
fn clear_loop_covers_the_candidate_itself() {
    // Döngü ew..=candidate aralığını KAPSAMALI (candidate dahil). `<=` yerine `>` olursa
    // döngü hiç çalışmaz ve tüm bayat bitler kalır.
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    w.commit(7);
    w.commit(15_999);
    // pn=16391 ring yuvası 7 (16391 % 16384 = 7) — pn=7'nin bayat biti burada.
    assert_eq!(16_391 % W, 7);
    w.commit(16_391);
    // 16391 commit edildi → Duplicate. Döngü çalışmasaydı da bit_set(16391) bunu verirdi,
    // bu yüzden asıl ayrım aşağıdaki komşu yuvada.
    assert_eq!(w.precheck(16_391), Err(PnReject::Duplicate));
    // pn=16390 (yuva 6) commit edilmedi → Ok olmalı.
    assert_eq!(w.precheck(16_390), Ok(()));
}

// ════════════════════════════════════════════════════════════════════════════
// ReplayWindow::precheck() — kabul/red sınırları
// ════════════════════════════════════════════════════════════════════════════

/// MUTANT `pn.rs:112:62` — TOO-OLD: `v < ew` → `v <= ew`.
#[test]
fn too_old_boundary_is_exclusive() {
    // `candidate + window == ew` TAM eşitliği too-old DEĞİLDİR — pencerenin en eski
    // kabul edilebilir noktasıdır. `<=` olsaydı pencere bir birim daralır ve en uçtaki
    // meşru geç paket düşerdi.
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    let top = W + 100;
    w.commit(top);
    let ew = w.expected_pn();
    assert_eq!(ew, top + 1);

    let edge = ew - W; // edge + window == ew
    assert_eq!(
        w.precheck(edge),
        Ok(()),
        "candidate + window == expected TAM sınırı kabul edilmeli (too-old DEĞİL)"
    );
    // Bir birim aşağısı artık too-old.
    assert_eq!(
        w.precheck(edge - 1),
        Err(PnReject::TooOld),
        "sınırın bir altı too-old olmalı"
    );
}

/// MUTANT `pn.rs:116:22` — DUPLICATE guard'ı: `candidate < ew` → `candidate <= ew`.
#[test]
fn duplicate_check_applies_only_to_the_past_not_to_expected_itself() {
    // `expected_pn` HENÜZ GÖRÜLMEMİŞ ilk pn'dir; duplicate kontrolüne girmemelidir.
    // `<=` olsaydı, ring'te aynı yuvaya düşen bayat bir bit yüzünden bir sonraki
    // meşru paket sahte Duplicate alırdı.
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);

    // Yuva 0'a bayat bit koy, sonra expected'i pn=W'ye (yuva 0) getir.
    w.commit(0);
    w.commit(W - 1); // expected = W, yuva W % 16384 = 0
    assert_eq!(w.expected_pn(), W);
    assert_eq!(W % W, 0, "expected_pn ring yuvası 0 ile çakışmalı");

    // candidate == expected_pn: duplicate kontrolüne GİRMEMELİ.
    assert_eq!(
        w.precheck(W),
        Ok(()),
        "candidate == expected duplicate sayıldı — guard `<` yerine `<=` olabilir"
    );
}

/// FUTURE-GAP kapısı — YALNIZ extended-pn32'de vardır; legacy-seq16'da [D-GAP-1] ile kalktı.
///
/// Extended: `candidate > ew + gap` → `>=` mutantı için sınır pinlenir (noise-wire §4; D-GAP-1
/// kapsamı dışında, kapı durur). Legacy: aynı uzaklık ve çok ötesi KABUL edilir.
#[test]
fn future_gap_boundary_is_inclusive_of_the_limit() {
    // Extended: `candidate == ew + FUTURE_GAP_MAX` TAM sınırı KABUL edilir; bir üstü reddedilir.
    let w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    let gap = EXTENDED_PN_PARAMS
        .future_gap_max
        .expect("extended sette far-future kapısı var");
    let ew = w.expected_pn();

    assert_eq!(
        w.precheck(ew + gap),
        Ok(()),
        "tam future_gap sınırı kabul edilmeli"
    );
    assert_eq!(
        w.precheck(ew + gap + 1),
        Err(PnReject::FutureGap),
        "sınırın bir üstü reddedilmeli"
    );

    // Legacy: kapı YOK ([D-GAP-1]). Eski sınırların (4096, 8192) bir üstü de, çok ötesi de kabul.
    assert_eq!(LEGACY_PN_PARAMS.future_gap_max, None);
    let lw = ReplayWindow::new(LEGACY_PN_PARAMS);
    for far in [4096u64, 4097, 8192, 8193, 1 << 20, 1 << 40, u64::MAX - 1] {
        assert_eq!(
            lw.precheck(far),
            Ok(()),
            "legacy'de far-future ({far}) precheck'te reddedildi — D-GAP-1 ihlali"
        );
    }
    // Tek istisna sayaç uzayının sonu: expected = candidate + 1 temsil edilemez.
    assert_eq!(lw.precheck(u64::MAX), Err(PnReject::FutureGap));
}

/// Legacy-seq16 — [D-GAP-1] sonrası davranış: ileri sıçrama KABUL, geriye tek sınır eski-paket kuralı.
///
/// Eski hâli `MAX_REORDER_DISTANCE = 8192` dalını pinliyordu; o dal ccr §35.8'den kaldırıldı
/// (ileri yarısı far-future reddiydi, geri yarısı zaten `window` ile kapsanıyordu).
#[test]
fn legacy_max_reorder_boundary_and_direction() {
    let mut w = ReplayWindow::new(LEGACY_PN_PARAMS);
    w.commit(20_000);
    let ew = w.expected_pn();
    assert_eq!(ew, 20_001);
    let win = LEGACY_PN_PARAMS.window; // 1024

    // İLERİ YÖN: eskiden 4097'de (future_gap) ve 8193'te (max_reorder) FutureGap dönüyordu.
    for ahead in [1u64, 4096, 4097, 8192, 8193, 100_000] {
        assert_eq!(
            w.precheck(ew + ahead),
            Ok(()),
            "legacy ileri sıçrama +{ahead} reddedildi — D-GAP-1: far-future kapısı yok"
        );
    }

    // GERİ YÖN: eski-paket kuralı `candidate + WINDOW < expected` → TooOld (underflow-safe form).
    // Sınır: candidate + win == ew kabul; bir altı TooOld.
    assert_eq!(
        w.precheck(ew - win),
        Ok(()),
        "tam pencere sınırı kabul edilmeli"
    );
    assert_eq!(
        w.precheck(ew - win - 1),
        Err(PnReject::TooOld),
        "pencerenin bir altı TooOld vermeli"
    );
    // Eski max_reorder uzaklığı da (geri yönde) TooOld — pencere daha dar olduğu için hep öyleydi.
    assert_eq!(w.precheck(ew - 8192), Err(PnReject::TooOld));

    // İLERİ SIÇRAMA KABULÜ: highest sıçrar, atlanan aralık "görülmedi" olur, eski paketler TooOld.
    let jump = ew + 100_000;
    w.commit(jump);
    assert_eq!(w.expected_pn(), jump + 1, "highest sıçramalı");
    assert_eq!(
        w.precheck(jump),
        Err(PnReject::Duplicate),
        "sıçranan PN tekrar kabul edilemez"
    );
    assert_eq!(
        w.precheck(jump - 1),
        Ok(()),
        "atlanan aralık temizlenmeli (pencere içi, görülmedi)"
    );
    assert_eq!(
        w.precheck(20_000),
        Err(PnReject::TooOld),
        "sıçramadan önceki PN artık pencere dışında"
    );
}

/// u64 uç değerlerinde aritmetik TAŞMAZ (Kani K-1'in örnekleme karşılığı; ispat `kani_pn_proofs.rs`).
#[test]
fn arithmetic_is_total_at_u64_extremes() {
    // reconstruct: en üst span bloğunda ileri aday yoktur; panik de yoktur.
    for p in [&EXTENDED_PN_PARAMS, &LEGACY_PN_PARAMS] {
        let span = 1u64 << p.low_width_bits;
        for expected in [u64::MAX, u64::MAX - 1, u64::MAX - span / 2, u64::MAX - span] {
            for low in [0u64, 1, span / 2, span - 1] {
                let got = reconstruct(p, expected, low);
                assert_eq!(got & (span - 1), low, "düşük bitler korunmalı");
            }
        }
    }
    // commit: ew = 0'dan en uzak kabul edilebilir PN'e sıçrama (dist + 1 taşardı).
    let mut w = ReplayWindow::new(LEGACY_PN_PARAMS);
    assert_eq!(w.precheck(u64::MAX - 1), Ok(()));
    w.commit(u64::MAX - 1);
    assert_eq!(w.expected_pn(), u64::MAX);
    assert_eq!(w.precheck(u64::MAX - 1), Err(PnReject::Duplicate));
    assert_eq!(w.precheck(u64::MAX), Err(PnReject::FutureGap));
}

// ════════════════════════════════════════════════════════════════════════════
// Vakumsuzluk
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn precheck_never_mutates_state() {
    // §4: precheck window-İLERLETMEZ, iz-BIRAKMAZ. Yukarıdaki sınır testlerinin hepsi
    // bu sözleşmeye dayanır; ihlal edilirse testler birbirini kirletir.
    let mut w = ReplayWindow::new(EXTENDED_PN_PARAMS);
    w.commit(100);
    let before = w.expected_pn();
    for c in [0u64, 50, 100, 101, 5000, 1 << 20] {
        let _ = w.precheck(c);
    }
    assert_eq!(w.expected_pn(), before, "precheck expected_pn'i değiştirdi");
    // Bit durumu da değişmemeli: commit edilmiş pn hâlâ Duplicate, edilmemiş hâlâ Ok.
    assert_eq!(w.precheck(100), Err(PnReject::Duplicate));
    assert_eq!(w.precheck(99), Ok(()));
}
