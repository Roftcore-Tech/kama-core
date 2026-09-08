//! [FAZ-1 / DÖNÜŞ-YOLU GÖZLEMİ] Geri-bildirim yolunun ölçülen durumu.
//!
//! **Neden:** transport'un birincil hedef rejimleri arasında *asimetrik yol* ve *bozulmuş dönüş
//! yolu* var, ama gönderici bugüne dek **ileri-yön kaybı ile dönüş-yolu bozulmasını ayırt
//! edemiyordu**: kayıp defteri yalnız ileri yönü görür, `ack_confidence` yalnız sessizliği ölçer,
//! `seen_count` telde taşınıp tüketilmiyordu (transport-v1 denetimi, tema A).
//!
//! **Kapsam (KASITLI OLARAK DAR):** bu modül YALNIZ GÖZLEMLER. Hiçbir kontrol yolu (pacing, ARQ,
//! FEC, congestion, confirm ufku) buradan beslenmez. Spec'in `return_bw`-türevli ACK decimation
//! gövdesi (§3.3-E FULL) spec tarafından ADIM-6'ya ertelenmiştir ve v1 kapsamı DIŞINDADIR; burada
//! üretilen `measured_return_bw_bps` o borcun ölçüm yarısıdır.
//!
//! **Mimari invaryant (INV-RETURN-1):** düşük dönüş bant genişliği ileri-yön kaybı DEĞİLDİR;
//! eksik ACK "DATA kayboldu" DEMEK DEĞİLDİR. Bu modül dört fiziksel büyüklüğü ayrı tutar:
//! kayıp · bant · gecikme · süreklilik.
//!
//! Determinizm: tamsayı EWMA, RNG yok, sabit bellek (ADR-17 / SR-AUDIT-001).
//!
//! **Bilinen sınır (dürüstlük notu):** `feedback_continuity_permille` KÜMÜLATİFTİR ve EWMA aralık
//! yeni ritme uyum sağladıkça anlık katkısı söner. Yani "yavaş ama düzenli" bir geri bildirim bir
//! süre sonra `Healthy` görünür; bozulmayı asıl gösteren doğrudan ölçümler
//! `measured_return_bw_bps` (debinin düşmesi) ve `ack_interval_ns` (ritmin uzaması)'dır.
//! Üçü BİRLİKTE okunmalıdır; testler (`r2_...`) bu üçlüyü pinler.

/// EWMA katsayısı (per-mille). Yeni örneğin ağırlığı = `EWMA_ALPHA/1000`.
const EWMA_ALPHA: u64 = 250;

/// `seen_count`/`sent` oranı için asgari kanıt — bunun altında yön-ayrımı BİLDİRİLMEZ.
/// (Az örnekte in-flight kuyruğu ileri-kaybı yapay olarak şişirir.)
const MIN_DIRECTION_EVIDENCE_DATAGRAMS: u64 = 8;

/// Alıcının belgelenmiş ACK sıklığı: her `ACK_FREQ_N` datagram'da bir SACK (receiver.rs:76).
/// Dönüş-yolu ACK kaybının MUTLAK referansı budur — göndericinin gözlediği "ACK başına datagram"
/// bu değerin çok üstündeyse aradaki fark dönüşte kaybolmuştur.
const RECEIVER_ACK_FREQ_N: u64 = 2;
/// `return_ack_loss` için asgari örnek (az örnekte oran gürültülü).
const MIN_ACK_LOSS_SAMPLES: u64 = 4;

/// `Degraded` eşiği: gözlenen ACK aralığı, EWMA aralığın bu katından büyükse süreklilik düşük.
const DEGRADED_GAP_MULT: u64 = 3;
/// `Dead` eşiği (aynı ölçekte, daha sert).
const DEAD_GAP_MULT: u64 = 12;

