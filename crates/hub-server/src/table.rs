//! Tablolar (KM-HUB-TDD-001 §2; S4 H3, H9): uçlar, hatlar, tombstone'lar ve tahsis anahtarı
//! indeksleri. Hepsi bellekte; hub yeniden başlarsa boş başlar.
//!
//! **Zaman tek başına hiçbir şeyi silmez** (S4 KR-9, H9): uç kaydı hiç silinmez (dizinle sınırlı); hat
//! yalnız yeni hat açılırken emeklilik planıyla (S4 §2.6) emekli olur; tek zamanlayıcı süresi dolmuş
//! tombstone'u kaldırır ([`Table::sweep`]).
//!
//! Değişmezler (testlerle her adımda denetlenir, T-S15 / T-M8):
//! * `by_addr[a] = d` ⇔ `endpoints[d].addr = Some(a)`;
//! * `by_key` = aktif hatların anahtarları (hat başına bir girdi), `tomb_by_key` = tombstone'ların;
//! * uç `link_count` = o ucun bulunduğu hat sayısı; `links ≤ max_links`, her uç `≤ max_links_per_endpoint`,
//!   `tombstones ≤ max_tombstones`.

use core::fmt;
use core::time::Duration;
use std::collections::HashMap;
use std::net::SocketAddr;

use blake2::{Blake2s256, Digest};

use crate::config::Limits;
use crate::rate::TokenBucket;
use crate::wire::{handle_candidate_input, AllocationKey, CertRef, DestHash, Nonce, SECRET_LEN};

/// Handle adayı deneme sayısı (S4 §2.6).
pub const HANDLE_ATTEMPTS: usize = 8;

/// Son kabul edilen bind kanıtı (yalnız kayıp ACK'te aynı yanıtı yinelemek için; S4 §2.4(b) adım 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastAccepted {
    pub client_nonce: Nonce,
    pub hub_nonce: Nonce,
    pub addr: SocketAddr,
    /// Yollanmış başarı yanıtının baytları (bayt-özdeş yineleme).
    pub ack: Vec<u8>,
}

/// Uç: bind olmuş bir cihaz.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// Dönüş adresi. Yalnız doğrulanmış bind yazar (H4). `None`: aynı adresi başka bir kimlik kanıtladı.
    pub addr: Option<SocketAddr>,
    pub cert_ref: CertRef,
    pub last_activity: Duration,
    /// Kabul edilmiş bind sayısı; cookie girdisi (H3).
    pub bind_epoch: u32,
    pub last_accepted: Option<LastAccepted>,
    /// Bu ucun bulunduğu hat sayısı (her hat iki uçta sayılır).
    pub link_count: usize,
    pub store_bucket: TokenBucket,
    pub alloc_bucket: TokenBucket,
    pub route_miss_bucket: TokenBucket,
}

/// Hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub a: DestHash,
    pub b: DestHash,
    pub last_activity: Duration,
    /// Hattı açan isteğin anahtarı (KR-19).
    pub key: AllocationKey,
}

impl Link {
    /// `end` bu hattın bir ucuysa karşı uç.
    #[must_use]
    pub fn other(&self, end: &DestHash) -> Option<DestHash> {
        if *end == self.a {
            Some(self.b)
        } else if *end == self.b {
            Some(self.a)
        } else {
            None
        }
    }

    fn touches(&self, end: &DestHash) -> bool {
        self.a == *end || self.b == *end
    }
}

/// Emekli handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    pub retired_at: Duration,
    pub key: AllocationKey,
}

/// [`Table::accept_bind`]'in yaptığı değişiklik (sayaçlar için).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BindChange {
    /// Uç başka bir adresten geliyordu; adres taşındı (H3).
    pub moved: bool,
    /// Adresi başka bir kimlik tutuyordu; o uç adressiz kaldı (H3, `bind_displaced`).
    pub displaced: bool,
}

