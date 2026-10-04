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
//! # Neden S3 tiplerinin yerel aynası
//!
//! `SessionCommand`/`SessionResult` trait tipleri `client-core/src/seam.rs`'te yaşıyor (R-7(b)) ve
//! `client-core` zaten `kama-noise-wire`'a bağımlı; `kama-noise-wire` testleri onları import ederse
//! bağımlılık DÖNGÜSÜ oluşur. Bu yüzden aşağıdaki [`InjectedCommand`] KM-SEAM-TC-001 §1.2'nin
//! test-yerel aynasıdır. Kalıcı çözüm (trait'lerin ortak bir crate'e taşınması ya da adaptörün
//! `client-core`'da yazılması) A5 öncesi sahip kararıdır.

#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};

/// app-rpc `STORE_MESSAGE.envelope_type` registry'si (AMENDMENT-7).
pub const ENVELOPE_INIT: u8 = 0x01;
pub const ENVELOPE_INIT_RESPONSE: u8 = 0x02;

/// Taşıma kuyruğu tavanı.
pub const MOCK_MAX_IN_FLIGHT: usize = 64;
/// Yeniden gönderim için saklanan zarf tavanı.
pub const MOCK_MAX_CACHED: usize = 64;

/// Transport'un Crypto'ya enjekte ettiği komut (KM-SEAM-TC-001 §1.2 `SessionCommand` aynası).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectedCommand {
    Initiate {
        peer: u64,
        early_unit: Option<Vec<u8>>,
    },
    AcceptInit {
        wire: Vec<u8>,
    },
    AcceptResponse {
        wire: Vec<u8>,
    },
    Renew {
        session: u64,
    },
    Close {
        session: u64,
    },
}

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

    /// Crypto'nun `OutboundEnvelope{envelope_type, bytes}` çıktısını alır: SAKLAR ve yollar.
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
    pub fn deliver(&mut self) -> Option<Result<InjectedCommand, MockTransportError>> {
        let (ty, wire) = self.in_flight.pop_front()?;
        Some(match ty {
            ENVELOPE_INIT => Ok(InjectedCommand::AcceptInit { wire }),
            ENVELOPE_INIT_RESPONSE => Ok(InjectedCommand::AcceptResponse { wire }),
            _ => Err(MockTransportError::UnknownEnvelopeType),
        })
    }

    pub fn in_flight_len(&self) -> usize {
        self.in_flight.len()
    }
}
