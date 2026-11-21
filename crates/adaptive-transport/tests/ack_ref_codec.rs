//! ACK REFERANS DÜZENEĞİ · (a) codec kesinliği + (e) çerçeve boyu — BUGÜNKÜ davranışın kaydı.
//!
//! # Neden var
//!
//! Dönüş hattı dar (hedef hatta 62 B/s) ve bir ACK datagramı ≈ 110 B tutuyor. ACK'in
//! sıkıştırılması (yeni çerçeve biçimi) gündemde; ama önce bugünkü ACK'in NE TAŞIDIĞI makinece
//! sabitlenmeli ki gelecekteki biçim değişikliği bu dosyaya karşı yargılanabilsin. Bu dosya ACK
//! tel biçimini ya da motor davranışını DEĞİŞTİRMEZ; yalnız ölçer.
//!
//! # Ne sınanır
//!
//! 1. **Bilgi kümesi kesinliği (anlamsal özdeşlik).** Her çerçeve türü için (0x14 V2 — bugün
//!    yayılan; 0x10 V1 — yalnız çözme uyumluluğu) rastgele alanlı çerçeve: `decode(encode(f)) == f`
//!    — paket numaraları, aralıklar ve diğer alanlar korunur. BAYT eşitliği burada İDDİA EDİLMEZ:
//!    farklı kodlamalar aynı bilgiyi taşıyabilir; bayt eşitliği yalnız sabit kodlama vektörlerinde
//!    (`vectors/transport/ack-frame-v1.json`) denetlenir.
//! 2. **Alınan küme → teyit kümesi.** Alıcının alındı kümesi (rastgele kayıp, patlamalı kayıp
//!    (Gilbert–Elliott), u16 sarması) `from_received → encode → decode → acked_full` yolundan
//!    geçince çıkan küme, BAĞIMSIZ referans modelin ürettiği kümeye EŞİTTİR: en yüksek
//!    `1 + MAX_ACK_RANGES` blok (blok0 + 16 aralık) eksiksiz, daha eskiler hiç. Yani çerçeve
//!    alınmamış hiçbir seq'i teyit etmez ve tavan dışında hiçbir alınmış seq'i atlamaz.
//! 3. **Düşman girdi** tipli hatayla reddedilir: aralık sayısı tavan üstü
//!    (`TooManyAckRanges`), her kesik önek (`Truncated`), artık bayt (`TrailingBytes`), bilinmeyen
//!    tür (`UnknownFrameType`). `acked_full` çıktısı düşman `len` şişirmesinde sınırlı kalır.
//! 4. **Boy tavanı (e, çerçeve yarısı).** Yayılan her çerçeve ≤ [`ACK_FRAME_MAX_BYTES`]; tavana
//!    gerçekten ulaşıldığı da kanıtlanır (vakum bekçisi). Zarf dahil datagram boyu
//!    `crates/kama-integration/tests/ack_ref_wire_size.rs`'te gerçek kriptoyla sınanır.
//!
//! Sabit vektörler `vectors/transport/ack-frame-v1.json`'dadır (Rust'tan bağımsız üretici
//! `ref/transport/gen_ack_vectors.py`; tüketici `crates/kama-integration/tests/ack_ref_vectors.rs`).
//!
//! Rastgelelik testin kendi LCG'sidir (yeni bağımlılık yok; tohumlar sabit → koşu belirlenimli).

use std::collections::BTreeSet;

use adaptive_transport::ack::{
    AckFrame, AckRange, ARRIVAL_RATE_SENTINEL, FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2, MAX_ACKED_SEQS,
    MAX_ACK_RANGES,
};
use adaptive_transport::error::TransportError;

/// Bugün yayılan en büyük ACK çerçevesi (bayt): V2 sabit gövde 15 B + 16 aralık × 4 B = 79 B.
/// SAYI ELLE YAZILDI (koddan türetilmez): ACK biçimini değiştiren PR bu sabiti bilerek değiştirir
/// ve değişiklik PR'da görünür.
const ACK_FRAME_MAX_BYTES: usize = 79;
/// Bugünkü aralık tavanı (elle; `MAX_ACK_RANGES` değişirse bu dosya kırmızı olur).
const RANGE_CAP_TODAY: usize = 16;
/// V2 sabit gövde / aralık başı bayt (elle; spec §2.2).
const V2_FIXED: usize = 15;
const V1_FIXED: usize = 9;
const PER_RANGE: usize = 4;