/// Bir tahsis anahtarının durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// Aktif bir hatta.
    Active(u32),
    /// Tombstone'da (emekli).
    Retired,
    /// Hiçbirinde.
    Absent,
}

/// Emeklilik planı (S4 §2.6 adım 3–6): emekli edilecek hatlar; küme.
pub type Plan = Vec<u32>;

/// Tablolar.
pub struct Table {
    limits: Limits,
    endpoints: HashMap<DestHash, Endpoint>,
    by_addr: HashMap<SocketAddr, DestHash>,
    links: HashMap<u32, Link>,
    by_key: HashMap<AllocationKey, u32>,
    tombstones: HashMap<u32, Tombstone>,
    tomb_by_key: HashMap<AllocationKey, u32>,
    handle_secret: [u8; SECRET_LEN],
    alloc_counter: u64,
}

impl fmt::Debug for Table {
    /// `handle_secret` ve `alloc_counter` basılmaz: ikisini bilen gelecekteki bütün handle'ları tahmin eder.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Table")
            .field("endpoints", &self.endpoints.len())
            .field("links", &self.links.len())
            .field("tombstones", &self.tombstones.len())
            .finish_non_exhaustive()
    }
}

impl Table {
    #[must_use]
    pub fn new(limits: Limits, handle_secret: [u8; SECRET_LEN]) -> Self {
        Self {
            limits,
            endpoints: HashMap::new(),
            by_addr: HashMap::new(),
            links: HashMap::new(),
            by_key: HashMap::new(),
            tombstones: HashMap::new(),
            tomb_by_key: HashMap::new(),
            handle_secret,
            alloc_counter: 0,
        }
    }

    // ── Okuma ──────────────────────────────────────────────────────────────────────────────────

    #[must_use]
    pub fn endpoint(&self, dest: &DestHash) -> Option<&Endpoint> {
        self.endpoints.get(dest)
    }

    pub(crate) fn endpoint_mut(&mut self, dest: &DestHash) -> Option<&mut Endpoint> {
        self.endpoints.get_mut(dest)
    }

    /// `addr`'e bağlı uç.
    #[must_use]
    pub fn key_at(&self, addr: SocketAddr) -> Option<DestHash> {
        self.by_addr.get(&addr).copied()
    }

    /// Ucun dönüş adresi (bağlıysa).
    #[must_use]
    pub fn bound_addr(&self, dest: &DestHash) -> Option<SocketAddr> {
        self.endpoints.get(dest)?.addr
    }

    /// Ucun `bind_epoch`'u (kayıt yoksa 0).
    #[must_use]
    pub fn bind_epoch(&self, dest: &DestHash) -> u32 {
        self.endpoints
            .get(dest)
            .map_or(0, |endpoint| endpoint.bind_epoch)
    }

    #[must_use]
    pub fn link(&self, handle: u32) -> Option<&Link> {
        self.links.get(&handle)
    }

    pub(crate) fn link_mut(&mut self, handle: u32) -> Option<&mut Link> {
        self.links.get_mut(&handle)
    }

    #[must_use]
    pub fn tombstone(&self, handle: u32) -> Option<&Tombstone> {
        self.tombstones.get(&handle)
    }

    #[must_use]
    pub fn endpoint_count(&self) -> usize {
        self.endpoints.len()
    }

