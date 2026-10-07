//! `MockTransport` — Crypto testleri için sahte Transport (A4.3; KM-SEAM-TC-001 §7). YALNIZ TEST.
//!
//! Crypto'nun TEK üst komşusu Transport'tur (INV-NEIGHBOR-1). Bu modül o komşunun Crypto'ya
//! gösterdiği yüzü taklit eder: zarfları taşır, kaybeder, sıraya sokar, çoğaltır ve yeniden gönderir.
//! **Application'a ait HİÇBİR tip ya da import içermez** — `kama-app`, AMP, `KamaTransport` yok.
//!
//! # Sahtenin SADIK kaldığı sözleşme
//!
//! * **INV-INIT-KEY-1 / K9** — yeniden gönderim, saklanan zarfın BYTE-FOR-BYTE kopyasıdır. Transport
//!   zarfı yeniden ÜRETTİRMEZ; Crypto'ya ikinci bir `Initiate` çağırmaz.
//! * **K6** — komut TYPED'dır, zarf payload'ı OPAKTIR: sahte, `envelope_type`'a bakarak komut türünü
//!   seçer (`0x01` → `AcceptInit`, `0x02` → `AcceptResponse`) ve zarfın İÇİNİ asla yorumlamaz.
//! * Sınırlı kuyruk: dolunca typed `Backpressure`; sessiz düşürme yalnız AÇIKÇA enjekte edilen
//!   kayıptır (SR-AUDIT-001).
//!
//! # Komut tipi
//!
//! Sahte, Crypto'ya enjekte edilecek komutu S3'ün GERÇEK tipiyle verir
//! (`kama_contracts::s3::SessionCommand`). B1'den önce burada bir yerel ayna (`InjectedCommand`)
//! vardı; gerekçesi olarak yazılan "import edilirse bağımlılık döngüsü oluşur" iddiası YANLIŞTI
//! (dev-dependency döngüsüne Cargo izin verir; 2026-10-05'te ölçüldü). Tipler `kama-contracts`'a
//! taşınınca ayna kaldırıldı. Bu modül `kama_contracts::s1`'den (Application seam'i) HİÇBİR ŞEY
//! import etmez.

#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};

use kama_contracts::s3::SessionCommand;

/// app-rpc `STORE_MESSAGE.envelope_type` registry'si (AMENDMENT-7).
pub const ENVELOPE_INIT: u8 = 0x01;
pub const ENVELOPE_INIT_RESPONSE: u8 = 0x02;

/// Taşıma kuyruğu tavanı.
pub const MOCK_MAX_IN_FLIGHT: usize = 64;
/// Yeniden gönderim için saklanan zarf tavanı.
pub const MOCK_MAX_CACHED: usize = 64;

/// Saklanan zarfın kimliği (Transport-yerel; Crypto görmez).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EnvelopeId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockTransportError {
    /// Taşıma kuyruğu ya da zarf önbelleği dolu.
    Backpressure,
    /// Bilinmeyen (ya da bırakılmış) zarf kimliği.
    UnknownEnvelope,
    /// Tanımsız `envelope_type` — item DROP edilir (app-rpc: reserved/unknown → DROP).
    UnknownEnvelopeType,
}

/// Bir sonraki gönderime uygulanacak arıza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// Zarf yola çıkmaz (önbellekte KALIR — yeniden gönderilebilir).
    Drop,
    /// Zarf iki kez teslim edilir.
    Duplicate,
    /// Zarf, kuyruktaki bir öncekinin ÖNÜNE geçer.
    Reorder,
}

#[derive(Debug, Default)]
pub struct MockTransport {
    next_id: u64,
    /// Yeniden gönderim önbelleği: Crypto'nun ÜRETTİĞİ byte'lar, değiştirilmeden.
    cache: BTreeMap<EnvelopeId, (u8, Vec<u8>)>,
    /// Karşı uca doğru yoldaki zarflar.
    in_flight: VecDeque<(u8, Vec<u8>)>,
    faults: VecDeque<Fault>,
    pub sent: u64,
    pub dropped: u64,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sıradaki gönderim(ler) için arıza enjekte eder (FIFO; her gönderim bir arıza tüketir).
    pub fn inject(&mut self, fault: Fault) {
        self.faults.push_back(fault);
    }

    /// Crypto'nun `Initiated{session, envelope_type, bytes}` (s3 v1.1; eski `OutboundEnvelope`) ya da
    /// yanıt zarfını alır: SAKLAR ve yollar.
    pub fn submit(
        &mut self,
        envelope_type: u8,
        bytes: &[u8],
    ) -> Result<EnvelopeId, MockTransportError> {
        if self.cache.len() >= MOCK_MAX_CACHED {
            return Err(MockTransportError::Backpressure);
        }
        let id = EnvelopeId(self.next_id);
        self.cache.insert(id, (envelope_type, bytes.to_vec()));
        if let Err(e) = self.transmit(id) {
            self.cache.remove(&id);
            return Err(e);
        }
        self.next_id += 1;
        Ok(id)
    }