/// Testin kendi LCG'si (Knuth MMIX sabitleri).
struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn permille(&mut self) -> u64 {
        self.below(1000)
    }
}

/// Kayıp deseni: rastgele (Bernoulli) ya da patlamalı (Gilbert–Elliott iki durum).
#[derive(Debug, Clone, Copy)]
enum Pattern {
    Random { loss: u64 },
    Burst { p_gb: u64, p_bg: u64, loss_bad: u64 },
}

/// `start..start+span` aralığında desene göre ALINAN seq kümesi (en az bir eleman).
fn received_set(rng: &mut Lcg, start: u64, span: u64, pat: Pattern) -> Vec<u64> {
    let mut out = Vec::new();
    let mut bad = false;
    for s in start..start + span {
        let lost = match pat {
            Pattern::Random { loss } => rng.permille() < loss,
            Pattern::Burst {
                p_gb,
                p_bg,
                loss_bad,
            } => {
                if bad {
                    if rng.permille() < p_bg {
                        bad = false;
                    }
                } else if rng.permille() < p_gb {
                    bad = true;
                }
                bad && rng.permille() < loss_bad
            }
        };
        if !lost {
            out.push(s);
        }
    }
    if out.is_empty() {
        out.push(start + span - 1);
    }
    out
}

/// BAĞIMSIZ referans: alınan kümenin en yüksek `1 + cap` ardışık bloğunun birleşimi.
fn reference_acked(received: &[u64], cap: usize) -> BTreeSet<u64> {
    let set: BTreeSet<u64> = received.iter().copied().collect();
    let mut blocks: Vec<(u64, u64)> = Vec::new(); // azalan
    for &s in set.iter().rev() {
        match blocks.last_mut() {
            Some((lo, _)) if *lo == s + 1 => *lo = s,
            _ => blocks.push((s, s)),
        }
    }
    blocks
        .into_iter()
        .take(1 + cap)
        .flat_map(|(lo, hi)| lo..=hi)
        .collect()
}

fn block_count(received: &[u64]) -> usize {
    let set: BTreeSet<u64> = received.iter().copied().collect();
    let v: Vec<u64> = set.into_iter().collect();
    1 + v.windows(2).filter(|w| w[1] != w[0] + 1).count()
}

fn patterns() -> Vec<Pattern> {
    vec![
        Pattern::Random { loss: 0 },
        Pattern::Random { loss: 50 },
        Pattern::Random { loss: 200 },
        Pattern::Random { loss: 500 },
        Pattern::Random { loss: 900 },
        Pattern::Burst {
            p_gb: 20,
            p_bg: 300,
            loss_bad: 900,
        },
        Pattern::Burst {
            p_gb: 100,
            p_bg: 100,
            loss_bad: 1000,
        },
        Pattern::Burst {
            p_gb: 5,
            p_bg: 50,
            loss_bad: 700,
        },
    ]
}

#[test]
fn ack_ref_constants_pinned() {
    // Bugünkü değerler (elle). Değişen PR bu dosyayı bilerek günceller.
    assert_eq!(MAX_ACK_RANGES, RANGE_CAP_TODAY, "aralık tavanı değişti");
    assert_eq!(FRAME_TYPE_ACK_V2, 0x14);
    assert_eq!(FRAME_TYPE_ACK, 0x10);
    assert_eq!(V2_FIXED + PER_RANGE * RANGE_CAP_TODAY, ACK_FRAME_MAX_BYTES);
}

/// (a-1) Rastgele alanlı çerçeve: decode(encode(f)) == f (anlamsal özdeşlik) — iki tür.
#[test]
fn ack_ref_frame_roundtrip_every_type() {
    let mut rng = Lcg::new(0xACC0_0001);
    let mut checked = [0u32; 2];
    for i in 0..20_000u32 {
        let v2 = i % 2 == 0;
        let n = rng.below(RANGE_CAP_TODAY as u64 + 1) as usize;
        let ranges: Vec<AckRange> = (0..n)
            .map(|_| AckRange {
                gap: rng.next() as u16,
                len: rng.next() as u16,
            })
            .collect();
        let f = AckFrame {
            ack_epoch: rng.next() as u8,
            largest_seq: rng.next() as u16,
            ack_delay: rng.next() as u16,
            block0_len: rng.next() as u16,
            ranges,
            v2,
            // V1 bu alanları taşımaz: çözücü nötr değer döner (bilgi kümesinin parçası değil).
            seen_count: if v2 { rng.next() as u32 } else { 0 },
            arrival_rate: if v2 {
                rng.next() as u16
            } else {
                ARRIVAL_RATE_SENTINEL
            },
            bitmap: None,
        };
        let b = f.encode();
        let fixed = if v2 { V2_FIXED } else { V1_FIXED };
        assert_eq!(b.len(), fixed + PER_RANGE * n, "boy formülü");
        assert_eq!(
            b[0],
            if v2 {
                FRAME_TYPE_ACK_V2
            } else {
                FRAME_TYPE_ACK
            }
        );
        let g = AckFrame::decode(&b).expect("geçerli çerçeve çözülmeli");
        assert_eq!(g, f, "decode(encode(f)) != f (i={i})");
        checked[usize::from(v2)] += 1;
    }
    assert!(checked[0] > 0 && checked[1] > 0, "iki tür de sınanmalı");
}