    #[must_use]
    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.tombstones.len()
    }

    /// `(by_key, tomb_by_key)` boyları (T-S15: `links` ve `tombstones` ile eşit olmalı).
    #[must_use]
    pub fn index_sizes(&self) -> (usize, usize) {
        (self.by_key.len(), self.tomb_by_key.len())
    }

    /// Aday üreticisinin sayacı (T-M5).
    #[must_use]
    pub fn alloc_counter(&self) -> u64 {
        self.alloc_counter
    }

    /// Tablonun kapasiteyle ilgili bütün hâli: sıralı hatlar, uç sayaçları, tombstone'lar. `HUB_CAPACITY`
    /// yanıtının durumu değiştirmediğini kanıtlamak için (T-M4, T-M8).
    #[must_use]
    pub fn capacity_snapshot(&self) -> CapacitySnapshot {
        let mut links: Vec<_> = self
            .links
            .iter()
            .map(|(h, l)| (*h, l.a, l.b, l.key))
            .collect();
        links.sort_unstable_by_key(|entry| entry.0);
        let mut counts: Vec<_> = self
            .endpoints
            .iter()
            .map(|(d, e)| (*d, e.link_count))
            .collect();
        counts.sort_unstable();
        let mut tombstones: Vec<_> = self.tombstones.iter().map(|(h, t)| (*h, t.key)).collect();
        tombstones.sort_unstable_by_key(|entry| entry.0);
        CapacitySnapshot {
            links,
            counts,
            tombstones,
        }
    }

    /// Bütün değişmezler tutuyor mu (testler her adımda çağırır). İhlalde açıklama döner.
    ///
    /// # Errors
    /// İlk bulunan ihlal.
    pub fn check_invariants(&self) -> Result<(), &'static str> {
        for (addr, dest) in &self.by_addr {
            if self.endpoints.get(dest).and_then(|e| e.addr) != Some(*addr) {
                return Err("by_addr ↛ endpoints");
            }
        }
        for (dest, endpoint) in &self.endpoints {
            if let Some(addr) = endpoint.addr {
                if self.by_addr.get(&addr) != Some(dest) {
                    return Err("endpoints ↛ by_addr");
                }
            }
            let count = self
                .links
                .values()
                .filter(|link| link.touches(dest))
                .count();
            if count != endpoint.link_count {
                return Err("link_count");
            }
            if count > self.limits.max_links_per_endpoint {
                return Err("MAX_LINKS_PER_ENDPOINT");
            }
        }
        if self.by_key.len() != self.links.len()
            || self
                .links
                .iter()
                .any(|(h, l)| self.by_key.get(&l.key) != Some(h))
        {
            return Err("by_key ≠ links");
        }
        if self.tomb_by_key.len() != self.tombstones.len()
            || self
                .tombstones
                .iter()
                .any(|(h, t)| self.tomb_by_key.get(&t.key) != Some(h))
        {
            return Err("tomb_by_key ≠ tombstones");
        }
        if self.links.len() > self.limits.max_links {
            return Err("MAX_LINKS");
        }
        if self.tombstones.len() > self.limits.max_tombstones {
            return Err("MAX_TOMBSTONES");
        }
        if self
            .links
            .keys()
            .any(|h| *h == 0 || self.tombstones.contains_key(h))
        {
            return Err("handle hem aktif hem tombstone (ya da 0)");
        }
        Ok(())
    }

    // ── Bind (H3) ──────────────────────────────────────────────────────────────────────────────

    /// Doğrulanmış bind'i kabul eder (S4 §2.4(b) adım 6): uç yoksa oluşturur, adres farklıysa TAŞIR
    /// (hatlar korunur), `bind_epoch += 1`, `last_accepted` yazılır. Adresi değiştiren TEK yol (H4).
    /// `addr` başka bir kimliğin dönüş adresiyse o ucun adresi düşer (uç ve hatları kalır).
    pub fn accept_bind(
        &mut self,
        dest: DestHash,
        cert_ref: CertRef,
        addr: SocketAddr,
        now: Duration,
        accepted: LastAccepted,
    ) -> BindChange {
        let mut change = BindChange::default();
        if let Some(previous) = self
            .by_addr
            .get(&addr)
            .copied()
            .filter(|held| *held != dest)
        {
            if let Some(other) = self.endpoints.get_mut(&previous) {
                other.addr = None;
                change.displaced = true;
            }
        }
        let limits = self.limits;
        let endpoint = self.endpoints.entry(dest).or_insert_with(|| Endpoint {
            addr: None,
            cert_ref,
            last_activity: now,
            bind_epoch: 0,
            last_accepted: None,
            link_count: 0,
            store_bucket: TokenBucket::full(limits.store_rate, now),
            alloc_bucket: TokenBucket::full(limits.link_alloc_rate, now),
            route_miss_bucket: TokenBucket::full(limits.route_miss_rate, now),
        });
        if let Some(old) = endpoint.addr.filter(|old| *old != addr) {
            self.by_addr.remove(&old);
            change.moved = true;
        }
        endpoint.addr = Some(addr);
        endpoint.cert_ref = cert_ref;
        endpoint.last_activity = now;
        endpoint.bind_epoch = endpoint.bind_epoch.wrapping_add(1);
        endpoint.last_accepted = Some(accepted);
        self.by_addr.insert(addr, dest);
        change
    }

    // ── Hat açma (S4 §2.6) ─────────────────────────────────────────────────────────────────────

    /// Tahsis anahtarı nerede.
    #[must_use]
    pub fn key_state(&self, key: &AllocationKey) -> KeyState {
        if let Some(handle) = self.by_key.get(key) {
            KeyState::Active(*handle)
        } else if self.tomb_by_key.contains_key(key) {
            KeyState::Retired
        } else {
            KeyState::Absent
        }
    }

    /// Emeklilik planı ve denetimler (adım 3–7, aday hariç). Durum DEĞİŞTİRMEZ. `None` = `HUB_CAPACITY`.
    #[must_use]
    pub fn plan_new_link(&self, me: &DestHash, recipient: &DestHash) -> Option<Plan> {
        let per = self.limits.max_links_per_endpoint;
        let count = |dest: &DestHash| self.endpoints.get(dest).map_or(0, |e| e.link_count);
        let mut plan: Plan = Vec::new();

        // 4. Alıcı kotası: YALNIZ gönderen–alıcı ortak hatlarından.
        if count(recipient) >= per {
            let shared =
                self.oldest_idle(&plan, |link| link.touches(me) && link.touches(recipient))?;
            plan.push(shared);
        }
        let after = |dest: &DestHash, plan: &Plan| {
            let retiring = plan
                .iter()
                .filter(|h| self.links.get(h).is_some_and(|link| link.touches(dest)))
                .count();
            count(dest).saturating_sub(retiring)
        };
        // 5. Gönderen kotası: kendi kalan hatlarından.
        if after(me, &plan) >= per {
            if let Some(own) = self.oldest_idle(&plan, |link| link.touches(me)) {
                plan.push(own);
            }
        }
        // 6. Küresel tavan, plan sonrası sayıyla.
        if self.links.len().saturating_sub(plan.len()) >= self.limits.max_links {
            if let Some(any) = self.oldest_idle(&plan, |_| true) {
                plan.push(any);
            }
        }
        // 7. Denetimler (aday ayrı: `pick_handle`).
        let fits = after(me, &plan) < per
            && after(recipient, &plan) < per
            && self.links.len().saturating_sub(plan.len()) < self.limits.max_links
            && self.tombstones.len().saturating_add(plan.len()) <= self.limits.max_tombstones;
        fits.then_some(plan)
    }

    /// Aday handle: `BLAKE2s(handle_secret ‖ alloc_counter)[0..4]`; HER deneme sayacı ilerletir; 0,
    /// aktif ya da tombstone'daki değer atlanır; 8 denemede bulunamazsa `None`. Yan etkisi YALNIZ sayaç.
    pub fn pick_handle(&mut self) -> Option<u32> {
        for _ in 0..HANDLE_ATTEMPTS {
            let candidate = handle_candidate(&self.handle_secret, self.alloc_counter);
            self.alloc_counter = self.alloc_counter.wrapping_add(1);
            if candidate != 0
                && !self.links.contains_key(&candidate)
                && !self.tombstones.contains_key(&candidate)
            {
                return Some(candidate);
            }
        }
        None
    }

    /// Adım 8: planı uygular ve yeni hattı yazar. Yalnız [`Self::plan_new_link`] ve
    /// [`Self::pick_handle`] başarılıysa çağrılır.
    pub fn commit_new_link(
        &mut self,
        plan: &Plan,
        handle: u32,
        me: DestHash,
        recipient: DestHash,
        key: AllocationKey,
        now: Duration,
    ) {
        for retiring in plan {
            self.retire(*retiring, now);
        }
        self.links.insert(
            handle,
            Link {
                a: me,
                b: recipient,
                last_activity: now,
                key,
            },
        );
        self.by_key.insert(key, handle);
        for end in [me, recipient] {
            if let Some(endpoint) = self.endpoints.get_mut(&end) {
                endpoint.link_count = endpoint.link_count.saturating_add(1);
            }
        }
    }

    /// Hattı emekli eder: anahtar tombstone'a taşınır, iki ucun sayacı −1.
    pub fn retire(&mut self, handle: u32, now: Duration) {
        let Some(link) = self.links.remove(&handle) else {
            return;
        };
        self.by_key.remove(&link.key);
        for end in [link.a, link.b] {
            if let Some(endpoint) = self.endpoints.get_mut(&end) {
                endpoint.link_count = endpoint.link_count.saturating_sub(1);
            }
        }
        self.tombstones.insert(
            handle,
            Tombstone {
                retired_at: now,
                key: link.key,
            },
        );
        self.tomb_by_key.insert(link.key, handle);
    }

    /// Süresi dolan (`HANDLE_TOMBSTONE`) tombstone'ları kaldırır — tablolardaki TEK zamanlı silme.
    /// Kaldırılan sayısını döner.
    pub fn sweep(&mut self, now: Duration) -> usize {
        let keep_for = self.limits.handle_tombstone;
        let expired: Vec<u32> = self
            .tombstones
            .iter()
            .filter(|(_, t)| now.saturating_sub(t.retired_at) >= keep_for)
            .map(|(h, _)| *h)
            .collect();
        for handle in &expired {
            if let Some(tombstone) = self.tombstones.remove(handle) {
                self.tomb_by_key.remove(&tombstone.key);
            }
        }
        expired.len()
    }

    /// Plana girmemiş ve `filter`'ı sağlayan hatlardan en uzun süredir kullanılmayanı (eşitlikte küçük
    /// handle — belirlenimli).
    fn oldest_idle(&self, plan: &Plan, filter: impl Fn(&Link) -> bool) -> Option<u32> {
        self.links
            .iter()
            .filter(|(h, link)| !plan.contains(h) && filter(link))
            .min_by_key(|(h, link)| (link.last_activity, **h))
            .map(|(h, _)| *h)
    }
}

