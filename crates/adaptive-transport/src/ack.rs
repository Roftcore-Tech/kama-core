//! TV-ACK: SACK feedback frame (engine spec §2), fixed-width big-endian (varint YASAK).
//!
//! Wire layout — V2 0x14 (EMIT, §2.2; 15 B sabit + 4 B × range_count):
//! ```text
//! frame_type u8=0x14 · ack_epoch u8 · largest_seq u16 · ack_delay u16(256µs) ·
//! block0_len u16 · range_count u8 · ranges[]{ gap u16, len u16 } ·
//! seen_count u32 · arrival_rate u16
//! ```
//! V1 0x10 (yalnız DECODE-uyumluluk; 9 B + 4 B × range): aynı baş, ack_delay µs, ölçüm eki YOK.
//! Descending reconstruction (engine §2):
//! ```text
//! cursor = largest_seq − block0_len + 1                 (block0'ın alt ucu)
//! her range: cursor −= gap;  block = [cursor−len+1 .. cursor];  cursor −= len
//! ```
//! u16 wrap: aritmetik mod 2^16 (wrapping); `ack_epoch` yalnız rekey sınırını
//! disambiguate eder — u16 sarması epoch değişimi DEĞİLDİR.
//!
//! **[ACK-BITMAP — biçim onayı YOK (tel değişikliği, sahip kararı), varsayılan kapalı]** 0x15
//! (17 B sabit + bitmap; varint yok, ADR-05):
//! ```text
//! frame_type u8=0x15 · ack_epoch u8 · largest_seq u16 · ack_delay u16(256µs) ·
//! arrival_rate u16 · rx_span u16(256µs) · span_first u16 · pair_rate u16 · base_seq u16 ·
//! bitmap_len u8 · bitmap[bitmap_len]
//! ```
//! `pair_rate` (ÇİFT ARALIĞI, #175 7068773849): ileri hattın kapasitesi için alıcıda ölçülmüş bir
//! ALT SINIR (B/s, UDP gövdesi). Son çıkan ACK'ten bu yana ardışık iki datagramın varış aralığı
//! `g` ve ikincinin boyu `b` (≥ [`PAIR_MIN_UDP_BYTES`]) için `b / g`'nin en büyük İKİNCİ değeri
//! (iki çift uyuşmalı). Varsayım: tek FIFO darboğaz, ondan sonra sıkışma yok — ikinci datagram
//! darboğazda en az kendi serileşme süresi kadar sonra çıkar, yani `b / g ≤ hat`; birinci datagram
//! ikincinin gönderiminde hâlâ serileşiyorsa eşittir. Kayıp, uygulama sınırlı ya da aralıklı
//! gönderim değeri yalnız küçültür. [`PAIR_RATE_NONE`] = ölçüm yok (iki uygun çift yok).
//! `rx_span` (ÖLÇÜM ARALIĞI, #175 7067964700): ölçüm aralığı `[span_first, largest_seq]`;
//! `span_first` alınmış bir seq'tir (alıcı, son çıkan ACK'inin en büyüğünden sonraki ilk alınan
//! seq'i seçer: aralık bir ACK geriye uzanır, iki değil — daralmayı eski veriyle seyreltmez).
//! Değer alıcıda `ilk_varış(largest) − ilk_varış(span_first)` (256 µs, aşağı yuvarlı).
//! [`RX_SPAN_NONE`] = ölçüm yok (span_first = largest, varış anı bilinmiyor ya da tavan aşıldı). Bayt taşınmaz: gönderici
//! M'deki seq'lerin boyunu kendi kaydından bilir; bayt ve süre aynı aralığa aittir. Dönüş
//! gecikmesi bu ölçüye girmez (ileri kuyruk büyümesi veri varışından ölçülür).
//! `bitmap` `[base_seq, largest_seq)` aralığını kapsar: bit i ↔ seq `base_seq + i`; bayt i/8,
//! bayt içinde MSB önce (bit 7 − i%8); 1 = alındı. `largest_seq` her zaman alınmıştır.
//! Açıklık `span = largest_seq − base_seq` (mod 2^16) ≤ [`ACK_BITMAP_MAX_BITS`];
//! `bitmap_len = ⌈span/8⌉` (kanonik) ve dolgu bitleri 0. **Anlam (kanıt aralığı):**
//! `[base_seq, largest_seq]` içinde bilgi TAMDIR — bit 0 = alıcı bu seq'i ACK anında almamıştı
//! (negatif kanıt). `base_seq` altı = BİLİNMİYOR (bu çerçeve hakkında hiçbir şey söylemez).
//! `arrival_rate` V2 (§2.2) ile AYNI alan ve anlamdır: alıcının gözlediği pencere-oranı (B/s),
//! sentinel [`ARRIVAL_RATE_SENTINEL`] = "ölçüm yok / üst-bantta, ipucu olarak kullanma". Erken
//! hız bilgisi bitmap biçiminde de taşınır (başlangıç kuyruğu işi bu alanı doldurur). V2'nin
//! `seen_count`'u taşınmaz: yalnız gözlemdir (`return_path`, karar girdisi değil).

use std::time::Duration;

use crate::error::TransportError;
use crate::read::Reader;

/// Transport-control frame_type: ACK **V1** (engine spec §1.1) — `ack_delay` birimi **µs**.
/// [TUR-1x] DECODE-uyumluluk için tutulur; EMIT artık [`FRAME_TYPE_ACK_V2`] (µs u16 tavanı 65.5ms,
/// sub-kbps cadence-hold'unda anında satüre → RTT düzeltmesi etkisizdi).
pub const FRAME_TYPE_ACK: u8 = 0x10;
/// Transport-control frame_type: ACK **V2** (§2.2, 15B gövde) [TUR-1x, Seçenek-A]:
/// `ack_delay` birimi **256µs** (tavan 65535×256µs ≈ 16.78s > ACK_CADENCE_FLOOR 8.39s) +
/// ölçüm eki (`seen_count` u32, `arrival_rate` u16 — K3 cold-start yön-ayrımı).
pub const FRAME_TYPE_ACK_V2: u8 = 0x14;
/// V2 `ack_delay` birimi (µs) [STD: Seçenek-A kararı, time-model-v1 D-4/1].
pub const ACK_DELAY_UNIT_US: u64 = 256;

