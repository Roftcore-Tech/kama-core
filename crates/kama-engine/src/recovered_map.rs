//! DR-S5-4 (şart-2): RECOVERED-control logical→native çevrimi — **çevrim ENGINE'de** (SoC).
//!
//! `Control::Recovered { block_id, esi_ranges }` LOGICAL-referans taşır; transport-sender cancel'ı
//! native `unit_id` ile yapar (ACK-cancel/retention-release mekanizmalarının birebir-paraleli —
//! yeni-cancel-yolu İCAT-EDİLMEZ). Engine, kendi send-record'undan (emit-anı kayıt) `(block_id, esi)
//! → unit_id` eşlemesini tutar ve RECOVERED'ı çözer. BOUNDED (SR-AUDIT-001: cap-aşımında en-eski-drop).

use crate::error::EngineError;
use kama_transport_schema::control::Control;
use std::collections::{BTreeMap, VecDeque};

/// Eşleme-tavanı (bounded; sender TOMBSTONE_MAX/CONFIRMED_LOGICAL_CAP kalıbı).
pub const RECOVERED_MAP_CAP: usize = 4096;

/// Çözülmüş recovered-birim: native unit_id + logical-kimlik (transport `RecoveredUnit`'e eşlenir;
/// engine-üretimi adaptive-transport'a bağlanmaz — kompozisyon-kökü/test eşler).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedRecovered {
    pub unit_id: u64,
    pub block_id: u32,
    pub esi: u16,
}

/// Emit-anı `(block_id, esi) → unit_id` defteri + RECOVERED-çözücü.
#[derive(Debug, Default)]
pub struct RecoveredMap {
    map: BTreeMap<(u32, u16), u64>,
    order: VecDeque<(u32, u16)>,
    evicted: u64,
}

impl RecoveredMap {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
            order: VecDeque::new(),
            evicted: 0,
        }
    }

    /// Emit-anı kaydı: bu (block_id, esi) hangi native-unit ile gönderildi. BOUNDED: cap-aşımında
    /// en-eski-kayıt düşer (audit-sayaç). Aynı-anahtar yeniden-kayıt = güncelleme (en-son-emisyon).
    pub fn register(&mut self, block_id: u32, esi: u16, unit_id: u64) {
        let key = (block_id, esi);
        if self.map.insert(key, unit_id).is_none() {
            if self.order.len() >= RECOVERED_MAP_CAP {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                    self.evicted += 1;
                }
            }
            self.order.push_back(key);
        }
    }

    /// RECOVERED-control'ü çöz: esi_ranges (kapsayıcı) → kayıtlı-eşlemeler. Kayıtsız-esi SESSİZCE
    /// atlanmaz-değil: çözülen-liste + `unresolved` sayısı döner (teşhis; bilinmeyen-logical
    /// sender'da zaten NO-OP). Range-taraması BTreeMap::range ile (present-key sayısında; DoS-güvenli).
    ///
    /// # Errors
    /// [`EngineError::UnexpectedControl`] — Control::Recovered değilse.
    pub fn resolve(&self, ctl: &Control) -> Result<(Vec<ResolvedRecovered>, usize), EngineError> {
        let Control::Recovered {
            block_id,
            esi_ranges,
            ..
        } = ctl
        else {
            return Err(EngineError::UnexpectedControl);
        };
        let mut resolved = Vec::new();
        let mut requested: usize = 0;
        for &(lo, hi) in esi_ranges {
            let (lo, hi) = (lo.min(hi), lo.max(hi));
            requested += usize::from(hi - lo) + 1;
            for (&(_, esi), &unit_id) in self.map.range((*block_id, lo)..=(*block_id, hi)) {
                resolved.push(ResolvedRecovered {
                    unit_id,
                    block_id: *block_id,
                    esi,
                });
            }
        }
        let unresolved = requested.saturating_sub(resolved.len());
        Ok((resolved, unresolved))
    }

    /// Kayıtlı-eşleme sayısı.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Defter boş mu?
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Cap-aşımında düşürülen-kayıt sayısı (audit; bounded-kanıtı).
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_resolve_roundtrip() {
        let mut m = RecoveredMap::new();
        m.register(7, 0, 100);
        m.register(7, 1, 101);
        m.register(7, 3, 103);
        m.register(8, 0, 200); // farklı-blok karışmaz
        let ctl = Control::Recovered {
            id: 1,
            block_id: 7,
            esi_ranges: vec![(0, 1), (3, 3)],
        };
        let (res, unresolved) = m.resolve(&ctl).unwrap();
        assert_eq!(
            res,
            vec![
                ResolvedRecovered {
                    unit_id: 100,
                    block_id: 7,
                    esi: 0
                },
                ResolvedRecovered {
                    unit_id: 101,
                    block_id: 7,
                    esi: 1
                },
                ResolvedRecovered {
                    unit_id: 103,
                    block_id: 7,
                    esi: 3
                },
            ]
        );
        assert_eq!(unresolved, 0);
    }

    #[test]
    fn unresolved_counted_not_silent() {
        let mut m = RecoveredMap::new();
        m.register(7, 0, 100);
        let ctl = Control::Recovered {
            id: 1,
            block_id: 7,
            esi_ranges: vec![(0, 2)], // esi 1,2 kayıtsız
        };
        let (res, unresolved) = m.resolve(&ctl).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(unresolved, 2, "kayıtsız-esi'ler sayılır (sessiz-değil)");
    }

    #[test]
    fn non_recovered_control_rejected() {
        let m = RecoveredMap::new();
        let ack = Control::Ack {
            id: 1,
            acked_dh_epoch: 0,
            ack_delay_256us: 0,
            seen_count: 0,
            arrival_rate: 0xFFFF,
            ranges: vec![(0, 1)],
        };
        assert_eq!(m.resolve(&ack), Err(EngineError::UnexpectedControl));
    }

    #[test]
    fn bounded_eviction_oldest() {
        let mut m = RecoveredMap::new();
        for i in 0..(RECOVERED_MAP_CAP as u32 + 50) {
            m.register(i, 0, u64::from(i));
        }
        assert_eq!(m.len(), RECOVERED_MAP_CAP, "bounded: cap-aşılmaz");
        assert_eq!(m.evicted_count(), 50, "aşan-50 en-eski-drop (audit)");
        // En-eski (block 0..50) düştü; en-yeni duruyor.
        let ctl = Control::Recovered {
            id: 1,
            block_id: 0,
            esi_ranges: vec![(0, 0)],
        };
        assert_eq!(m.resolve(&ctl).unwrap().0.len(), 0, "en-eski evict-edildi");
    }
}