    /// Saklanan zarfı BYTE-FOR-BYTE yeniden gönderir. Crypto ÇAĞRILMAZ.
    pub fn retry(&mut self, id: EnvelopeId) -> Result<(), MockTransportError> {
        if !self.cache.contains_key(&id) {
            return Err(MockTransportError::UnknownEnvelope);
        }
        self.transmit(id)
    }

    /// Teyit geldi / niyet bitti: zarf artık yeniden gönderilmez.
    pub fn forget(&mut self, id: EnvelopeId) {
        self.cache.remove(&id);
    }

    fn transmit(&mut self, id: EnvelopeId) -> Result<(), MockTransportError> {
        let (ty, bytes) = self
            .cache
            .get(&id)
            .cloned()
            .ok_or(MockTransportError::UnknownEnvelope)?;
        let fault = self.faults.pop_front();
        let copies = match fault {
            Some(Fault::Drop) => {
                self.dropped += 1;
                self.sent += 1;
                return Ok(());
            }
            Some(Fault::Duplicate) => 2,
            _ => 1,
        };
        if self.in_flight.len() + copies > MOCK_MAX_IN_FLIGHT {
            // Arıza tüketilmedi sayılır: gönderim hiç yapılmadı.
            if let Some(f) = fault {
                self.faults.push_front(f);
            }
            return Err(MockTransportError::Backpressure);
        }
        self.sent += 1;
        for _ in 0..copies {
            if fault == Some(Fault::Reorder) && !self.in_flight.is_empty() {
                let at = self.in_flight.len() - 1;
                self.in_flight.insert(at, (ty, bytes.clone()));
            } else {
                self.in_flight.push_back((ty, bytes.clone()));
            }
        }
        Ok(())
    }

    /// Karşı uca ulaşan bir sonraki zarfı, Crypto'ya enjekte edilecek KOMUT olarak verir.
    /// Tür `envelope_type`'tan seçilir; zarfın içi yorumlanmaz (K6).
    pub fn deliver(&mut self) -> Option<Result<SessionCommand, MockTransportError>> {
        let (ty, wire) = self.in_flight.pop_front()?;
        Some(match ty {
            ENVELOPE_INIT => Ok(SessionCommand::AcceptInit { wire }),
            ENVELOPE_INIT_RESPONSE => Ok(SessionCommand::AcceptResponse { wire }),
            _ => Err(MockTransportError::UnknownEnvelopeType),
        })
    }

    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }
}

// ── PeerSlot simülasyonu (S3 §7; H-PN-OWN) ───────────────────────────────────────────────────

/// Kayıt tablosu tavanı (SR-AUDIT-001).
pub const MOCK_MAX_SLOT_RECORDS: usize = 4096;

/// Transport'un eş başına tuttuğu yerel durumun en ince modeli: kendi (dar) gönderim numarası ve
/// Crypto'nun `seal()` dönüşünde verdiği `full_wire_pn`'lerin kaydı.
///
/// Sözleşme iddiası (S3 K2): PN'in sahibi Crypto'dur. Transport'un yerel durumu sıfırlansa, boşta
/// emekliye ayrılsa ya da kendi dar numarası sarsa bile seam sayacı bundan ETKİLENMEZ — çünkü
/// Transport `seal()`'e PN vermez. Bu tip o olayları üretir; sayaca dokunacak bir yolu YOKTUR.
#[derive(Debug, Default)]
pub struct PeerSlotSim {
    /// Transport-yerel dar numara (interim çerçevedeki `seq u16`'nın karşılığı).
    pub tx_seq: u16,
    /// `seal()` dönüşlerinden kaydedilen PN'ler (ARQ tablosunun anahtarları).
    recorded: Vec<u64>,
    pub resets: u32,
}

impl PeerSlotSim {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bir gönderimi kaydeder: yerel numara ilerler (sarar), seam'in verdiği PN tabloya yazılır.
    pub fn on_sealed(&mut self, full_wire_pn: u64) -> Result<(), MockTransportError> {
        if self.recorded.len() >= MOCK_MAX_SLOT_RECORDS {
            return Err(MockTransportError::Backpressure);
        }
        self.recorded.push(full_wire_pn);
        self.tx_seq = self.tx_seq.wrapping_add(1);
        Ok(())
    }

    /// PeerSlot sıfırlandı (yeniden bağlanma): yerel numara 0'a döner, tablo boşalır.
    pub fn reset(&mut self) {
        self.tx_seq = 0;
        self.recorded.clear();
        self.resets += 1;
    }

    /// Boşta emeklilik: slot bırakılır; sonraki gönderim taze slotla başlar.
    pub fn retire_idle(&mut self) {
        self.reset();
    }

    /// Yerel dar numarayı sarma sınırına getirir (bir sonraki gönderimde 0'a sarar).
    pub fn force_tx_seq_to_wrap_edge(&mut self) {
        self.tx_seq = u16::MAX;
    }

    pub fn recorded(&self) -> &[u64] {
        &self.recorded
    }
}