/// [ACK-BITMAP · ölçüm prototipi] 0x15 frame_type'ı (client-core çerçeve düzlemi; 0x14 V2 ile aynı
/// ad-uzayı — `kama-transport-schema` inner_kind kayıt defteri AYRI bir ad-uzayıdır).
pub const FRAME_TYPE_ACK_BITMAP: u8 = 0x15;
/// [PTT-YALIN · YETENEK] 0x15 ile bayt-aynı bitmap ACK'i; tek farkı türü: yazan uç yalın PTT
/// çerçevesini (client-core 0x09) çözer. Yalnız karşı uç CAPS'te yalın PTT bitini bildirdiyse
/// yazılır (eski uç 0x16 görmez). Tel maliyeti yok.
pub const FRAME_TYPE_ACK_BITMAP_PTT: u8 = 0x16;
/// [PTT-BİRLEŞİK · deney] 0x16 ile bayt-aynı; yazan uç ayrıca birleşik PTT kayıtlarını çözer
/// (opak uygulama yeteneği). Yalnız karşı uç CAPS'te birleşik PTT bitini bildirdiyse yazılır.
pub const FRAME_TYPE_ACK_BITMAP_PTT_COALESCE: u8 = 0x17;
/// [ACK-BITMAP] 0x15 sabit gövde (bitmap hariç): tür + epoch + largest + delay + arrival_rate +
/// rx_span + span_first + pair_rate + base + len.
pub const ACK_BITMAP_HEADER_BYTES: usize = 17;
/// [ACK-BITMAP] ACK datagramının telde tavanı (B): bugünkü en büyük V2 ACK'i (16 aralık; 15 + 64 B
/// çerçeve + 50 B zarf: UDP/IP 28 + handle 4 + seq 2 + etiket 16). Bitmap çerçevesi bunu AŞMAZ.
pub const ACK_DATAGRAM_MAX_LINE_BYTES: usize = 129;
/// [ACK-BITMAP] ACK datagramının çerçeve dışı zarfı (B): UDP/IP 28 + handle 4 + seq 2 + etiket 16.
pub const ACK_DATAGRAM_ENVELOPE_BYTES: usize = 50;
/// [ACK-BITMAP] Bitmap bayt tavanı: tel tavanından türetilir (129 − 50 − 17 = 62 B).
pub const ACK_BITMAP_MAX_BYTES: usize =
    ACK_DATAGRAM_MAX_LINE_BYTES - ACK_DATAGRAM_ENVELOPE_BYTES - ACK_BITMAP_HEADER_BYTES;
/// [ACK-BITMAP] Bitmap'in kapsayabildiği en büyük açıklık (seq): 62 × 8 = 496.
pub const ACK_BITMAP_MAX_BITS: u64 = (ACK_BITMAP_MAX_BYTES * 8) as u64;

/// [ACK-BITMAP] 0x15 gövdesi: `[base_seq, largest_seq)` için alındı bitleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckBitmap {
    /// Bitmap'in kapsadığı en düşük seq (u16 wire).
    pub base_seq: u16,
    /// Bitler (bayt i/8, MSB önce); uzunluk `⌈(largest − base)/8⌉`.
    pub bits: Vec<u8>,
    /// Ölçüm aralığının alıcıdaki varış süresi (256 µs; [`RX_SPAN_NONE`] = ölçüm yok).
    pub rx_span: u16,
    /// Ölçüm aralığının ilk (alınmış) seq'i (u16 wire).
    pub span_first: u16,
    /// [ÇİFT ARALIĞI] İleri hat kapasitesinin alt sınırı (B/s, UDP gövdesi; [`PAIR_RATE_NONE`] =
    /// ölçüm yok). Modül belgesi.
    pub pair_rate: u16,
    /// [PTT-YALIN · YETENEK] `true` → çerçeve türü 0x16 (yazan uç yalın PTT'yi çözer).
    pub ptt_lean: bool,
    /// [PTT-BİRLEŞİK · deney] `ptt_lean` ile birlikte `true` → çerçeve türü 0x17.
    pub ptt_coalesce: bool,
}

/// [ACK-BITMAP · ÖLÇÜM ARALIĞI] `rx_span` "ölçüm yok" değeri.
pub const RX_SPAN_NONE: u16 = 0xFFFF;
/// [ÇİFT ARALIĞI] `pair_rate` "ölçüm yok" değeri (ölçülen değer 0xFFFE'de doyar).
pub const PAIR_RATE_NONE: u16 = 0xFFFF;
/// [ÇİFT ARALIĞI] Çiftin ikinci datagramının en küçük UDP gövdesi (B): küçük datagramda varış
/// aralığı zamanlayıcı ve sıra titreşimine göre kısadır.
pub const PAIR_MIN_UDP_BYTES: u64 = 800;

/// ACK range üst sınırı. `[CAL]` — netem baseline'dan (ADR-18) kalibre edilecek;
/// magic-number olmasın diye isimlendirildi (engine spec §2.1).
pub const MAX_ACK_RANGES: usize = 16;

/// [SR-AUDIT-001 / P3] Tek `acked_full` çağrısının döndürebileceği MAX seq sayısı (bounded transient).
/// 17 blok × u16-len yasal olarak ~1.1M üretebilir; gerçek inflight-penceresi bunun çok altında.
/// 65536 = bir tam u16-span: block0 tek başına asla aşamaz, yasal trafik yaklaşamaz, düşman `len`-
/// şişirmesi 512KB'ta durur (kümülatif SACK kaybı telafi eder). Bkz. `acked_full`.
pub const MAX_ACKED_SEQS: usize = 65536;

/// Önceki bloğun altındaki ek (gap, len) acknowledgement bloğu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckRange {
    /// Bir önceki bloğun altındaki kayıp aralık ölçüsü (u16, wire mod 2^16).
    pub gap: u16,
    /// Ardışık-alınan blok uzunluğu.
    pub len: u16,
}

/// u16 hop-seq uzayında selective-acknowledgement frame'i.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckFrame {
    /// Rekey / counter-wrap epoch'u (`largest_seq` bu epoch'ta).
    pub ack_epoch: u8,
    /// Alınan en yüksek seq (bu epoch, u16 wire).
    pub largest_seq: u16,
    /// RAW wire değeri — `largest_seq` alımı → bu ACK gönderimi (RTT düzeltmesi). Birim frame
    /// versiyonuna bağlı: V2 (0x14) → 256µs [TUR-1x], V1 (0x10) → µs. [`Self::ack_delay_duration`]
    /// ile çöz; raw değeri doğrudan µs sayma.
    pub ack_delay: u16,
    /// En üstteki ardışık-alınan blok uzunluğu (`largest_seq` dahil).
    pub block0_len: u16,
    /// Ek gap+blok çiftleri (descending); en çok [`MAX_ACK_RANGES`].
    pub ranges: Vec<AckRange>,
    /// [TUR-1x] wire versiyonu: `true` → 0x14 (ack_delay 256µs + V2 ölçüm eki), `false` → 0x10
    /// (µs, yalnız decode-uyumluluk).
    pub v2: bool,
    /// [§2.2 V2 ölçüm eki] Görülen toplam DATA datagram'ı (birim+kopya; saturating u32).
    /// Sender kendi gönderdiğiyle kıyaslar: seen≪sent = forward-loss, seen≈sent ama SACK yok =
    /// return-loss (K3 cold-start yön-ayrımı). V1 decode'da 0 (alan yok).
    pub seen_count: u32,
    /// [§2.2 V2 ölçüm eki] Receiver-gözlemli pencere-oranı (B/s). [`ARRIVAL_RATE_SENTINEL`] =
    /// ölçüm YOK ya da üst-bantta — cold-start ipucu olarak KULLANILMAZ. V1 decode'da sentinel.
    pub arrival_rate: u16,
    /// [ACK-BITMAP · ölçüm prototipi] `Some` → çerçeve 0x15'tir: `block0_len`/`ranges` boş ve
    /// anlamsız, `v2 = false`, `seen_count = 0`, `arrival_rate = sentinel`. `None` → 0x14/0x10
    /// (bugünkü yol, bayt bayt aynı).
    pub bitmap: Option<AckBitmap>,
}