/// Dönüş yolunun sağlık sınıfı — yalnız gözlem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReturnPathHealth {
    /// Henüz yeterli ACK örneği yok (kuruluş fazı dâhil).
    #[default]
    Unknown,
    /// Geri bildirim beklenen ritimde akıyor.
    Healthy,
    /// Geri bildirim seyrekleşti ama tamamen kesilmedi.
    Degraded,
    /// Geri bildirim fiilen kesildi (ileri yön hâlâ akıyor olabilir!).
    Dead,
}

/// Dönüş yolunun ölçülen anlık durumu (salt gözlem; `Copy`, sabit boyut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReturnPathState {
    /// Ölçülen dönüş bant genişliği (B/s): ACK/kontrol telinin gözlenen debisi (EWMA).
    /// **`EAI_RETURN_BW_PRIOR` (31 B/s) İLE KARIŞTIRILMAZ:** o bir en-kötü-durum ÖNSELİDİR,
    /// bu ise ölçümdür. Örnek yokken 0.
    pub measured_return_bw_bps: u64,
    /// Gözlenen ACK varış aralığı (ns, EWMA).
    pub ack_interval_ns: u64,
    /// Son ACK'ten bu yana geçen süre (ns).
    pub silence_ns: u64,
    /// Süreklilik: beklenen ACK penceresi başına gerçekleşen ACK oranı (per-mille, 1000 = kesintisiz).
    pub feedback_continuity_permille: u32,
    /// **İLERİ-YÖN kaybı** (per-mille): `(sent_{n-1} − seen_n) / sent_{n-1}` — bir-ACK gecikmeli
    /// KÜMÜLATİF oran. Seyrek geri bildirimde OLDUĞUNDAN AZ gösterir (muhafazakâr); dönüş-yolu
    /// bozulmasını ASLA ileri yola yazmaz. Kanıt eşiğinin altında 0 ve `direction_evidence=false`.
    pub forward_loss_permille: u32,
    /// **DÖNÜŞ-YOLU bozulması** (per-mille): kaçırılan geri-bildirim penceresi oranı.
    /// İleri-yön kaybından BAĞIMSIZ türetilir (INV-RETURN-1).
    pub return_impairment_permille: u32,
    /// Yön-ayrımı için yeterli kanıt var mı (`seen_count` taşıyan ≥2 V2 ACK ve ≥8 datagram).
    pub direction_evidence: bool,
    /// Tahmin güveni (per-mille): örnek sayısıyla doyan basit bir ölçü.
    pub confidence_permille: u32,
    /// Sağlık sınıfı.
    pub health: ReturnPathHealth,
    /// **DÖNÜŞ-YOLU ACK kaybı** (per-mille) — MUTLAK ölçü: alıcı her `ACK_FREQ_N`(=2) datagram'da
    /// bir SACK üretir; gönderici ACK başına bundan çok daha fazla datagram yolladıysa aradaki
    /// fark dönüş yolunda kaybolmuştur: `1000 × (1 − ACK_FREQ_N / datagram_per_ack)`.
    /// `ack_samples < 4` iken 0. İleri-yön ölçüsünden TAMAMEN bağımsızdır.
    pub return_ack_loss_permille: u32,
    /// ACK başına gözlenen datagram (×100; 200 = alıcının nominal ritmi).
    pub datagrams_per_ack_x100: u64,
    /// Gözlenen ACK sayısı (kümülatif).
    pub ack_samples: u64,
    /// `seen_count` taşıyan (V2) ACK sayısı.
    pub v2_samples: u64,
}

/// Bir ACK'in gözlem girdisi. Motor davranışını DEĞİŞTİRMEZ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckObservation {
    /// ACK datagram'ının TELDEKİ boyutu (mühürlü; kabuk dâhil).
    pub wire_bytes: u64,
    /// `AckFrame.seen_count` — alıcının GÖRDÜĞÜ kümülatif datagram sayısı (yalnız V2).
    pub seen_count: u32,
    /// Çerçeve V2 mi (yani `seen_count` anlamlı mı).
    pub v2: bool,
    /// Göndericinin bu ACK'i aldığı andaki kümülatif TX datagram sayısı.
    pub sent_datagrams: u64,
}