/// [`Table::capacity_snapshot`] sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapacitySnapshot {
    pub links: Vec<(u32, DestHash, DestHash, AllocationKey)>,
    pub counts: Vec<(DestHash, usize)>,
    pub tombstones: Vec<(u32, AllocationKey)>,
}

/// S4 §2.6 aday değeri (`handle-alloc` vektörünün Rust tarafı).
#[must_use]
pub fn handle_candidate(secret: &[u8; SECRET_LEN], counter: u64) -> u32 {
    let digest = Blake2s256::digest(handle_candidate_input(secret, counter));
    let mut head = [0u8; 4];
    for (slot, byte) in head.iter_mut().zip(digest.iter()) {
        *slot = *byte;
    }
    u32::from_be_bytes(head)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn dest(n: u8) -> DestHash {
        [n; 32]
    }

    fn addr(n: u8) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, n)), 40_000)
    }

    fn key(sender: u8, recipient: u8, request_id: u32) -> AllocationKey {
        AllocationKey {
            sender: dest(sender),
            request_id,
            recipient: dest(recipient),
            envelope_type: 1,
            envelope_digest: [0; 8],
        }
    }

    fn accepted(n: u8) -> LastAccepted {
        LastAccepted {
            client_nonce: [n; 16],
            hub_nonce: [n; 16],
            addr: addr(n),
            ack: vec![n],
        }
    }

    fn table(limits: Limits) -> Table {
        let mut table = Table::new(limits, [7; 32]);
        for n in 1..=6 {
            table.accept_bind(dest(n), [n; 32], addr(n), Duration::ZERO, accepted(n));
        }
        table
    }

    /// Plan + aday + uygula; `None` = HUB_CAPACITY.
    fn open(table: &mut Table, me: u8, recipient: u8, request_id: u32, now: u64) -> Option<u32> {
        let plan = table.plan_new_link(&dest(me), &dest(recipient))?;
        let handle = table.pick_handle()?;
        table.commit_new_link(
            &plan,
            handle,
            dest(me),
            dest(recipient),
            key(me, recipient, request_id),
            Duration::from_secs(now),
        );
        table.check_invariants().expect("değişmez");
        Some(handle)
    }

    fn small() -> Limits {
        Limits {
            max_links: 6,
            max_links_per_endpoint: 3,
            max_tombstones: 4,
            ..Limits::S4
        }
    }

    #[test]
    fn debug_never_shows_the_handle_secret() {
        let shown = format!("{:?}", Table::new(Limits::S4, [0xab; 32]));
        assert!(
            !shown.contains("171") && !shown.contains("ab, ab"),
            "{shown}"
        );
    }

    #[test]
    fn bind_moves_the_address_and_bumps_the_epoch() {
        let mut t = table(Limits::S4);
        assert_eq!(t.bind_epoch(&dest(1)), 1);
        t.accept_bind(
            dest(1),
            [1; 32],
            addr(9),
            Duration::from_secs(1),
            accepted(9),
        );
        assert_eq!(t.bind_epoch(&dest(1)), 2);
        assert_eq!(t.bound_addr(&dest(1)), Some(addr(9)));
        assert_eq!(t.key_at(addr(1)), None);
        // Başka kimliğin adresini kanıtlayan bind onun adresini düşürür; uç kalır.
        t.accept_bind(
            dest(2),
            [2; 32],
            addr(9),
            Duration::from_secs(2),
            accepted(9),
        );
        assert_eq!(t.bound_addr(&dest(1)), None);
        assert!(t.endpoint(&dest(1)).is_some());
        assert_eq!(t.key_at(addr(9)), Some(dest(2)));
        t.check_invariants().expect("değişmez");
    }

    #[test]
    fn sender_quota_retires_the_senders_own_oldest_link() {
        let mut t = table(small());
        let first = open(&mut t, 1, 2, 1, 1).expect("1");
        open(&mut t, 1, 3, 2, 2).expect("2");
        open(&mut t, 1, 4, 3, 3).expect("3");
        let fourth = open(&mut t, 1, 5, 4, 4).expect("gönderen kotası emeklilikle açılır");
        assert!(t.link(first).is_none() && t.tombstone(first).is_some());
        assert!(t.link(fourth).is_some());
        assert_eq!(
            t.endpoint(&dest(2)).map(|e| e.link_count),
            Some(0),
            "iki ucun sayacı −1"
        );
    }

    #[test]
    fn recipient_quota_only_touches_the_shared_links() {
        let mut t = table(small());
        let shared = open(&mut t, 1, 2, 1, 1).expect("1-2");
        open(&mut t, 3, 2, 2, 2).expect("3-2");
        open(&mut t, 4, 2, 3, 3).expect("4-2");
        // 2 dolu; 5'in 2 ile ortak hattı yok → HUB_CAPACITY, hiçbir şey değişmez.
        let before = t.capacity_snapshot();
        assert_eq!(t.plan_new_link(&dest(5), &dest(2)), None);
        assert_eq!(t.capacity_snapshot(), before);
        // 1'in 2 ile ortak hattı var → yalnız o emekli.
        assert_eq!(t.plan_new_link(&dest(1), &dest(2)), Some(vec![shared]));
    }

    #[test]
    fn global_ceiling_and_plan_is_a_set() {
        let mut t = table(Limits {
            max_links: 2,
            max_links_per_endpoint: 2,
            ..small()
        });
        let oldest = open(&mut t, 1, 2, 1, 1).expect("1-2");
        open(&mut t, 1, 3, 2, 2).expect("1-3");
        // Gönderen kotası ve küresel tavan AYNI hattı işaret eder → bir kez.
        assert_eq!(t.plan_new_link(&dest(1), &dest(4)), Some(vec![oldest]));
        // Küresel tavan, başka çift.
        assert_eq!(t.plan_new_link(&dest(5), &dest(6)), Some(vec![oldest]));
    }

    #[test]
    fn full_graveyard_refuses_without_changing_anything() {
        let mut t = table(Limits {
            max_tombstones: 1,
            ..small()
        });
        for n in 0..4 {
            open(&mut t, 1, 2 + (n % 3) as u8, n, u64::from(n)).expect("aç");
        }
        assert_eq!(t.tombstone_count(), 1);
        let before = t.capacity_snapshot();
        assert_eq!(
            t.plan_new_link(&dest(1), &dest(5)),
            None,
            "emeklilik tombstone'a sığmaz"
        );
        assert_eq!(t.capacity_snapshot(), before);
    }

    /// T-M5 — 8 aday çakışırsa HUB_CAPACITY; mevcut hat üstüne yazılmaz; sayaç 8 ilerler; sonraki
    /// istek farklı adaylarla başarılı.
    #[test]
    fn t_m5_candidate_exhaustion_never_overwrites() {
        let mut t = table(Limits::S4);
        let secret = [7u8; 32];
        // Sayaç 0..8'in adaylarını önceden hat (4) ve tombstone (4) olarak kur.
        for counter in 0..8u64 {
            let handle = handle_candidate(&secret, counter);
            let k = key(1, 2, u32::try_from(counter).expect("küçük"));
            if counter < 4 {
                t.links.insert(
                    handle,
                    Link {
                        a: dest(1),
                        b: dest(2),
                        last_activity: Duration::ZERO,
                        key: k,
                    },
                );
                t.by_key.insert(k, handle);
            } else {
                t.tombstones.insert(
                    handle,
                    Tombstone {
                        retired_at: Duration::ZERO,
                        key: k,
                    },
                );
                t.tomb_by_key.insert(k, handle);
            }
        }
        let before = t.capacity_snapshot();
        assert_eq!(t.pick_handle(), None);
        assert_eq!(t.alloc_counter(), 8);
        assert_eq!(t.capacity_snapshot(), before, "üstüne yazma yok");
        let next = t.pick_handle().expect("yeni adaylar");
        assert_eq!(next, handle_candidate(&secret, 8));
        assert_eq!(t.alloc_counter(), 9);
    }

    #[test]
    fn retired_keys_move_to_the_graveyard_and_expire_after_seven_days() {
        let mut t = table(small());
        let h = open(&mut t, 1, 2, 1, 0).expect("aç");
        t.retire(h, Duration::from_secs(10));
        assert_eq!(t.key_state(&key(1, 2, 1)), KeyState::Retired);
        assert_eq!(t.sweep(Duration::from_secs(10 + 7 * 86_400 - 1)), 0);
        assert_eq!(t.sweep(Duration::from_secs(10 + 7 * 86_400)), 1);
        assert_eq!(t.key_state(&key(1, 2, 1)), KeyState::Absent);
        t.check_invariants().expect("değişmez");
    }
}