/// [§2.2] `arrival_rate` sentinel: ölçüm yok / üst-bantta → ipucu olarak kullanma.
pub const ARRIVAL_RATE_SENTINEL: u16 = 0xFFFF;

impl AckFrame {
    /// Alınan full-counter seq kümesinden bir ACK kurar.
    ///
    /// `received_full` 64-bit monoton counter'lardır ([`crate::seq::resolve_seq`]
    /// ile çözülmüş). Gruplama full-counter uzayında yapılır → u16 wire sarması
    /// doğal olarak ele alınır; alanlar u16 wire değeri olarak yazılır. Range'ler
    /// en yüksek seq'ler korunacak şekilde [`MAX_ACK_RANGES`] ile clamp'lenir
    /// (engine spec §2.1). Boş girdide `None`.
    ///
    /// u16 alana sığmayan boşluk ya da blok (engine spec §2: "gap_i kadar seq KAYIP"; alan
    /// `u16_be`): bir boşluk 65535'i aşarsa o ve daha eski aralıklar YAZILMAZ; bir blok 65535'i
    /// aşarsa yalnız en üst 65535 seq'i yazılır ve daha eski aralıklar yazılmaz. Kırpılmış değer
    /// çözücüde alt blokları yanlış yere koyar ve hiç alınmamış seq'leri teyit ederdi; bildirmemek
    /// kayıp değildir (SACK kümülatif, §2.1 "taşarsa en yeni bloklar öncelik").
    #[must_use]
    pub fn from_received(
        received_full: &[u64],
        ack_epoch: u8,
        ack_delay: u16,
        seen_count: u32,
        arrival_rate: u16,
    ) -> Option<Self> {
        if received_full.is_empty() {
            return None;
        }
        let mut v: Vec<u64> = received_full.to_vec();
        v.sort_unstable();
        v.dedup();

        // Full-counter uzayında contiguous ascending bloklar [low, high].
        let mut blocks: Vec<(u64, u64)> = Vec::new();
        let mut low = v[0];
        let mut high = v[0];
        for &x in &v[1..] {
            if x == high + 1 {
                high = x;
            } else {
                blocks.push((low, high));
                low = x;
                high = x;
            }
        }
        blocks.push((low, high));

        // En yüksek blok = block0.
        let (b0_low, b0_high) = *blocks.last().expect("blocks boş olamaz (v boş değil)");
        let largest_seq = (b0_high & 0xFFFF) as u16;
        let block0_len = block_len_u16(b0_low, b0_high);

        // Alt blokları descending yay; en yüksek seq'leri koruyarak clamp'le.
        let mut ranges = Vec::new();
        let mut cursor = b0_low; // yayılmış son bloğun full-counter alt ucu
                                 // block0 kırpıldıysa çözücünün cursor'u b0_low değildir → alt aralık yazılamaz.
        let block0_exact = !block_len_clamped(b0_low, b0_high);
        for &(low, high) in blocks.iter().rev().skip(1) {
            if !block0_exact || ranges.len() == MAX_ACK_RANGES {
                break;
            }
            // full-uzay farkı; u16'ya sığmazsa bu ve daha eski aralıklar yazılmaz (doc).
            let Ok(gap) = u16::try_from(cursor - high) else {
                break;
            };
            let len = block_len_u16(low, high);
            ranges.push(AckRange { gap, len });
            if block_len_clamped(low, high) {
                // Yazılan blok yalnız [high − 65534, high]; çözücünün cursor'u `low − 1` değil.
                break;
            }
            cursor = low.saturating_sub(1);
        }

        Some(AckFrame {
            ack_epoch,
            largest_seq,
            ack_delay,
            block0_len,
            ranges,
            v2: true, // [TUR-1x] emit daima V2 (0x14, 15B gövde §2.2); V1 yalnız decode-uyumluluk.
            seen_count,
            arrival_rate,
            bitmap: None,
        })
    }

    /// [ACK-BITMAP · ölçüm prototipi] Alınan full-counter seq kümesinden 0x15 çerçevesi kurar.
    ///
    /// `largest_full` alınmış en büyük seq (çerçevenin `largest_seq`'i); `base_full` bitmap'in alt
    /// ucudur ve `[largest_full − ACK_BITMAP_MAX_BITS, largest_full]` içine kırpılır. `received`
    /// alınmış seq'leri artan sırada verir (`BTreeSet::range` gibi); aralık dışındakiler yok sayılır.
    #[must_use]
    #[allow(clippy::too_many_arguments)] // sabit genişlikli çerçeve alanları; yapı yerine düz liste
    pub fn bitmap_from_received(
        received: impl IntoIterator<Item = u64>,
        largest_full: u64,
        base_full: u64,
        ack_epoch: u8,
        ack_delay: u16,
        arrival_rate: u16,
        rx_span: u16,
        span_first: u64,
    ) -> Self {
        let base = base_full
            .min(largest_full)
            .max(largest_full.saturating_sub(ACK_BITMAP_MAX_BITS));
        let span = largest_full - base;
        let mut bits = vec![0u8; usize::try_from(span.div_ceil(8)).unwrap_or(0)];
        for s in received {
            if s < base || s >= largest_full {
                continue;
            }
            let i = usize::try_from(s - base).unwrap_or(0);
            bits[i / 8] |= 0x80 >> (i % 8);
        }
        AckFrame {
            ack_epoch,
            largest_seq: (largest_full & 0xFFFF) as u16,
            ack_delay,
            block0_len: 0,
            ranges: Vec::new(),
            v2: false,
            seen_count: 0,
            arrival_rate,
            bitmap: Some(AckBitmap {
                rx_span,
                span_first: (span_first & 0xFFFF) as u16,
                pair_rate: PAIR_RATE_NONE,
                ptt_lean: false,
                ptt_coalesce: false,
                base_seq: (base & 0xFFFF) as u16,
                bits,
            }),
        }
    }

    /// [ÖLÇÜM ARALIĞI] `span_first` tam sayaca (largest'tan geriye).
    #[must_use]
    pub fn span_first_full(&self, expected: u64) -> Option<u64> {
        let bm = self.bitmap.as_ref()?;
        let largest_full = crate::seq::resolve_seq(expected, self.largest_seq);
        let back = u64::from(self.largest_seq.wrapping_sub(bm.span_first));
        largest_full.checked_sub(back)
    }

