//! dh_epoch 0→1 CONFIRMATION-SAFE state-machine (§4-B3; send/recv-epoch DIRECTION-LOCAL).
//!
//! 5-ADIM (§4-B3):
//! 1. Bob RESPONSE-gönderir AMA Alice→Bob epoch-0-KABULÜNÜ-SÜRDÜRÜR (grace-BAŞLAMAZ — Alice'in-aldığı-belirsiz).
//! 2. Alice RESPONSE'u-alınca send_epoch[A→B]=1 + HEMEN epoch-1'de-≥1-paket (idle-deadlock-önlenir).
//! 3. Bob İLK-GEÇERLİ-epoch-1-paketini-alınca Alice'in-RESPONSE'u-aldığını-ÖĞRENİR (implicit-confirmation).
//! 4. Bob ANCAK-bundan-sonra epoch-0-recv-GRACE-başlatır + send_epoch[B→A]=1.
//! 5. GRACE — KS-GRACE-1 (2026-09-24; A5.4 2026-10-08 ürün yolunda): `N_g` = [`GRACE_N_G`] (64)
//!    AEAD-geçerli epoch-1 paketi (onaylayan ilk paket #1) VEYA `T_g` = [`GRACE_T_G`] (30 s, monotonic,
//!    onay anından), HANGİSİ ÖNCE; kapanınca epoch-0-recv-DISCARD, yeniden AÇILMAZ. sRTT bağı YOK:
//!    ~~grace = max(4×sRTT, GRACE_MIN_PACKETS)~~ kurucu bütçe argümanı kaldırıldı (A5.4 hücre 5b).
//!
//! RESPONSE-kaybolursa: Alice-epoch-0'da-kalır, Bob-epoch-0-kabulü-SÜRER → "epoch-0-usable"-garantisi KORUNUR.
//! PN sayacı epoch'lar boyunca SÜREKLİDİR: epoch-1 epoch-0'ın son PN'inden +1 ile devam eder; ayrı olan
//! yalnız anahtardır (§4 EPOCH-PN-INIT, 2026-10-06). Deterministik (RNG-YOK); zaman çağırandan gelir
//! (`now`: enjekte edilen monoton saatin değeri — `crate::clock`).
//!
//! **Alım yönü iki rolde de aynı kuralla kapanır [YORUM, A5.4]:** Initiator'ın epoch-0 alımı (Bob→Alice)
//! Bob onaylayıp epoch-1'e geçtikten sonra gelen İLK geçerli epoch-1 paketiyle grace'e girer ve aynı
//! `N_g`/`T_g` kuralıyla kapanır; aksi hâlde Alice'in epoch-0 alım anahtarı oturum boyunca canlı kalırdı.

use core::time::Duration;

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
    /// epoch-0 recv kabul-ediliyor (transition-öncesi).
    Accepting,
    /// grace-aktif: epoch-0 hâlâ kabul. `counted` = sayılan AEAD-geçerli epoch-1 paketi (onaylayan
    /// #1); `since` = grace'in başladığı monoton an.
    Grace { counted: u32, since: Duration },
    /// grace-bitti: epoch-0 recv-state DISCARD (§4-B3 adım-5). Geri dönüş YOK.
    Discarded,
}

/// Machine'in çağırana ürettiği eylem (oturum katmanı protector'a uygular).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochAction {
    /// epoch-1 anahtarları kur + HEMEN epoch-1'de-≥1-paket-gönder (idle-deadlock-önleme).
    InstallEpoch1AndSend,
    /// epoch-0 recv-grace başlat (bounded).
    StartGrace,
    /// epoch-0 recv-state discard (grace-bitişi).
    DiscardEpoch0Recv,
}

/// KS-GRACE-1 `N_g` = 64 — SABİT (yalnız AEAD-geçerli epoch-1 paketleri sayılır).
pub const GRACE_N_G: u32 = 64;
/// Eski ad (A5.4 öncesi "alt sınır"dı; artık `N_g`'nin TAM değeri) — mevcut tüketiciler için.
pub const GRACE_MIN_PACKETS: u32 = GRACE_N_G;
/// KS-GRACE-1 `T_g` = 30 s (monotonic; implicit confirmation anından).
pub const GRACE_T_G: Duration = Duration::from_secs(30);

/// dh_epoch 0→1 machine (direction-local). Deterministik; RNG-YOK.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DhEpochMachine {
    role: Role,
    send_epoch: u8,
    recv0: RecvEpoch0,
    epoch1_installed: bool,
}

impl DhEpochMachine {
    #[must_use]
    pub fn new(role: Role) -> Self {
        Self {
            role,
            send_epoch: 0,
            recv0: RecvEpoch0::Accepting,
            epoch1_installed: false,
        }
    }

    #[must_use]
    pub fn role(&self) -> Role {
        self.role
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

    /// Responder'da epoch-1 alım anahtarları RESPONSE gönderilirken kurulur (Bob, Alice'in
    /// RESPONSE sonrası epoch-1 paketlerini kabul edebilsin; gönderim epoch-0'da KALIR — adım-1).
    pub fn on_response_sent(&mut self) {
        if self.role == Role::Responder {
            self.epoch1_installed = true;
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

    /// AEAD-GEÇERLİ bir epoch-1 paketi alındı (`now` = monoton an). Geçersiz paket bu yola GİRMEZ
    /// (çağıranın sözleşmesi: yalnız açılmış paket).
    ///
    /// * İlk paket — Responder: implicit confirmation (adım-3/4) → send_epoch=1 + grace (#1);
    ///   Initiator: karşı yön geçti → grace (#1).
    /// * Grace içinde sonraki her paket bir sayımdır; `N_g`'inci pakette discard.
    /// * Discard sonrası: hiçbir şey (yeniden açılmaz).
    pub fn on_valid_epoch1_received(&mut self, now: Duration) -> Vec<EpochAction> {
        if !self.epoch1_installed {
            return Vec::new();
        }
        match self.recv0 {
            RecvEpoch0::Accepting => {
                let mut actions = Vec::with_capacity(2);
                if self.role == Role::Responder && self.send_epoch == 0 {
                    self.send_epoch = 1;
                    actions.push(EpochAction::InstallEpoch1AndSend);
                }
                actions.push(EpochAction::StartGrace);
                self.recv0 = RecvEpoch0::Grace {
                    counted: 1,
                    since: now,
                };
                actions
            }
            RecvEpoch0::Grace { counted, since } => {
                let counted = counted.saturating_add(1);
                if counted >= GRACE_N_G {
                    self.recv0 = RecvEpoch0::Discarded;
                    return vec![EpochAction::DiscardEpoch0Recv];
                }
                self.recv0 = RecvEpoch0::Grace { counted, since };
                Vec::new()
            }
            RecvEpoch0::Discarded => Vec::new(),
        }
    }

    /// Zaman girdisi: grace içinde `now − since ≥ T_g` → discard (`T_g` paket sayımından
    /// bağımsız; hangisi önce).
    pub fn on_time(&mut self, now: Duration) -> Option<EpochAction> {
        if let RecvEpoch0::Grace { since, .. } = self.recv0 {
            if now.saturating_sub(since) >= GRACE_T_G {
                self.recv0 = RecvEpoch0::Discarded;
                return Some(EpochAction::DiscardEpoch0Recv);
            }
        }
        None
    }
}
