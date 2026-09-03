//! MUTATION-GÜDÜMLÜ — `seq.rs` full-counter yeniden kurulumu + `read.rs` bounds aritmetiği.
//!
//! **Kaynak:** İlk tam koşumda (`cargo mutants -p adaptive-transport`, 1106 mutant) bu iki
//! dosyada 5 kaçan vardı. Sonraki dosya-filtreli koşumlarda İKİSİ DE listeye alınmadı
//! (benim atlamam), yani hiç hedeflenmediler.
//!
//! ```text
//! seq.rs:24:40  replace | with ^  in resolve_seq
//! seq.rs:26:50  replace - with /  in resolve_seq
//! seq.rs:26:82  replace - with /  in resolve_seq
//! read.rs:27:27 replace - with +  in Reader::take
//! read.rs:27:27 replace - with /  in Reader::take
//! ```
//!
//! **Neden önemli:** `resolve_seq` wire'daki u16 seq'i 64-bit monoton sayaca çevirir
//! (RFC 9000 A.3'ün 16-bit özelleştirmesi). Yanlış çözülen bir seq, alıcıda sahte gap
//! (gereksiz ACK epizodu) ya da sahte duplicate (meşru paket düşer) üretir. `Reader::take`
//! ise parser bounds'udur; `needed` alanı çağırana kaç byte eksik olduğunu söyler.
//!
//! **[ÖLÇÜLDÜ] `24:40` (`|` → `^`) EŞDEĞERDİR** — `pn.rs:50:25` ile birebir aynı sınıf:
//! `expected & !mask` düşük 16 biti sıfırlar, `wire` u16 olduğu için her zaman `< 2^16`,
//! dolayısıyla `|` ile `^` ÖZDEŞ. Bu mutant için test yazılmamalıdır.
//!
//! Koş: `cargo test -p adaptive-transport --test seq_and_reader_bounds`

use adaptive_transport::seq::resolve_seq;

const WIN: u64 = 1 << 16; // 65536
const HWIN: u64 = WIN / 2; // 32768

// ════════════════════════════════════════════════════════════════════════════
// resolve_seq — epoch seçimi
// ════════════════════════════════════════════════════════════════════════════

/// Doküman-örnekleri regresyon çapası olarak pinlenir.
#[test]
fn documented_examples_stay_pinned() {
    // [KOD] doc-test'teki iki örnek. Doc-test workspace'te koşuyor ama burada da
    // pinlenir: doc-comment silinirse ya da örnek değişirse davranış kaybolmasın.
    assert_eq!(resolve_seq(65536, 0xFFFF), 65535, "boundary-altı reorder");
    assert_eq!(resolve_seq(65535, 0x0001), 65537, "boundary-üstü ileri");
}

/// MUTANT `seq.rs:26:50` — `candidate <= expected - hwin` → `expected / hwin`.
#[test]
fn backward_epoch_bump_uses_subtraction_not_division() {
    // Aday `expected`'in yarım pencereden FAZLA gerisindeyse, aynı wire'ın BİR SONRAKİ
    // epoch'una ait olduğu anlaşılır ve bir pencere ileri taşınır.
    //
    // `expected - hwin` yerine `expected / hwin` konursa eşik devasa küçülür ve neredeyse
    // hiçbir aday kolu tetiklemez — geç gelen bir paket geçmişte kalır, alıcıda sahte
    // duplicate olarak düşer.
    //
    // [ÖLÇÜLDÜ] exp=105536, wire=1000 → doğru 132072, mutant 66536.
    let expected = 105_536u64;
    assert_eq!(
        resolve_seq(expected, 1000),
        132_072,
        "geri-epoch tespiti çalışmıyor — eşik bölmeye dönmüş olabilir"
    );

    // Aynı desen ikinci bir noktada.
    assert_eq!(resolve_seq(WIN + 40_000, 1000), 132_072);

    // Kontrast: yarım pencere İÇİNDEKİ aday taşınmamalı (kolun aşırı tetiklenmediği).
    let near = resolve_seq(200_000, 5000);
    assert_eq!(near, 201_608, "yakın aday gereksiz yere taşındı");
}

/// MUTANT `seq.rs:26:82` — `candidate <= u64::MAX - win` → `u64::MAX / win`.
#[test]
fn overflow_guard_uses_subtraction_not_division() {
    // Bu, `candidate + win` toplamasının taşmasını engelleyen guard'dır. `u64::MAX - win`
    // pratikte "her zaman doğru"dur (aday o kadar büyük olamaz); `u64::MAX / win` ise
    // 2^48'e iner ve ondan BÜYÜK her aday için kolu kapatır.
    //
    // Yani mutant, yüksek sayaç değerlerinde geri-epoch tespitini SESSİZCE devre dışı
    // bırakır. Uzun oturumlarda (sub-kbps'te saatlerce) sayaç oraya ulaşır.
    //
    // [ÖLÇÜLDÜ] exp=2^50+40000, wire=1000 → doğru 1125899906909160, mutant 1125899906843624.
    let expected = (1u64 << 50) + 40_000;
    assert_eq!(
        resolve_seq(expected, 1000),
        1_125_899_906_909_160,
        "yüksek sayaçta geri-epoch tespiti kapandı — taşma guard'ı bölmeye dönmüş olabilir"
    );

    // İkinci nokta, daha da yüksek.
    let expected2 = (1u64 << 52) + 50_000;
    assert_eq!(resolve_seq(expected2, 2000), 4_503_599_627_438_032);

    // Sınırın DOĞRU tarafı: 2^48 ALTINDA aynı desen hem doğru kodda hem mutantta çalışır,
    // yani ayrım yalnız yukarıda görünür. Bu, yukarıdaki iki noktanın neden gerekli
    // olduğunu gösterir (küçük değerlerle test etmek bu mutantı yakalamazdı).
    assert_eq!(resolve_seq(105_536, 1000), 132_072);
}