/// Sabit bellekli dönüş-yolu gözlemcisi.
#[derive(Debug, Clone, Default)]
pub struct ReturnPathObserver {
    last_ack_ns: u64,
    ack_interval_ewma_ns: u64,
    return_bw_ewma_bps: u64,
    ack_samples: u64,
    v2_samples: u64,
    /// Bir önceki ACK'te alıcının gördüğü kümülatif datagram sayısı.
    prev_seen: u64,
    /// Bir önceki ACK ANINDA göndericinin yollamış olduğu kümülatif datagram sayısı
    /// (bir-ACK gecikme: aradaki in-flight kuyruğu ileri-kayıp sayılmasın).
    prev_sent_at_ack: u64,
    forward_loss_permille: u32,
    direction_evidence: bool,
    /// En son gözlenen kümülatif TX datagram sayısı (ACK-başına-datagram için).
    last_sent_seen: u64,
    /// Kaçırılan geri-bildirim penceresi (kümülatif) ve beklenen pencere sayısı.
    missed_windows: u64,
    expected_windows: u64,
}

fn ewma(prev: u64, sample: u64) -> u64 {
    if prev == 0 {
        return sample;
    }
    (prev.saturating_mul(1000 - EWMA_ALPHA) + sample.saturating_mul(EWMA_ALPHA)) / 1000
}

impl ReturnPathObserver {
    /// Bir ACK gözlendi. `now_ns` monotondur.
    pub fn on_ack(&mut self, obs: AckObservation, now_ns: u64) {
        let gap = now_ns.saturating_sub(self.last_ack_ns);
        if self.last_ack_ns > 0 && gap > 0 {
            // Süreklilik: bu boşluk kaç "beklenen pencere" kaçırdı?
            if self.ack_interval_ewma_ns > 0 {
                let windows = gap / self.ack_interval_ewma_ns.max(1);
                self.expected_windows = self.expected_windows.saturating_add(windows.max(1));
                self.missed_windows = self
                    .missed_windows
                    .saturating_add(windows.saturating_sub(1));
            }
            self.ack_interval_ewma_ns = ewma(self.ack_interval_ewma_ns, gap);
            // Ölçülen dönüş debisi: bu ACK'in teldeki boyu / geçen süre.
            let bps = obs
                .wire_bytes
                .saturating_mul(1_000_000_000)
                .checked_div(gap)
                .unwrap_or(0);
            self.return_bw_ewma_bps = ewma(self.return_bw_ewma_bps, bps);
        }
        self.last_ack_ns = now_ns;
        self.ack_samples = self.ack_samples.saturating_add(1);

        // ── YÖN AYRIMI (§2.2: seen≪sent ⇒ ileri kayıp; seen≈sent ama ACK yok ⇒ dönüş kaybı) ──
        //
        // [DÜZELTME 2026-09-08, R0–R5 ölçümüyle ÇÜRÜTÜLDÜ] İlk uygulama iki DELTA'yı kıyaslıyordu
        // (Δseen [t_{n-1},t_n] vs Δsent [t_{n-2},t_{n-1}]) — pencereler HİZASIZDI ve seyrek ACK'li
        // hücrelerde (R2/R3/R5) TEMİZ ileri yolda %75–80 yapay "ileri kayıp" üretiyordu
        // (INV-RETURN-1 ihlali; ölçüm: reports/phase1-return-path/).
        //
        // Doğru form BİR-ACK GECİKMELİ KÜMÜLATİF orandır: alıcı ACK_n'i kurarken göndericinin en
        // fazla ACK_{n-1} anındaki kadarını görmüş olabilir (aradaki ~1 RTT'lik uçuş henüz
        // varmamıştır) ⇒ ileri_kayıp = (sent_{n-1} − seen_n) / sent_{n-1}.
        //
        // YAPISAL OLARAK MUHAFAZAKÂR: geri bildirim seyrekleştikçe `sent_{n-1}` daha geride kalır
        // ve ölçü ileri kaybı OLDUĞUNDAN AZ gösterir; dönüş bozulmasını ASLA ileri yola yazmaz.
        // Faz-1'in koruması gereken invaryant tam olarak budur.
        if obs.v2 {
            self.v2_samples = self.v2_samples.saturating_add(1);
            let seen = u64::from(obs.seen_count);
            let base = self.prev_sent_at_ack; // sent_{n-1}
            if base >= MIN_DIRECTION_EVIDENCE_DATAGRAMS {
                let lost = base.saturating_sub(seen);
                self.forward_loss_permille = u32::try_from(lost.saturating_mul(1000) / base)
                    .unwrap_or(1000)
                    .min(1000);
                self.direction_evidence = true;
            }
            self.prev_seen = seen;
            self.prev_sent_at_ack = obs.sent_datagrams;
        }
        self.last_sent_seen = obs.sent_datagrams;
    }

