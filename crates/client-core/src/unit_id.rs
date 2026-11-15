//! [ŞERİT-SIRA] Birim kimliğinde şerit etiketi — alıcıda şerit başına bağımsız sıralama.
//!
//! # Neden
//!
//! S1 sözleşmesi (`kama_contracts::s1::KamaTransport::send`, D-09) sıralı teslimi **aynı oturum ve
//! aynı sınıf içinde** garanti eder; sınıflar arası `send` sırası korunmayabilir. Alıcı ise bugüne
//! dek oturumun tek `unit_id` uzayını bütünüyle sıralıyordu (`transport.rs` `Reorder`): dosyanın
//! kayıp bir parçası sıra başındayken gelmiş metin ve PTT birimleri uygulamaya VERİLMİYORDU —
//! sözleşmenin vermediği bir sıra garantisi, ürün hedefinin ("akışlar birbirini bloklamaz")
//! ihlali pahasına uygulanıyordu. Sınıfın telde karşılığı yoktur (D-09: tel ve altın vektörler
//! değişmez); alıcı şeridi yalnız birim kimliğinden türetebilir.
//!
//! # Düzen (tel bayt düzeni DEĞİŞMEZ: DATA başlığı yine `[tip][seq u16][unit_id u64]`)
//!
//! ```text
//! unit_id (u64) = [ şerit 2 bit ][ şerit-içi sıra 62 bit ]
//!                   63..62          61..0
//! ```
//!
//! * Şerit indeksi [`crate::lanes::Lane`] kabul sırasıdır: 0 PTT · 1 SIG · 2 TEXT · 3 FILE.
//! * Her şeridin sırası KENDİ içinde 0'dan başlar ve boşluksuzdur → alıcı şerit içindeki boşluğu
//!   kayıp olarak tanır; başka şeridin birimi boşluk yaratmaz.
//! * Şeritler kapalıyken (ürün bugünkü yolu, bağımsız `bind`, bütün eski testler) her birim şerit
//!   0'a yazılır: etiket 0, kimlikler eskisiyle **bayt-aynı**.
//! * **En küçük kimlik en eski birim DEĞİLDİR** (PTT kimlikleri her zaman dosya kimliklerinden
//!   küçüktür). Kimlik sırasını "gönderim sırası" yerine kullanan yerler:
//!   - Motor (`adaptive-transport`) kimliği opak taşır. PTO prob hedefi "en küçük birim kimliği"dir
//!     (CC-PORT U16): etiketle bu sıra "önce şerit, sonra sıra" olur — prob önce PTT, sonra SIG,
//!     TEXT, FILE'ın en eski teyitsiz birimini taşır; şerit kabul sırasıyla tutarlıdır, tek şeritte
//!     eski davranışla aynıdır (bilinçli olarak korundu). PTO yalnız ACK ilerlemesi yokken çalışır.
//!   - Port motorunun kayıp-yapısı kestiricisi kimlik yerine orijinalin motor-içi gönderim
//!     sırasını kullanır (`Sender::fate_key`); kimlik sırası pencereyi en yüksek şeridin eski
//!     kayıtlarında dondururdu.
//!   - client-core'un tavanlı kümeleri (gönderici onarım deposu, alıcı FEC birleştiricisi, ilk-teslim
//!     dedup penceresi) en küçük kimliği değil, en eski girişi / en kalabalık şeridin en eskisini
//!     düşürür (`runtime.rs` `repair_store_insert`, `busiest_lane`).
//!
//! # FEC blok eşlemesi
//!
//! Blok/ESI iki uçta da birim kimliğinden türetilir (`block = sıra / K`, `esi = sıra % K`); blok
//! kimliği (telde u32) şerit etiketini en yüksek 2 bitinde taşır ki iki şeridin blokları çakışmasın:
//!
//! ```text
//! block_id (u32) = [ şerit 2 bit ][ şerit-içi blok 30 bit ]
//! ```
//!
//! Şerit başına 2^30 blok; aşım `wrapping_add` ile şerit alanı içinde kalır.

use crate::lanes::Lane;

/// Şerit etiketinin bit konumu.
pub const LANE_SHIFT: u32 = 62;
/// Etiket maskesi (en yüksek 2 bit).
pub const LANE_TAG_MASK: u64 = 0b11 << LANE_SHIFT;
/// Şerit-içi sıra maskesi.
pub const LANE_SEQ_MASK: u64 = !LANE_TAG_MASK;
/// Şerit sayısı (etiket genişliğinin izin verdiği azami; [`Lane::ALL`] ile aynı).
pub const LANE_COUNT: usize = 4;

/// FEC blok kimliğinde şerit etiketinin bit konumu.
pub const FEC_BLOCK_LANE_SHIFT: u32 = 30;
/// FEC blok kimliğinde şerit-içi blok maskesi.
pub const FEC_BLOCK_SEQ_MASK: u32 = (1 << FEC_BLOCK_LANE_SHIFT) - 1;