/// (a-2) Alınan küme → teyit kümesi; rastgele + patlamalı kayıp, sarma, beklenen-ileri ofset.
#[test]
fn ack_ref_received_set_survives_codec_exactly() {
    let mut cases = 0u32;
    let mut capped = 0u32;
    let mut max_len = 0usize;
    for (pi, pat) in patterns().into_iter().enumerate() {
        let mut rng = Lcg::new(0xACC0_1000 + pi as u64);
        for _ in 0..400 {
            // Başlangıçlar: sıfır yakını, u16 sarma sınırı, büyük sayaç.
            let start = match rng.below(3) {
                0 => rng.below(1000),
                1 => 65_536 * (1 + rng.below(4)) - rng.below(300),
                _ => (1u64 << 40) + rng.below(1 << 20),
            };
            // Alıcının kayan penceresi (RX_DEDUP_WINDOW) 4096; span bunun altında.
            let span = 1 + rng.below(4000);
            let recv = received_set(&mut rng, start, span, pat);
            let largest = *recv.iter().max().expect("boş değil");
            let f = AckFrame::from_received(&recv, 0, 7, 1, ARRIVAL_RATE_SENTINEL)
                .expect("boş olmayan küme");
            assert!(f.v2, "emit daima V2");
            let b = f.encode();
            max_len = max_len.max(b.len());
            assert!(
                b.len() <= ACK_FRAME_MAX_BYTES,
                "çerçeve {} B > {ACK_FRAME_MAX_BYTES}",
                b.len()
            );
            let g = AckFrame::decode(&b).expect("çöz");
            assert_eq!(g, f);
            // Gönderici `expected`'i alıcının en büyüğünün ilerisinde olabilir (uçuştaki seq'ler):
            // yarım pencerenin altında her ofset aynı sonucu vermeli.
            let ahead = rng.below(16_384);
            let got: BTreeSet<u64> = g.acked_full(largest + 1 + ahead).into_iter().collect();
            let want = reference_acked(&recv, RANGE_CAP_TODAY);
            assert_eq!(
                got, want,
                "teyit kümesi referansla aynı değil (desen {pat:?}, başlangıç {start}, span {span})"
            );
            let all: BTreeSet<u64> = recv.iter().copied().collect();
            assert!(got.is_subset(&all), "alınmamış seq teyit edildi");
            let blocks = block_count(&recv);
            if blocks > 1 + RANGE_CAP_TODAY {
                capped += 1;
                assert_eq!(f.ranges.len(), RANGE_CAP_TODAY, "tavanda 16 aralık");
            } else {
                assert_eq!(got, all, "tavan altında küme eksiksiz taşınmalı");
                assert_eq!(f.ranges.len(), blocks - 1);
            }
            cases += 1;
        }
    }
    assert!(cases >= 3000, "vakum: {cases}");
    assert!(capped > 0, "vakum: tavan kesmesi hiç sınanmadı");
    assert_eq!(
        max_len, ACK_FRAME_MAX_BYTES,
        "tavan boyuna gerçekten ulaşılmalı"
    );
}