    /// [ACK-BITMAP] Kanıt aralığı (full-counter, kapsayıcı): 0x15 çerçevesinde
    /// `Some((base, largest))` — bu aralıkta alınmamış her seq için negatif kanıt vardır; aralığın
    /// altı bilinmez. 0x14/0x10'da `None` (bugünkü anlam; gönderici kuralı değişmez).
    #[must_use]
    pub fn evidence_span_full(&self, expected: u64) -> Option<(u64, u64)> {
        let bm = self.bitmap.as_ref()?;
        let largest_full = crate::seq::resolve_seq(expected, self.largest_seq);
        let span = u64::from(self.largest_seq.wrapping_sub(bm.base_seq));
        Some((largest_full.saturating_sub(span), largest_full))
    }

    /// Çerçevenin telde (zarf hariç) bayt boyu — `encode().len()` ile aynı, ayırma yapmadan.
    #[must_use]
    pub fn frame_len(&self) -> usize {
        match &self.bitmap {
            Some(bm) => ACK_BITMAP_HEADER_BYTES + bm.bits.len(),
            None if self.v2 => 15 + 4 * self.ranges.len(),
            None => 9 + 4 * self.ranges.len(),
        }
    }

    /// [TUR-1x] `ack_delay` raw wire değerini süreye çözer (V2 → ×256µs, V1 → ×1µs).
    /// Floor-encoding V2'de encode-tarafındadır → decode şişirme yapmaz (delay ≤ gerçek).
    #[must_use]
    pub fn ack_delay_duration(&self) -> Duration {
        // [ACK-BITMAP] 0x15 da 256µs birimlidir.
        let unit_us = if self.v2 || self.bitmap.is_some() {
            ACK_DELAY_UNIT_US
        } else {
            1
        };
        Duration::from_micros(u64::from(self.ack_delay) * unit_us)
    }

    /// Acknowledge edilen full-counter seq kümesini rekonstrükte eder — [`Self::from_received`]'in
    /// TERSİ (§2 descending). `expected` = beklenen bir sonraki full counter; `largest_seq` (u16 wire)
    /// bununla [`crate::seq::resolve_seq`] üzerinden full'e kaldırılır, alt bloklar full-uzayda türetilir.
    ///
    /// Sender'ın `OnAckReceived` (§3.2) wire ACK'ten çözülmüş `acked_seqs`'i buradan alır. TAM'dır ancak
    /// encode'da hiçbir gap/blok u16'yı aşmadıysa (`from_received` u16'ya sığmayan boşluktan ya da
    /// bloktan sonraki eski aralıkları yazmaz, kırpılmış değer yazmaz — normal çalışmada aşılmaz). `ranges` [`MAX_ACK_RANGES`] ile sınırlı → çok-eski bloklar bu
    /// ACK'te olmayabilir (SACK kümülatif: sonraki ACK'ler en yüksek seq'leri tekrar kapsar).
    #[must_use]
    pub fn acked_full(&self, expected: u64) -> Vec<u64> {
        let mut acked = Vec::new();
        if let Some(bm) = &self.bitmap {
            // [ACK-BITMAP] largest + bit=1 olan her seq (artan); çıktı ≤ ACK_BITMAP_MAX_BITS + 1.
            let largest_full = crate::seq::resolve_seq(expected, self.largest_seq);
            let span = u64::from(self.largest_seq.wrapping_sub(bm.base_seq));
            let base = largest_full.saturating_sub(span);
            for i in 0..span.min(ACK_BITMAP_MAX_BITS) {
                let k = usize::try_from(i).unwrap_or(0);
                if bm
                    .bits
                    .get(k / 8)
                    .is_some_and(|b| b & (0x80 >> (k % 8)) != 0)
                {
                    acked.push(base + i);
                }
            }
            acked.push(largest_full);
            return acked;
        }
        if self.block0_len == 0 {
            return acked;
        }
        let largest_full = crate::seq::resolve_seq(expected, self.largest_seq);
        // block0 = [largest_full − block0_len + 1 ..= largest_full]
        let mut cursor = largest_full.saturating_sub(u64::from(self.block0_len) - 1);
        push_inclusive(&mut acked, cursor, largest_full);
        acked.truncate(MAX_ACKED_SEQS); // block0 tek başına (≤65535) tavanı aşamaz; savunmacı.
                                        // alt bloklar (descending, §2): cursor −= gap; block = [cursor−len+1 .. cursor]; cursor −= len.
        for r in &self.ranges {
            cursor = cursor.saturating_sub(u64::from(r.gap));
            if r.len == 0 {
                continue;
            }
            let low = cursor.saturating_sub(u64::from(r.len) - 1);
            push_inclusive(&mut acked, low, cursor);
            cursor = cursor.saturating_sub(u64::from(r.len));
            // [SR-AUDIT-001 / P3 derinlemesine-savunma] toplam çıktı tavanı: range_count decode'da
            // ≤16 olsa da her `len` u16 (≤65535) → 17 blok yasal olarak ~1.1M seq üretebilir. Tek
            // wire ACK'inin sender'a taşıdığı YENİ ack sayısı inflight-penceresiyle sınırlıdır;
            // SACK KÜMÜLATİF olduğundan tavana çarpınca kesmek KAYIP DEĞİL (en yüksek/en yeni seq'ler
            // önce — block0 descending — konur; kesilen en-eski seq'ler sonraki ACK'te yeniden kapsanır,
            // bkz. bu fn'in doc'u). Düşman `len`-şişirmesini bounded transient'e indirger.
            if acked.len() >= MAX_ACKED_SEQS {
                acked.truncate(MAX_ACKED_SEQS);
                break;
            }
        }
        acked
    }

