//! dh_epoch 0→1 CONFIRMATION-SAFE state-machine (§4-B3; send/recv-epoch DIRECTION-LOCAL).
//!
//! 5-ADIM (§4-B3):
//! 1. Bob RESPONSE-gönderir AMA Alice→Bob epoch-0-KABULÜNÜ-SÜRDÜRÜR (grace-BAŞLAMAZ — Alice'in-aldığı-belirsiz).
//! 2. Alice RESPONSE'u-alınca send_epoch[A→B]=1 + HEMEN epoch-1'de-≥1-paket (idle-deadlock-önlenir).
//! 3. Bob İLK-GEÇERLİ-epoch-1-paketini-alınca Alice'in-RESPONSE'u-aldığını-ÖĞRENİR (implicit-confirmation).
//! 4. Bob ANCAK-bundan-sonra epoch-0-recv-GRACE-başlatır + send_epoch[B→A]=1.
//! 5. GRACE = max(4×sRTT, GRACE_MIN_PACKETS=64) [CAL — D1-ölçümle-kilitlenir]; bitince epoch-0-recv-discard.
//!
//! RESPONSE-kaybolursa: Alice-epoch-0'da-kalır, Bob-epoch-0-kabulü-SÜRER → "epoch-0-usable"-garantisi KORUNUR.
//! PN sayacı epoch'lar boyunca SÜREKLİDİR: epoch-1 epoch-0'ın son PN'inden +1 ile devam eder; ayrı olan
//! yalnız anahtardır (§4 EPOCH-PN-INIT, 2026-10-06). Deterministik (RNG-YOK).

/// Endpoint rolü (§0.7-A one-way zero-RTT: initiator SESSION_INIT gönderir).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Alice — SESSION_INIT gönderen; RESPONSE-alınca epoch-1'e-geçer (adım-2).
    Initiator,
    /// Bob — RESPONSE gönderen; ilk-geçerli-epoch-1-alınca epoch-1'e-geçer + grace-başlatır (adım-3/4).
    Responder,
}

/// epoch-0 recv-state (§4-B3 grace).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecvEpoch0 {
    /// epoch-0 recv kabul-ediliyor (transition-öncesi / grace-boyunca).
    Accepting,
    /// grace-aktif: epoch-0 hâlâ-kabul ama bounded-geri-sayım (paket-eşdeğeri).
    Grace { remaining_pkts: u32 },
    /// grace-bitti: epoch-0 recv-state DISCARD (§4-B3 adım-5).
    Discarded,
}

/// Machine'in çağırana ürettiği eylem (engine protector'a uygular).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochAction {
    /// epoch-1 anahtarları kur + HEMEN epoch-1'de-≥1-paket-gönder (idle-deadlock-önleme).
    InstallEpoch1AndSend,
    /// epoch-0 recv-grace başlat (bounded).
    StartGrace,
    /// epoch-0 recv-state discard (grace-bitişi).
    DiscardEpoch0Recv,
}

/// GRACE alt-sınır (§4-B3; paket-eşdeğeri). sRTT-tabanlı üst-değer [CAL] engine'de max()'lanır.
pub const GRACE_MIN_PACKETS: u32 = 64;

/// dh_epoch 0→1 machine (direction-local). Deterministik; RNG-YOK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhEpochMachine {
    role: Role,
    send_epoch: u8,
    recv0: RecvEpoch0,
    epoch1_installed: bool,
    grace_budget_pkts: u32,
}

impl DhEpochMachine {
    /// `grace_budget_pkts` = max(4×sRTT-eşdeğeri, GRACE_MIN_PACKETS) [CAL — engine hesaplar].
    #[must_use]
    pub fn new(role: Role, grace_budget_pkts: u32) -> Self {
        Self {
            role,
            send_epoch: 0,
            recv0: RecvEpoch0::Accepting,
            epoch1_installed: false,
            grace_budget_pkts: grace_budget_pkts.max(GRACE_MIN_PACKETS),
        }
    }

    #[must_use]
    pub fn send_epoch(&self) -> u8 {
        self.send_epoch
    }
    #[must_use]
    pub fn recv0(&self) -> RecvEpoch0 {
        self.recv0
    }
    /// Verilen dh_epoch recv-tarafında kabul-ediliyor mu (§4 accept-adım-1 + grace).
    #[must_use]
    pub fn recv_accepts(&self, dh_epoch: u8) -> bool {
        match dh_epoch & 1 {
            0 => !matches!(self.recv0, RecvEpoch0::Discarded),
            _ => self.epoch1_installed,
        }
    }

    /// **Adım-2 (Initiator):** RESPONSE alındı → send_epoch=1 + epoch-1-kur + HEMEN-gönder.
    /// (Responder'da NO-OP: RESPONSE'u Bob göndermez, almaz.) İdempotent.
    pub fn on_response_received(&mut self) -> Option<EpochAction> {
        if self.role != Role::Initiator || self.send_epoch == 1 {
            return None;
        }
        self.send_epoch = 1;
        self.epoch1_installed = true;
        Some(EpochAction::InstallEpoch1AndSend)
    }

    /// **Adım-3/4 (Responder):** İLK-GEÇERLİ-epoch-1-paket alındı → implicit-confirmation → send_epoch=1 +
    /// epoch-1-kur + epoch-0-recv-GRACE-başlat. (Initiator'da NO-OP.) İdempotent (ilk-kez).
    pub fn on_valid_epoch1_received(&mut self) -> Vec<EpochAction> {
        if self.role != Role::Responder || self.send_epoch == 1 {
            return Vec::new();
        }
        self.send_epoch = 1;
        self.epoch1_installed = true;
        self.recv0 = RecvEpoch0::Grace {
            remaining_pkts: self.grace_budget_pkts,
        };
        vec![EpochAction::InstallEpoch1AndSend, EpochAction::StartGrace]
    }

    /// **Adım-5:** grace-boyunca gelen-her-paket bir tick düşer; sıfırlanınca epoch-0-recv-discard.
    /// (Engine grace-aktifken her-alınan-pakette çağırır; alternatif sRTT-timer engine-tarafında.)
    pub fn on_grace_tick(&mut self) -> Option<EpochAction> {
        if let RecvEpoch0::Grace { remaining_pkts } = self.recv0 {
            let left = remaining_pkts.saturating_sub(1);
            if left == 0 {
                self.recv0 = RecvEpoch0::Discarded;
                return Some(EpochAction::DiscardEpoch0Recv);
            }
            self.recv0 = RecvEpoch0::Grace {
                remaining_pkts: left,
            };
        }
        None
    }
}