/// (a-3) Aralık sayısı tavan üstü: her iki tür, 17..=255, tipli red.
#[test]
fn ack_ref_hostile_range_count_rejected() {
    for ft in [FRAME_TYPE_ACK, FRAME_TYPE_ACK_V2] {
        for count in (RANGE_CAP_TODAY + 1)..=255usize {
            let mut b = vec![ft, 0, 0x12, 0x34, 0, 1, 0, 3, count as u8];
            b.extend(std::iter::repeat_n(0u8, count * PER_RANGE));
            if ft == FRAME_TYPE_ACK_V2 {
                b.extend_from_slice(&[0, 0, 0, 1, 0xFF, 0xFF]);
            }
            assert_eq!(
                AckFrame::decode(&b),
                Err(TransportError::TooManyAckRanges {
                    count,
                    max: RANGE_CAP_TODAY
                }),
                "ft=0x{ft:02x} count={count}"
            );
            // Gövdesi kesik olsa da red aralık sayısında (bounds-before-alloc).
            assert!(matches!(
                AckFrame::decode(&b[..9]),
                Err(TransportError::TooManyAckRanges { .. })
            ));
        }
    }
}

/// (a-3) Her kesik önek `Truncated`, her artık bayt `TrailingBytes`, yabancı tür `UnknownFrameType`.
#[test]
fn ack_ref_hostile_truncation_trailing_unknown() {
    let mut rng = Lcg::new(0xACC0_2000);
    let mut frames = Vec::new();
    for n in [0usize, 1, 5, RANGE_CAP_TODAY] {
        for v2 in [false, true] {
            let f = AckFrame {
                ack_epoch: 3,
                largest_seq: 0xFFFE,
                ack_delay: 9,
                block0_len: 2,
                ranges: (0..n)
                    .map(|_| AckRange {
                        gap: rng.next() as u16,
                        len: rng.next() as u16,
                    })
                    .collect(),
                v2,
                seen_count: if v2 { 77 } else { 0 },
                arrival_rate: ARRIVAL_RATE_SENTINEL,
                bitmap: None,
            };
            frames.push(f.encode());
        }
    }
    for b in &frames {
        for cut in 0..b.len() {
            match AckFrame::decode(&b[..cut]) {
                Err(TransportError::Truncated { .. }) => {}
                other => panic!(
                    "kesik önek ({cut}/{} B) Truncated olmalı: {other:?}",
                    b.len()
                ),
            }
        }
        for extra in 1..=8usize {
            let mut t = b.clone();
            t.extend(std::iter::repeat_n(0xA5u8, extra));
            assert_eq!(
                AckFrame::decode(&t),
                Err(TransportError::TrailingBytes { count: extra })
            );
        }
        for ft in 0u8..=255 {
            if ft == FRAME_TYPE_ACK || ft == FRAME_TYPE_ACK_V2 {
                continue;
            }
            let mut t = b.clone();
            t[0] = ft;
            assert_eq!(
                AckFrame::decode(&t),
                Err(TransportError::UnknownFrameType(ft))
            );
        }
    }
    // Rastgele bayt dizileri: çözücü asla panik etmez; ya geçerli ya tipli hata.
    for _ in 0..20_000 {
        let len = rng.below(100) as usize;
        let mut t: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        if !t.is_empty() && rng.below(2) == 0 {
            t[0] = FRAME_TYPE_ACK_V2;
        }
        match AckFrame::decode(&t) {
            // Kabul edilen girdinin bilgisi yeniden kodlamada korunur (anlamsal; bayt değil).
            Ok(f) => assert_eq!(AckFrame::decode(&f.encode()), Ok(f)),
            Err(
                TransportError::Truncated { .. }
                | TransportError::TrailingBytes { .. }
                | TransportError::UnknownFrameType(_)
                | TransportError::TooManyAckRanges { .. },
            ) => {}
            Err(e) => panic!("beklenmeyen hata türü: {e:?}"),
        }
    }
}

/// (a-3) Geçerli aralık sayısında dev `len`/`gap`: çıktı sınırlı, panik yok.
#[test]
fn ack_ref_hostile_len_inflation_bounded() {
    let mut rng = Lcg::new(0xACC0_3000);
    for _ in 0..200 {
        let f = AckFrame {
            ack_epoch: 0,
            largest_seq: rng.next() as u16,
            ack_delay: 0,
            block0_len: 0xFFFF,
            ranges: (0..RANGE_CAP_TODAY)
                .map(|_| AckRange {
                    gap: rng.next() as u16,
                    len: 0xFFFF,
                })
                .collect(),
            v2: true,
            seen_count: 0,
            arrival_rate: 0,
            bitmap: None,
        };
        let g = AckFrame::decode(&f.encode()).expect("geçerli");
        let expected = [0u64, 1, 1 << 20, u64::MAX / 2][rng.below(4) as usize];
        assert!(g.acked_full(expected).len() <= MAX_ACKED_SEQS);
    }
}