    /// Canonical fixed-width big-endian byte'lara serialize eder.
    /// V2 (0x14): 15B sabit + 4B×range (§2.2 ölçüm eki dahil); V1 (0x10): 9B + 4B×range.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        if let Some(bm) = &self.bitmap {
            // [ACK-BITMAP] 0x15 (fixed-width; opsiyonel alan yok).
            debug_assert!(
                bm.bits.len() <= ACK_BITMAP_MAX_BYTES,
                "bitmap tavanı aşıldı"
            );
            let mut out = Vec::with_capacity(ACK_BITMAP_HEADER_BYTES + bm.bits.len());
            out.push(if bm.ptt_lean && bm.ptt_coalesce {
                FRAME_TYPE_ACK_BITMAP_PTT_COALESCE
            } else if bm.ptt_lean {
                FRAME_TYPE_ACK_BITMAP_PTT
            } else {
                FRAME_TYPE_ACK_BITMAP
            });
            out.push(self.ack_epoch);
            out.extend_from_slice(&self.largest_seq.to_be_bytes());
            out.extend_from_slice(&self.ack_delay.to_be_bytes());
            out.extend_from_slice(&self.arrival_rate.to_be_bytes());
            out.extend_from_slice(&bm.rx_span.to_be_bytes());
            out.extend_from_slice(&bm.span_first.to_be_bytes());
            out.extend_from_slice(&bm.pair_rate.to_be_bytes());
            out.extend_from_slice(&bm.base_seq.to_be_bytes());
            out.push(bm.bits.len() as u8);
            out.extend_from_slice(&bm.bits);
            return out;
        }
        debug_assert!(
            self.ranges.len() <= MAX_ACK_RANGES,
            "range sayısı MAX_ACK_RANGES'i aşıyor"
        );
        let mut out = Vec::with_capacity(15 + 4 * self.ranges.len());
        out.push(if self.v2 {
            FRAME_TYPE_ACK_V2
        } else {
            FRAME_TYPE_ACK
        });
        out.push(self.ack_epoch);
        out.extend_from_slice(&self.largest_seq.to_be_bytes());
        out.extend_from_slice(&self.ack_delay.to_be_bytes());
        out.extend_from_slice(&self.block0_len.to_be_bytes());
        out.push(self.ranges.len() as u8);
        for r in &self.ranges {
            out.extend_from_slice(&r.gap.to_be_bytes());
            out.extend_from_slice(&r.len.to_be_bytes());
        }
        if self.v2 {
            // §2.2 V2 ölçüm eki (fixed-width; opsiyonel alan YOK).
            out.extend_from_slice(&self.seen_count.to_be_bytes());
            out.extend_from_slice(&self.arrival_rate.to_be_bytes());
        }
        out
    }

    /// Canonical byte'ları çözer; truncated ve trailing girdiyi reddeder
    /// (PARSER-02: bounds-before-read, untrusted len ile with_capacity yok).
    ///
    /// # Errors
    /// Buffer kısa ([`TransportError::Truncated`]), frame_type yanlış
    /// ([`TransportError::UnknownFrameType`]) veya sonda artan byte varsa
    /// ([`TransportError::TrailingBytes`]).
    pub fn decode(buf: &[u8]) -> Result<Self, TransportError> {
        let mut r = Reader::new(buf);
        let ft = r.u8()?;
        // [TUR-1x] V2 (0x14, 256µs) VE V1 (0x10, µs — uyumluluk) kabul; layout aynı.
        if ft != FRAME_TYPE_ACK && ft != FRAME_TYPE_ACK_V2 {
            return Err(TransportError::UnknownFrameType(ft));
        }
        let ack_epoch = r.u8()?;
        let largest_seq = r.u16_be()?;
        let ack_delay = r.u16_be()?;
        let block0_len = r.u16_be()?;
        let range_count = usize::from(r.u8()?);
        // [SR-AUDIT-001 / P3 sertleştirme] range_count PROTOKOL tavanıyla sınırlanır. Encode ≤16
        // garanti eder (from_received clamp + encode debug_assert); doğrulanmamış range_count (u8→255)
        // acked_full'de ~16.7M-girişlik Vec<u64> (~134MB) amplifikasyonuna yol açardı (fail-closed,
        // decode reddi — with_capacity zaten yoktu ama asıl amplifikasyon acked_full'deydi).
        if range_count > MAX_ACK_RANGES {
            return Err(TransportError::TooManyAckRanges {
                count: range_count,
                max: MAX_ACK_RANGES,
            });
        }
        let mut ranges = Vec::new(); // count doğrulandı → yine de with_capacity yok (savunmacı)
        for _ in 0..range_count {
            let gap = r.u16_be()?;
            let len = r.u16_be()?;
            ranges.push(AckRange { gap, len });
        }
        let v2 = ft == FRAME_TYPE_ACK_V2;
        // §2.2 V2 ölçüm eki; V1'de alan yok → nötr değerler (0 / sentinel).
        let (seen_count, arrival_rate) = if v2 {
            (r.u32_be()?, r.u16_be()?)
        } else {
            (0, ARRIVAL_RATE_SENTINEL)
        };
        if r.remaining() != 0 {
            return Err(TransportError::TrailingBytes {
                count: r.remaining(),
            });
        }
        Ok(AckFrame {
            ack_epoch,
            largest_seq,
            ack_delay,
            block0_len,
            ranges,
            v2,
            seen_count,
            arrival_rate,
            bitmap: None,
        })
    }

    /// [ACK-BITMAP · ölçüm prototipi] [`Self::decode`] + 0x15 bitmap çerçevesi. Yalnız bitmap
    /// biçimi AÇIKÇA yapılandırılmış uç çağırır; [`Self::decode`] 0x15'i bugünkü gibi
    /// `UnknownFrameType` ile reddeder (ACK referans vektörleri bunu sabitler).
    ///
    /// # Errors
    /// [`Self::decode`] hataları; 0x15'te ayrıca [`TransportError::InvalidAckBitmap`].
    pub fn decode_with_bitmap(buf: &[u8]) -> Result<Self, TransportError> {
        if let Some(
            &t @ (FRAME_TYPE_ACK_BITMAP
            | FRAME_TYPE_ACK_BITMAP_PTT
            | FRAME_TYPE_ACK_BITMAP_PTT_COALESCE),
        ) = buf.first()
        {
            let mut r = Reader::new(buf);
            let _ = r.u8()?;
            let mut f = Self::decode_bitmap(r)?;
            if let Some(bm) = f.bitmap.as_mut() {
                bm.ptt_lean =
                    t == FRAME_TYPE_ACK_BITMAP_PTT || t == FRAME_TYPE_ACK_BITMAP_PTT_COALESCE;
                bm.ptt_coalesce = t == FRAME_TYPE_ACK_BITMAP_PTT_COALESCE;
            }
            return Ok(f);
        }
        Self::decode(buf)
    }

    /// [ACK-BITMAP] 0x15 gövdesini çözer (tür baytı okunmuş). Bounds-before-read; bitmap tavanı,
    /// kanonik uzunluk, dolgu bitleri ve trailing denetlenir (fail-closed).
    fn decode_bitmap(mut r: Reader<'_>) -> Result<Self, TransportError> {
        let ack_epoch = r.u8()?;
        let largest_seq = r.u16_be()?;
        let ack_delay = r.u16_be()?;
        let arrival_rate = r.u16_be()?;
        let rx_span = r.u16_be()?;
        let span_first = r.u16_be()?;
        let pair_rate = r.u16_be()?;
        let base_seq = r.u16_be()?;
        let len = usize::from(r.u8()?);
        if len > ACK_BITMAP_MAX_BYTES {
            return Err(TransportError::InvalidAckBitmap("bitmap_len tavanı aşıyor"));
        }
        let span = u64::from(largest_seq.wrapping_sub(base_seq));
        if span > ACK_BITMAP_MAX_BITS {
            return Err(TransportError::InvalidAckBitmap(
                "base_seq–largest_seq açıklığı tavanı aşıyor",
            ));
        }
        if u64::try_from(len).unwrap_or(u64::MAX) != span.div_ceil(8) {
            return Err(TransportError::InvalidAckBitmap(
                "bitmap_len açıklıkla tutarsız",
            ));
        }
        let bits = r.take(len)?.to_vec(); // bounds-before-read (len ≤ ACK_BITMAP_MAX_BYTES doğrulandı)
        let used = span % 8;
        if used != 0 {
            let pad_mask = 0xFFu8 >> used; // kullanılmayan alt bitler
            if bits.last().is_some_and(|b| b & pad_mask != 0) {
                return Err(TransportError::InvalidAckBitmap(
                    "dolgu bitleri sıfır değil",
                ));
            }
        }
        if r.remaining() != 0 {
            return Err(TransportError::TrailingBytes {
                count: r.remaining(),
            });
        }
        Ok(AckFrame {
            ack_epoch,
            largest_seq,
            ack_delay,
            block0_len: 0,
            ranges: Vec::new(),
            v2: false,
            seen_count: 0,
            arrival_rate,
            bitmap: Some(AckBitmap {
                base_seq,
                bits,
                rx_span,
                span_first,
                pair_rate,
                ptt_lean: false,
                ptt_coalesce: false,
            }),
        })
    }
}