/// Birimin şerit indeksi (0..4).
#[must_use]
#[inline]
pub const fn lane_index_of(unit_id: u64) -> usize {
    (unit_id >> LANE_SHIFT) as usize
}

/// Birimin şerit-içi sırası.
#[must_use]
#[inline]
pub const fn lane_seq_of(unit_id: u64) -> u64 {
    unit_id & LANE_SEQ_MASK
}

/// Şerit + şerit-içi sıra → birim kimliği. `lane ≥ 4` ya da `seq` 62 biti aşarsa maskelenir
/// (çağıranlar sınırlı sayaç kullanır; maske yalnız savunmadır).
#[must_use]
#[inline]
pub const fn tagged(lane: usize, seq: u64) -> u64 {
    (((lane & 0b11) as u64) << LANE_SHIFT) | (seq & LANE_SEQ_MASK)
}

/// Şerit → indeks ([`Lane::ALL`] sırası).
#[must_use]
#[inline]
pub const fn index_of_lane(lane: Lane) -> usize {
    match lane {
        Lane::Ptt => 0,
        Lane::Sig => 1,
        Lane::Text => 2,
        Lane::File => 3,
    }
}

/// Birimin FEC blok kimliği (şerit etiketli, telde u32).
#[must_use]
#[inline]
pub fn fec_block_of(unit_id: u64, k_shape: u64) -> u32 {
    let lane = lane_index_of(unit_id) as u32;
    let block = (lane_seq_of(unit_id) / k_shape.max(1)) as u32 & FEC_BLOCK_SEQ_MASK;
    (lane << FEC_BLOCK_LANE_SHIFT) | block
}

/// Birimin FEC blok içi kaynak indeksi (ESI).
#[must_use]
#[inline]
pub fn fec_esi_of(unit_id: u64, k_shape: u64) -> u16 {
    (lane_seq_of(unit_id) % k_shape.max(1)) as u16
}

/// Şeridin ilk FEC blok kimliği (şerit-içi blok 0).
#[must_use]
#[inline]
pub const fn first_fec_block_of_lane(lane: usize) -> u32 {
    ((lane & 0b11) as u32) << FEC_BLOCK_LANE_SHIFT
}

/// Şerit etiketli blok kimliğinin sıradaki bloğu (şerit alanı içinde sarar).
#[must_use]
#[inline]
pub const fn next_fec_block(block_id: u32) -> u32 {
    (block_id & !FEC_BLOCK_SEQ_MASK) | (block_id.wrapping_add(1) & FEC_BLOCK_SEQ_MASK)
}

/// (FEC blok kimliği, ESI) → birim kimliği; [`fec_block_of`] / [`fec_esi_of`]'un tersi.
#[must_use]
#[inline]
pub fn unit_of_fec(block_id: u32, esi: u16, k_shape: u64) -> u64 {
    let lane = (block_id >> FEC_BLOCK_LANE_SHIFT) as usize;
    let block = u64::from(block_id & FEC_BLOCK_SEQ_MASK);
    tagged(lane, block * k_shape + u64::from(esi))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_zero_is_byte_identical_to_untagged() {
        for seq in [0u64, 1, 7, 4095, u32::MAX as u64] {
            assert_eq!(tagged(0, seq), seq);
            assert_eq!(lane_index_of(seq), 0);
            assert_eq!(lane_seq_of(seq), seq);
            assert_eq!(fec_block_of(seq, 8), (seq / 8) as u32);
            assert_eq!(fec_esi_of(seq, 8), (seq % 8) as u16);
            assert_eq!(
                unit_of_fec(fec_block_of(seq, 8), fec_esi_of(seq, 8), 8),
                seq
            );
        }
    }

    #[test]
    fn tag_roundtrip_every_lane() {
        for lane in 0..LANE_COUNT {
            for seq in [0u64, 1, 9, 1000, (1 << 33) - 1] {
                let id = tagged(lane, seq);
                assert_eq!(lane_index_of(id), lane);
                assert_eq!(lane_seq_of(id), seq);
                let b = fec_block_of(id, 8);
                let e = fec_esi_of(id, 8);
                assert_eq!(b >> FEC_BLOCK_LANE_SHIFT, lane as u32);
                assert_eq!(unit_of_fec(b, e, 8), id);
                assert_eq!(
                    first_fec_block_of_lane(lane),
                    fec_block_of(tagged(lane, 0), 8)
                );
            }
            assert_eq!(
                next_fec_block(first_fec_block_of_lane(lane) | FEC_BLOCK_SEQ_MASK),
                first_fec_block_of_lane(lane),
                "şerit alanı içinde sarar"
            );
        }
    }

    #[test]
    fn lanes_do_not_collide() {
        let a = tagged(index_of_lane(Lane::Text), 3);
        let b = tagged(index_of_lane(Lane::File), 3);
        assert_ne!(a, b);
        assert_ne!(fec_block_of(a, 8), fec_block_of(b, 8));
        assert!(a < b, "etiketli sıra: önce PTT, SIG, TEXT, sonra FILE");
    }
}
