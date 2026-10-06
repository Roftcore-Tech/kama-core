//! İki seam'in ortak tuttuğu tipler.
//!
//! [`SessionHandle`] Crypto'da basılır (S3 K8), Transport onu yorumlamadan Application'a iletir
//! (S1 K2). [`PeerRef`] Application'da doğar ve Transport üzerinden Crypto'ya iner. [`CloseReason`]
//! iki seam'in `Closed` olayında da aynıdır. Üçü de her katmanda AYNI tip olmalıdır; bu yüzden ne
//! `s1`'e ne `s3`'e aittirler.

use core::fmt;

/// Oturum tutamacı.
///
/// OPAKTIR: içeriği yorumlanmaz, epoch DEĞİLDİR, wire'da gönderilmez, `Closed` sonrası
/// kullanılamaz (yaşam döngüsü invariantları transport-tdd §0). Wire routing değeri olan
/// [`crate::s3::HubHandle`] ile AYRI tiptir (ccr §35.7); biri diğerinin yerine geçemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionHandle(u64);

impl SessionHandle {
    /// Tutamaç basar. Yalnız basan taraf çağırır: gerçek sistemde Crypto, testlerde sahteleri.
    #[must_use]
    pub const fn from_raw(v: u64) -> Self {
        Self(v)
    }

    /// Ham değer — YALNIZ tutamacı basan sahte içindir. Transport ve Application ÇAĞIRMAZ:
    /// tutamaç onlar için opaktır.
    #[cfg(feature = "test-util")]
    #[must_use]
    pub const fn to_raw_for_mock(self) -> u64 {
        self.0
    }
}

/// Eşin opak referansı: `dest_hash`'in (cihaz kimliğinin 32 baytlık özeti, ccr-format-spec)
/// sarmalayıcısı.
///
/// Application eşi bununla adlandırır; Transport ve Crypto onu peer material'ı bulmak için
/// anahtar olarak kullanır. Alan public DEĞİLDİR; ham baytlar [`PeerRef::as_bytes`] ile okunur,
/// yorumlanmaz.
///
/// `Debug` çıktısı `dest_hash`'i BASMAZ: bu değer wire'da cleartext taşınmayan bir kimlik
/// özetidir ve log'a düşmemelidir.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PeerRef([u8; 32]);

impl PeerRef {
    /// `dest_hash`'ten eş referansı kurar.
    #[must_use]
    pub const fn from_dest_hash(dest_hash: [u8; 32]) -> Self {
        Self(dest_hash)
    }

    /// Ham `dest_hash`. Opaklık = YORUMLAMAMA: Application bu baytları yorumlayamaz/türetemez.
    /// Katman kuralı (`scripts/ci/layer_check.py`) crate kenarlarını ve `*_for_mock` erişimlerini
    /// yakalar; `as_bytes` çıktısının yorumlanmaması inceleme kuralıdır (bugün CI kapısı yok — iş
    /// emri F0.3). [2026-10-06 F0.3]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Ham `dest_hash` — YALNIZ sahteler içindir (oturum tutamacını eşten türetmek gibi).
    #[cfg(feature = "test-util")]
    #[must_use]
    pub const fn to_raw_for_mock(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for PeerRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Ürün derlemesinde hiçbir bayt basılmaz. Test derlemesinde (sentetik eşler) teşhis için
        // ilk dört bayt basılır.
        #[cfg(feature = "test-util")]
        {
            write!(
                f,
                "PeerRef({:02x}{:02x}{:02x}{:02x}…)",
                self.0[0], self.0[1], self.0[2], self.0[3]
            )
        }
        #[cfg(not(feature = "test-util"))]
        {
            f.write_str("PeerRef(<redacted>)")
        }
    }
}

/// Oturum kapanış nedeni — küme KAPALIDIR (iş emri 2026-10-05; önceki `Renewed` kalktı).
///
/// Zaman bir kapanış nedeni DEĞİLDİR (INV-NO-IDLE-TEARDOWN, transport-tdd §0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloseReason {
    /// Yeni oturum eskisinin yerini aldı: provisional supersession onaylandı (DR-CONNECT-1 H6) ya da
    /// yenilemede atomik cutover tamamlandı (S1 KARAR-2). İkisi aynı yoldur.
    Superseded,
    /// Yerel kullanıcı/uygulama kapattı.
    UserClosed,
    /// Karşı taraf kapattı.
    PeerClosed,
    /// Anahtar hijyeni.
    KeyHygiene,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Yüzey pini: bir variant eklenir/çıkarılırsa bu eşleşme derlenmez (kapalı küme).
    #[test]
    fn close_reason_set_is_closed() {
        match CloseReason::Superseded {
            CloseReason::Superseded
            | CloseReason::UserClosed
            | CloseReason::PeerClosed
            | CloseReason::KeyHygiene => {}
        }
    }

    #[test]
    fn peer_ref_is_an_opaque_wrapper_of_dest_hash() {
        let (a, b) = ([0xA5u8; 32], [0x5Au8; 32]);
        assert_eq!(PeerRef::from_dest_hash(a), PeerRef::from_dest_hash(a));
        assert_ne!(PeerRef::from_dest_hash(a), PeerRef::from_dest_hash(b));
        assert_eq!(PeerRef::from_dest_hash(a).as_bytes(), &a);
    }

    /// `Debug` çıktısı `dest_hash`'in tamamını asla içermez.
    #[test]
    fn peer_ref_debug_does_not_print_the_dest_hash() {
        let mut h = [0u8; 32];
        for (i, b) in h.iter_mut().enumerate() {
            *b = 0x40 + i as u8;
        }
        let shown = format!("{:?}", PeerRef::from_dest_hash(h));
        let full: String = h.iter().map(|b| format!("{b:02x}")).collect();
        assert!(!shown.contains(&full), "Debug dest_hash'in tamamını bastı");
        // Son 28 bayttan hiçbir dört baytlık pencere çıktıda görünmez.
        for w in h[4..].windows(4) {
            let hex: String = w.iter().map(|b| format!("{b:02x}")).collect();
            assert!(
                !shown.contains(&hex),
                "Debug ilk dört bayttan fazlasını bastı"
            );
        }
    }
}