    /// Anlık durum. `now_ns` ile sessizlik ve sağlık türetilir.
    #[must_use]
    pub fn state(&self, now_ns: u64) -> ReturnPathState {
        let silence = if self.last_ack_ns == 0 {
            0
        } else {
            now_ns.saturating_sub(self.last_ack_ns)
        };
        let cont = if self.expected_windows == 0 {
            1000
        } else {
            let ok = self.expected_windows.saturating_sub(self.missed_windows);
            u32::try_from(ok.saturating_mul(1000) / self.expected_windows.max(1)).unwrap_or(1000)
        };
        // Dönüş-yolu ACK kaybı: alıcının nominal ritmine göre MUTLAK oran.
        let dpa_x100 = if self.ack_samples == 0 {
            0
        } else {
            self.last_sent_seen.saturating_mul(100) / self.ack_samples
        };
        let ack_loss = if self.ack_samples < MIN_ACK_LOSS_SAMPLES || dpa_x100 == 0 {
            0
        } else {
            let nominal = RECEIVER_ACK_FREQ_N.saturating_mul(100);
            if dpa_x100 <= nominal {
                0
            } else {
                u32::try_from(1000 - (nominal.saturating_mul(1000) / dpa_x100)).unwrap_or(0)
            }
        };
        let health = self.health(silence, ack_loss);
        // Güven: 8 örnekte doyar (kaba, deterministik).
        let confidence = u32::try_from((self.ack_samples.min(8) * 1000) / 8).unwrap_or(0);
        ReturnPathState {
            measured_return_bw_bps: self.return_bw_ewma_bps,
            ack_interval_ns: self.ack_interval_ewma_ns,
            silence_ns: silence,
            feedback_continuity_permille: cont,
            forward_loss_permille: if self.direction_evidence {
                self.forward_loss_permille
            } else {
                0
            },
            return_impairment_permille: 1000u32.saturating_sub(cont),
            direction_evidence: self.direction_evidence,
            confidence_permille: confidence,
            return_ack_loss_permille: ack_loss,
            datagrams_per_ack_x100: dpa_x100,
            health,
            ack_samples: self.ack_samples,
            v2_samples: self.v2_samples,
        }
    }

    /// Sağlık: İKİ bağımsız kanıt — (a) anlık sessizlik (ritme göre), (b) ACK kaybının MUTLAK
    /// oranı (alıcının nominal ritmine göre). (b) EWMA'nın yeni ritme uyum sağlamasından
    /// ETKİLENMEZ; R5 gibi "yavaş ama düzenli" bozulmayı yakalayan budur.
    fn health(&self, silence_ns: u64, ack_loss_permille: u32) -> ReturnPathHealth {
        if self.ack_samples < 2 || self.ack_interval_ewma_ns == 0 {
            return ReturnPathHealth::Unknown;
        }
        let base = self.ack_interval_ewma_ns;
        if silence_ns >= base.saturating_mul(DEAD_GAP_MULT) || ack_loss_permille >= 800 {
            ReturnPathHealth::Dead
        } else if silence_ns >= base.saturating_mul(DEGRADED_GAP_MULT) || ack_loss_permille >= 300 {
            ReturnPathHealth::Degraded
        } else {
            ReturnPathHealth::Healthy
        }
    }
}