/// Yarım-pencere beraberliği — kolun yönü.
#[test]
fn exact_half_window_tie_resolves_forward() {
    // [KOD] modül başlığı normatif: "tam yarım-pencere (`hwin`) tie'ı ileri/yüksek adaya
    // çözülür". `<=` kullanıldığı için TAM eşitlik geri-epoch koluna girer.
    //
    // Bu bir tasarım kararıdır ve pinlenmelidir: beraberlikte geriye çözmek, geç gelen
    // paketi geçmişe atar ve replay penceresini yanlış yöne kaydırır.
    let expected = 100 * WIN + HWIN;
    let wire = 0u16;
    let cand = (expected & !(WIN - 1)) | u64::from(wire);
    assert_eq!(
        expected - cand,
        HWIN,
        "kurulum: aday tam yarım pencere geride olmalı"
    );
    assert_eq!(
        resolve_seq(expected, wire),
        cand + WIN,
        "tam hwin beraberliği ileri çözülmeli"
    );
}

#[test]
fn low_bits_are_always_preserved() {
    // Genel invariant: sonucun düşük 16 biti HER ZAMAN `wire`'a eşit olmalı. Aksi hâlde
    // yeniden kurulum wire'ı bozmuş demektir. Bu, `24:40` (`|` → `^`) eşdeğer mutantının
    // da neden öldürülemediğini gösterir: iki operatör de bu invariantı korur.
    for expected in [0u64, 1, HWIN, WIN, WIN + 1, 1 << 30, (1u64 << 50) + 12_345] {
        for wire in [0u16, 1, 0x7FFF, 0x8000, 0xFFFF] {
            let got = resolve_seq(expected, wire);
            assert_eq!(
                got & (WIN - 1),
                u64::from(wire),
                "exp={expected} wire={wire:#06x}: düşük bitler bozuldu"
            );
        }
    }
}

#[test]
fn result_is_always_within_one_window_of_expected() {
    // Sonuç `expected`'e en fazla bir pencere uzaklıkta olmalı — algoritmanın tanımı bu.
    // Herhangi bir kol yanlış tetiklenirse sonuç bu bantdan çıkar.
    for expected in [WIN, 2 * WIN, 100 * WIN + 12_345, (1u64 << 40) + 777] {
        for wire in [0u16, 1, 0x4000, 0x8000, 0xFFFF] {
            let got = resolve_seq(expected, wire);
            let dist = got.abs_diff(expected);
            assert!(
                dist <= WIN,
                "exp={expected} wire={wire:#06x} → {got}: mesafe {dist} > pencere {WIN}"
            );
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// read.rs::Reader::take — bounds aritmetiği
// ════════════════════════════════════════════════════════════════════════════
//
// [ÖLÇÜLDÜ] `read.rs:27:27` (`n - self.remaining()` → `+` / `/`) mutantları, `Reader`
// `pub(crate)` olduğu için ENTEGRASYON TESTİNDEN erişilemez. `TransportError::Truncated`
// yalnız parser hataları üzerinden gözlemlenebiliyor ve orada `needed` alanı çağırana
// hiç raporlanmıyor.
//
// [BEKLİYOR — Abkarada] `needed` alanı bir sözleşme mi (çağıran kaç byte daha okumalı)
// yoksa yalnız teşhis mi? Sözleşmeyse `pub` bir yüzeyden gözlemlenebilir olmalı;
// teşhisse mutantlar kalıcı olarak kaçacak ve kayda geçmeli.
//
// Ölçülen ayrım değerleri (birim testte kullanılabilir):
//   buf=4 pos=0 n=10 → doğru needed=6,  `+` mutantı 14, `/` mutantı 2
//   buf=4 pos=2 n=5  → doğru needed=3,  `+` mutantı 7,  `/` mutantı 2
//   buf=0 pos=0 n=1  → doğru needed=1,  `+` mutantı 1,  `/` mutantı DIV0 (panik!)
//
// Son satır özellikle önemli: `/` mutantı `remaining()==0` iken SIFIRA BÖLME paniği üretir.
// Yani o mutant aslında bir panik-yolu; testten değil, `debug_assert` veya fuzz'dan
// yakalanması daha doğal.