/// Full-uzay blok uzunluğunu u16'ya clamp'ler (degenerate >2^16 blok savunması).
fn block_len_u16(low: u64, high: u64) -> u16 {
    (high - low + 1).min(u64::from(u16::MAX)) as u16
}

/// `[low, high]` bloğu u16 `len` alanına sığmıyor mu (uzunluk > 65535; [`block_len_u16`] kırpar).
fn block_len_clamped(low: u64, high: u64) -> bool {
    high - low >= u64::from(u16::MAX)
}

/// [low, high] kapsayıcı full-counter aralığını çıkışa ekler.
fn push_inclusive(out: &mut Vec<u64>, low: u64, high: u64) {
    for s in low..=high {
        out.push(s);
    }
}

#[cfg(test)]
mod tests {
    /// [PTT-YALIN · YETENEK] 0x16, 0x15 ile bayt-aynı; yalnız tür baytı ve çözülen bayrak farklı.
    #[test]
    fn bitmap_ptt_capability_type_roundtrips() {
        let f = AckFrame::bitmap_from_received(
            [3u64, 5, 7],
            9,
            0,
            1,
            0,
            ARRIVAL_RATE_SENTINEL,
            RX_SPAN_NONE,
            0,
        );
        let plain = f.encode();
        let mut lean = f.clone();
        lean.bitmap.as_mut().expect("bitmap").ptt_lean = true;
        let tagged = lean.encode();
        assert_eq!(plain[0], FRAME_TYPE_ACK_BITMAP);
        assert_eq!(tagged[0], FRAME_TYPE_ACK_BITMAP_PTT);
        assert_eq!(plain[1..], tagged[1..], "gövde bayt-aynı");
        let d = AckFrame::decode_with_bitmap(&tagged).expect("çözülür");
        assert!(d.bitmap.as_ref().is_some_and(|b| b.ptt_lean));
        let d = AckFrame::decode_with_bitmap(&plain).expect("çözülür");
        assert!(d.bitmap.as_ref().is_some_and(|b| !b.ptt_lean));
        assert!(
            AckFrame::decode(&tagged).is_err(),
            "düz çözücü 0x16'yı reddeder"
        );
        // [PTT-BİRLEŞİK · deney] 0x17: gövde aynı, iki bayrak birden; düz çözücü reddeder.
        let mut both = lean.clone();
        both.bitmap.as_mut().expect("bitmap").ptt_coalesce = true;
        let coalesce = both.encode();
        assert_eq!(coalesce[0], FRAME_TYPE_ACK_BITMAP_PTT_COALESCE);
        assert_eq!(plain[1..], coalesce[1..], "gövde bayt-aynı");
        let d = AckFrame::decode_with_bitmap(&coalesce).expect("çözülür");
        assert!(d
            .bitmap
            .as_ref()
            .is_some_and(|b| b.ptt_lean && b.ptt_coalesce));
        assert!(AckFrame::decode(&coalesce).is_err());
        // Yalın bayrak olmadan birleşik bayrak türü değiştirmez (0x15 kalır).
        let mut only = f.clone();
        only.bitmap.as_mut().expect("bitmap").ptt_coalesce = true;
        assert_eq!(only.encode(), plain);
    }

    use super::*;

    /// [P3 SR-AUDIT-001] Düşman çerçeve: range_count > MAX_ACK_RANGES decode'da typed reddedilir
    /// (denetim bulgusu: doğrulanmamış range_count → acked_full ~134MB amplifikasyonu).
    #[test]
    fn decode_rejects_range_count_over_max() {
        // 255 range bildiren ama gövdesi kısa/uzun fark etmez — reddin range_count'ta olması esas.
        let mut buf = vec![FRAME_TYPE_ACK_V2, 0]; // ft, epoch
        buf.extend_from_slice(&0xFFFFu16.to_be_bytes()); // largest_seq
        buf.extend_from_slice(&0u16.to_be_bytes()); // ack_delay
        buf.extend_from_slice(&0xFFFFu16.to_be_bytes()); // block0_len
        buf.push(255); // range_count = 255 (> 16)
        buf.extend(std::iter::repeat_n(0u8, 255 * 4)); // 255 range gövdesi
        buf.extend_from_slice(&0u32.to_be_bytes()); // seen_count
        buf.extend_from_slice(&0u16.to_be_bytes()); // arrival_rate
        match AckFrame::decode(&buf) {
            Err(TransportError::TooManyAckRanges {
                count: 255,
                max: 16,
            }) => {}
            other => panic!("255-range çerçeve reddedilmeliydi: {other:?}"),
        }
        // Sınır: tam 16 range GEÇERLİ (encode'un ürettiği maksimum).
        let recv: Vec<u64> = (0..40).map(|i| i * 3).collect(); // 40 ayrık blok → 16'ya clamp
        let f = AckFrame::from_received(&recv, 0, 0, 0, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        assert!(f.ranges.len() <= MAX_ACK_RANGES);
        assert!(
            AckFrame::decode(&f.encode()).is_ok(),
            "16-range çerçeve geçerli olmalı"
        );
    }

    /// [P3 SR-AUDIT-001] `acked_full` çıktısı, geçerli range sayısında bile dev `len` alanlarıyla
    /// MAX_ACKED_SEQS'i aşamaz (derinlemesine savunma; `len`-şişirme amplifikasyonu).
    #[test]
    fn acked_full_is_bounded_against_len_inflation() {
        // 16 range, her biri gap=0 len=0xFFFF; block0_len=0xFFFF; büyük expected tabanı.
        let ranges = vec![
            AckRange {
                gap: 0,
                len: 0xFFFF
            };
            MAX_ACK_RANGES
        ];
        let f = AckFrame {
            ack_epoch: 0,
            largest_seq: 0xFFFF,
            ack_delay: 0,
            block0_len: 0xFFFF,
            ranges,
            v2: true,
            seen_count: 0,
            arrival_rate: 0,
            bitmap: None,
        };
        let acked = f.acked_full(u64::MAX / 2); // büyük taban → tüm bloklar dolar
        assert!(
            acked.len() <= MAX_ACKED_SEQS,
            "acked_full tavanı aşmamalı: {} > {MAX_ACKED_SEQS}",
            acked.len()
        );
    }

    #[test]
    fn acked_full_roundtrips_from_received() {
        // Alınan set (gaps 8,9,11): from_received → encode → decode → acked_full == set.
        let recv = [5u64, 6, 7, 10, 12, 13];
        let f = AckFrame::from_received(&recv, 0, 100, 6, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        let bytes = f.encode();
        let f2 = AckFrame::decode(&bytes).expect("decode");
        let mut got = f2.acked_full(14); // expected = largest+1
        got.sort_unstable();
        assert_eq!(got, recv, "acked_full, from_received'in tersi olmalı (§2)");
    }

    #[test]
    fn acked_full_single_contiguous_block() {
        let recv = [100u64, 101, 102, 103];
        let f = AckFrame::from_received(&recv, 0, 0, 4, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        assert!(f.ranges.is_empty(), "tek blok → range yok");
        let mut got = f.acked_full(104);
        got.sort_unstable();
        assert_eq!(got, recv);
    }

    #[test]
    fn acked_full_empty_on_zero_block0() {
        let f = AckFrame {
            ack_epoch: 0,
            largest_seq: 0,
            ack_delay: 0,
            block0_len: 0,
            ranges: Vec::new(),
            v2: true,
            seen_count: 0,
            arrival_rate: ARRIVAL_RATE_SENTINEL,
            bitmap: None,
        };
        assert!(f.acked_full(1).is_empty(), "block0_len 0 → boş");
    }

    #[test]
    fn ack_delay_unit_per_version() {
        // [TUR-1x] Aynı raw değer: V2 → ×256µs, V1 → ×1µs (decode-uyumluluk).
        let mut f =
            AckFrame::from_received(&[1], 0, 100, 1, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        assert!(f.v2, "emit daima V2");
        assert_eq!(f.ack_delay_duration(), Duration::from_micros(100 * 256));
        f.v2 = false;
        assert_eq!(f.ack_delay_duration(), Duration::from_micros(100));
    }

    #[test]
    fn decode_v1_frame_type_compat() {
        // V1 golden byte'ları (0x10, 9B gövde — ölçüm eki YOK) hâlâ decode edilir; µs semantiği.
        let mut bytes = AckFrame::from_received(&[5, 6, 7], 0, 40, 3, ARRIVAL_RATE_SENTINEL)
            .expect("nonempty")
            .encode();
        assert_eq!(bytes[0], FRAME_TYPE_ACK_V2, "emit V2");
        bytes[0] = FRAME_TYPE_ACK;
        bytes.truncate(9); // V1 gövdesi 9B — V2 ölçüm eki V1'de taşınmaz.
        let f = AckFrame::decode(&bytes).expect("V1 decode-uyumlu");
        assert!(!f.v2);
        assert_eq!(f.ack_delay_duration(), Duration::from_micros(40));
        assert_eq!(f.seen_count, 0, "V1'de alan yok → nötr");
        assert_eq!(f.arrival_rate, ARRIVAL_RATE_SENTINEL);
    }

    // ── [ACK-BITMAP · ölçüm prototipi] ─────────────────────────────────────────────────────

    /// Bağımsız model: [base, largest] içinde alınanlar + largest.
    fn model(recv: &[u64], base: u64, largest: u64) -> Vec<u64> {
        let mut v: Vec<u64> = recv
            .iter()
            .copied()
            .filter(|&s| s >= base && s <= largest)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    #[test]
    fn bitmap_roundtrip_semantic_equality() {
        for &(base, largest) in &[
            (0u64, 0u64),
            (0, 7),
            (0, 8),
            (3, 20),
            (100, 596),
            (1000, 1001),
        ] {
            let recv: Vec<u64> = (base..=largest)
                .filter(|s| s % 3 != 1 || *s == largest)
                .collect();
            let f = AckFrame::bitmap_from_received(
                recv.iter().copied(),
                largest,
                base,
                0,
                77,
                6_250,
                1_234,
                base,
            );
            let mut f = f;
            if let Some(bm) = f.bitmap.as_mut() {
                bm.pair_rate = 12_345;
            }
            let bytes = f.encode();
            assert_eq!(bytes[0], FRAME_TYPE_ACK_BITMAP);
            assert_eq!(bytes.len(), f.frame_len());
            let g = AckFrame::decode_with_bitmap(&bytes).expect("decode");
            assert_eq!(g, f, "gidiş-dönüş yapısal eşitlik");
            assert_eq!(g.acked_full(largest + 1), model(&recv, base, largest));
            assert_eq!(g.evidence_span_full(largest + 1), Some((base, largest)));
            assert_eq!(g.ack_delay_duration(), Duration::from_micros(77 * 256));
            assert!(!g.v2 && g.ranges.is_empty() && g.block0_len == 0);
        }
    }

    #[test]
    fn bitmap_window_is_clamped_to_the_wire_ceiling() {
        let largest = 5_000u64;
        let recv: Vec<u64> = (0..=largest).collect();
        let f = AckFrame::bitmap_from_received(
            recv.iter().copied(),
            largest,
            0,
            0,
            0,
            ARRIVAL_RATE_SENTINEL,
            RX_SPAN_NONE,
            0,
        );
        let bm = f.bitmap.as_ref().expect("bitmap");
        assert_eq!(bm.bits.len(), ACK_BITMAP_MAX_BYTES);
        assert_eq!(
            f.evidence_span_full(largest + 1),
            Some((largest - ACK_BITMAP_MAX_BITS, largest))
        );
        // Tel boyu tavanı: zarf + çerçeve ≤ bugünkü en büyük ACK datagramı (129 B).
        assert_eq!(
            ACK_DATAGRAM_ENVELOPE_BYTES + f.encode().len(),
            ACK_DATAGRAM_MAX_LINE_BYTES
        );
        // Bugünkü en büyük V2 (16 aralık) ile aynı tavan.
        let v2: Vec<u64> = (0..40).map(|i| i * 3).collect();
        let v2f = AckFrame::from_received(&v2, 0, 0, 0, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        assert_eq!(v2f.ranges.len(), MAX_ACK_RANGES);
        assert_eq!(
            ACK_DATAGRAM_ENVELOPE_BYTES + v2f.encode().len(),
            ACK_DATAGRAM_MAX_LINE_BYTES
        );
        assert_eq!(v2f.frame_len(), v2f.encode().len());
    }

    #[test]
    fn bitmap_wraps_across_u16() {
        let largest = 0x1_0005u64;
        let base = 0xFFF0u64;
        let recv: Vec<u64> = (base..=largest)
            .filter(|s| s % 2 == 0 || *s == largest)
            .collect();
        let f = AckFrame::bitmap_from_received(
            recv.iter().copied(),
            largest,
            base,
            0,
            0,
            0,
            RX_SPAN_NONE,
            0,
        );
        let g = AckFrame::decode_with_bitmap(&f.encode()).expect("decode");
        assert_eq!(g.largest_seq, 0x0005);
        assert_eq!(g.bitmap.as_ref().map(|b| b.base_seq), Some(0xFFF0));
        assert_eq!(g.acked_full(largest + 1), model(&recv, base, largest));
        assert_eq!(g.evidence_span_full(largest + 1), Some((base, largest)));
    }

    #[test]
    fn bitmap_decode_rejects_hostile_input() {
        let good = AckFrame::bitmap_from_received(
            [0u64, 2, 9],
            9,
            0,
            0,
            0,
            ARRIVAL_RATE_SENTINEL,
            RX_SPAN_NONE,
            0,
        )
        .encode();
        // Kesik: her önek reddedilir.
        for n in 0..good.len() {
            assert!(
                AckFrame::decode_with_bitmap(&good[..n]).is_err(),
                "kesik {n} reddedilmeli"
            );
        }
        // Aşırı uzun (trailing).
        let mut long = good.clone();
        long.push(0);
        assert!(matches!(
            AckFrame::decode_with_bitmap(&long),
            Err(TransportError::TrailingBytes { count: 1 })
        ));
        // bitmap_len tavanı aşıyor.
        let mut over = vec![
            FRAME_TYPE_ACK_BITMAP,
            0,
            0x02,
            0x40,
            0,
            0,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0,
            0,
            0xFF,
            0xFF,
            0,
            0,
        ];
        over.push(65);
        over.extend(std::iter::repeat_n(0u8, 65));
        assert!(matches!(
            AckFrame::decode_with_bitmap(&over),
            Err(TransportError::InvalidAckBitmap(_))
        ));
        // base > largest (sarma yok): açıklık mod 2^16 ≈ 65 k > tavan.
        let mut inv = vec![
            FRAME_TYPE_ACK_BITMAP,
            0,
            0x00,
            0x10,
            0,
            0,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0,
            0,
            0xFF,
            0xFF,
            0x00,
            0x20,
            0,
        ];
        assert!(matches!(
            AckFrame::decode_with_bitmap(&inv),
            Err(TransportError::InvalidAckBitmap(_))
        ));
        // Uzunluk açıklıkla tutarsız (span 9 → 2 B gerekir, 1 B verildi).
        inv = vec![
            FRAME_TYPE_ACK_BITMAP,
            0,
            0x00,
            0x09,
            0,
            0,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0,
            0,
            0xFF,
            0xFF,
            0x00,
            0x00,
            1,
            0xFF,
        ];
        assert!(matches!(
            AckFrame::decode_with_bitmap(&inv),
            Err(TransportError::InvalidAckBitmap(_))
        ));
        // Dolgu biti 1 (span 9 → ikinci baytta yalnız MSB anlamlı).
        inv = vec![
            FRAME_TYPE_ACK_BITMAP,
            0,
            0x00,
            0x09,
            0,
            0,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0,
            0,
            0xFF,
            0xFF,
            0x00,
            0x00,
            2,
            0xFF,
            0xC0,
        ];
        assert!(matches!(
            AckFrame::decode_with_bitmap(&inv),
            Err(TransportError::InvalidAckBitmap(_))
        ));
        inv[18] = 0x80;
        assert!(
            AckFrame::decode_with_bitmap(&inv).is_ok(),
            "aynı çerçeve dolgusuz geçerli"
        );
    }

    #[test]
    fn plain_decode_still_rejects_0x15() {
        let b = AckFrame::bitmap_from_received(
            [0u64, 2, 9],
            9,
            0,
            0,
            0,
            ARRIVAL_RATE_SENTINEL,
            RX_SPAN_NONE,
            0,
        )
        .encode();
        assert!(matches!(
            AckFrame::decode(&b),
            Err(TransportError::UnknownFrameType(0x15))
        ));
        // 0x14 her iki çözücüde aynı sonucu verir.
        let v = AckFrame::from_received(&[5, 6, 9], 0, 0, 3, ARRIVAL_RATE_SENTINEL)
            .expect("x")
            .encode();
        assert_eq!(AckFrame::decode(&v), AckFrame::decode_with_bitmap(&v));
    }

    #[test]
    fn v2_path_has_no_evidence_span() {
        let f = AckFrame::from_received(&[5, 6, 9], 0, 0, 3, ARRIVAL_RATE_SENTINEL).expect("x");
        assert_eq!(f.bitmap, None);
        assert_eq!(f.evidence_span_full(10), None);
        assert_eq!(f.encode()[0], FRAME_TYPE_ACK_V2);
    }

    /// `from_received` çerçevesinin tel üzerinden (encode → decode → `acked_full`) teyit kümesi.
    fn wire_acked(recv: &[u64]) -> Vec<u64> {
        let f = AckFrame::from_received(recv, 0, 0, 0, ARRIVAL_RATE_SENTINEL).expect("nonempty");
        let d = AckFrame::decode(&f.encode()).expect("çözülür");
        let expected = recv.iter().max().expect("nonempty") + 1;
        d.acked_full(expected)
    }

    /// [S-23 / A4-01] u16'ya sığmayan boşluk kırpılmaz: {0, 70000} çerçevesi 70000 dışında hiçbir
    /// seq'i teyit etmez (eski kod gap'i 4464'e kırpıp hiç alınmamış 65536'yı teyit ediyordu).
    #[test]
    fn gap_over_u16_stops_older_ranges_instead_of_false_ack() {
        let f = AckFrame::from_received(&[0, 70_000], 0, 0, 0, ARRIVAL_RATE_SENTINEL).expect("x");
        assert!(
            f.ranges.is_empty(),
            "sığmayan boşluktan sonraki aralık yazılmaz"
        );
        assert_eq!(wire_acked(&[0, 70_000]), vec![70_000]);
        // Sığan boşluk yazılır, sonraki sığmayan boşlukta durulur.
        let recv = [1u64, 80_000, 80_005];
        let acked = wire_acked(&recv);
        assert!(
            acked.iter().all(|s| recv.contains(s)),
            "yanlış teyit: {acked:?}"
        );
        assert_eq!(acked, vec![80_005, 80_000]);
    }

    /// [S-23 / A4-01] u16'ya sığmayan blok yalnız en üst 65535 seq'iyle yazılır ve daha eski
    /// aralıklar yazılmaz; block0 kırpılırsa hiç alt aralık yazılmaz. Teyit kümesi ⊆ alınan küme.
    #[test]
    fn block_over_u16_stops_older_ranges_instead_of_false_ack() {
        // Alt blok kırpılır: {100000} ∪ [100002, 170002) ∪ {170003}.
        let mut recv = vec![100_000u64];
        recv.extend(100_002..170_002);
        recv.push(170_003);
        let acked = wire_acked(&recv);
        assert!(
            acked.iter().all(|s| recv.binary_search(s).is_ok()),
            "yanlış teyit"
        );
        assert!(!acked.contains(&100_000), "kırpılmış bloğun altı yazılmaz");
        assert_eq!(acked.len(), 1 + usize::from(u16::MAX));
        // block0 kırpılır: [10, 70011) ∪ {0}.
        let mut recv = vec![0u64];
        recv.extend(10..70_011);
        let f = AckFrame::from_received(&recv, 0, 0, 0, ARRIVAL_RATE_SENTINEL).expect("x");
        assert!(
            f.ranges.is_empty(),
            "kırpılmış block0'ın altına aralık yazılmaz"
        );
        let acked = wire_acked(&recv);
        assert!(
            acked.iter().all(|s| recv.binary_search(s).is_ok()),
            "yanlış teyit"
        );
        assert_eq!(acked.len(), usize::from(u16::MAX));
    }
}
