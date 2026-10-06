//! A5.1 — Session Model V3 anahtar türetimi D1–D7 (ccr §35.2.1; identity §10.5 / §15.6).
//!
//! **PRİMİTİF (pki §13, cipher_suite_id 0x0001):** HKDF-BLAKE2s — RFC 5869 yapısı, HMAC =
//! HMAC-BLAKE2s-256 (RFC 2104; `hkdf::SimpleHkdfExtract<Blake2s256>`, HMAC'ı `hmac::SimpleHmac`
//! ile içeride kurar). BLAKE2s'in native keyed modu (`blake2::Blake2sMac`) KULLANILMAZ — o HMAC
//! DEĞİLDİR ve sessizce farklı anahtar üretir (INV-KDF-3; V-KDF-6 negatif vektörü yakalar).
//! `hkdf::Hkdf<Blake2s256>` derlenmez (BLAKE2'nin blok API'si lazy); `Simple*` varyantı kullanılır.
//!
//! **TEK KANONİK SARMALAYICI (INV-KDF-1):** her türetme `PRK = Extract(salt = ASCII(label),
//! IKM = secret)` ardından `OKM = Expand(PRK, info = context, L)` yapar. Bu modülün dışına açılan
//! tek yüzey D1–D7 fonksiyonlarıdır; Expand-only, salt=zeros ya da label'ı info'ya taşıyan başka
//! bir çağrım biçimi yapısal olarak yazılamaz. Label byte'ları protocol_version V1 için
//! değiştirilemez (INV-KDF-2) — `LABEL_*` sabitleri.
//!
//! **SIR BİRLEŞİMİ:** çok parçalı secret (D1 `ss_e ‖ ss_s`, D4 `init_root ‖ ss_e2`) heap'e ya da
//! ara tampona kopyalanmaz; parçalar sırayla HMAC'a beslenir (RFC 5869 Extract akışkan HMAC'tır,
//! `input_ikm(a); input_ikm(b)` ≡ `IKM = a ‖ b`, bayt-özdeş).
//!
//! **ÖN-KOŞUL (bu modülün DEĞİL, DH'yi yapan katmanın işi):** D1/D4'e giren her DH çıktısı
//! all-zero / low-order değildir (`x25519_dalek::SharedSecret::was_contributory()`; ccr §5.1).
//! Bu modül DH yapmaz, gelen 32 B'yi olduğu gibi türetime sokar.
//!
//! **GİZLİLİK ETİKETİ:** `ss_e = DH(sender_eph, recipient_static)` tam forward secrecy DEĞİLDİR
//! (0-RTT sınırı); epoch-0 anahtarları için doğru ad "per-INIT ephemeral confidentiality"dir.
//! Tam FS epoch-1'de, SESSION_INIT_RESPONSE responder ephemeral'ı (ee) katıldıktan sonra (D4).
//!
//! **KAPSAM DIŞI (oturum katmanı, L-A5c):** INV-INIT-KEY-1 / INV-RESP-KEY-1 (her kök TAM BİR
//! INIT/RESPONSE mühürler; retry = önbellekli byte-özdeş datagram), rol eşlemesi (initiator
//! `k_send = a2b`, responder tersi) ve ephemeral sk zeroize sırası (identity §15.6 P0-13).
//!
//! **ANAHTAR YAŞAMI:** çıktılar `Zeroizing` içinde döner; Extract'ın PRK çıktısı türetimden hemen
//! sonra sıfırlanır. KALINTI: `hkdf`/`hmac` 0.12 `zeroize` desteklemez — `Hkdf` nesnesinin iç
//! HMAC durumu Drop'ta silinmez (yeni bağımlılık/sürüm gerektirir; kapsam dışı, kayıtlı risk).

use core::fmt;

use blake2::digest::Output;
use blake2::Blake2s256;
use hkdf::{SimpleHkdf, SimpleHkdfExtract};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::protector::KEY_LEN;

/// ccr §35.2.1 HashLen (BLAKE2s-256 çıktısı) — kökler (init_root, epoch_1_root) bu boydadır.
pub const HASH_LEN: usize = 32;
/// ccr §35.2 `session_context_v1` boyu: 1 + 1 + 2 + 32 + 32 + 32.
pub const SESSION_CONTEXT_V1_LEN: usize = 100;
/// D3 `session_id_internal` boyu (L = 16; ccr §35.2 `Trunc128` yazımıyla bayt-özdeş).
pub const SESSION_ID_INTERNAL_LEN: usize = 16;
/// X25519 DH çıktısı (RFC 7748) — D1 `ss_e`/`ss_s`, D4 `ss_e2`'nin iki yarısı.
pub const DH_LEN: usize = 32;

/// D1 label (INV-KDF-2: protocol_version V1 için IMMUTABLE).
pub const LABEL_INIT: &[u8] = b"SR-SESSION-INIT-V3";
/// D2 ve D5 label (AYNI label; secret farklı ⇒ PRK farklı ⇒ anahtar farklı).
pub const LABEL_TRAFFIC: &[u8] = b"SR-SESSION-TRAFFIC-V3";
/// D3 label.
pub const LABEL_SESSION_ID: &[u8] = b"SR-SESSION-ID-V3";
/// D4 label.
pub const LABEL_EPOCH: &[u8] = b"SR-SESSION-EPOCH-V3";
/// D6 label (KARAR-1 = A: SESSION_INIT mühür anahtarı, trafik anahtarından ayrık).
pub const LABEL_INIT_DATA: &[u8] = b"SR-SESSION-INIT-DATA-V3";
/// D7 label (KARAR-1 = A: SESSION_INIT_RESPONSE mühür anahtarı).
pub const LABEL_RESP_DATA: &[u8] = b"SR-SESSION-RESP-DATA-V3";

/// Yön kodu A→B (initiator→responder); D2/D5/D6 context'i (ccr §35.9 direction alanıyla AYNI).
pub const DIR_A2B: u8 = 0x01;
/// Yön kodu B→A (responder→initiator); D2/D5/D7 context'i.
pub const DIR_B2A: u8 = 0x02;
/// D4 context = `u16_be(1)` (epoch numarası 1, big-endian).
pub const EPOCH_1_CONTEXT: [u8; 2] = 1u16.to_be_bytes();

/// RFC 5869 §2.3: Expand en fazla 255 blok üretir.
const RFC5869_MAX_BLOCKS: usize = 255;
/// RFC 5869 §2.3: `L ≤ 255 · HashLen`.
const MAX_OKM_LEN: usize = RFC5869_MAX_BLOCKS * HASH_LEN;
/// Sır taşıyan tiplerin `Debug` çıktısı (içerik sızdırmaz).
const REDACTED: &str = "[redacted]";

/// KDF hatası. D1–D7'nin çıktı boyları sabit ve derleme zamanında `L ≤ 255·HashLen` olarak
/// sınanır; varyant panik yerine tipli hata için vardır (ürün yolunda `expect`/`unwrap` yok).
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum KdfError {
    /// RFC 5869 §2.3: istenen çıktı boyu `255 · HashLen`'i aşıyor.
    #[error("kdf output length exceeds 255*HashLen")]
    OutputLength,
}

/// D1 çıktısı `init_root` (identity §16 `init_root : secret32`). Klonlanmaz; Drop'ta sıfırlanır.
pub struct InitRoot(Zeroizing<[u8; HASH_LEN]>);

impl InitRoot {
    /// Saklanan kökten geri yükleme (identity §16 SessionState) ve vektör girdisi
    /// (`init_root_hex`). Byte'lar D1 çıktısı olmalıdır; bu modül bunu doğrulayamaz.
    #[must_use]
    pub fn from_bytes(bytes: Zeroizing<[u8; HASH_LEN]>) -> Self {
        Self(bytes)
    }

    /// Ham kök — yalnız crate içi (türetim ve oturum katmanı).
    pub(crate) fn expose(&self) -> &[u8; HASH_LEN] {
        &self.0
    }
}

impl fmt::Debug for InitRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "InitRoot({REDACTED})")
    }
}

/// D4 çıktısı `epoch_1_root` (eski ad `session_root`). Klonlanmaz; Drop'ta sıfırlanır.
pub struct Epoch1Root(Zeroizing<[u8; HASH_LEN]>);

impl Epoch1Root {
    /// Saklanan kökten geri yükleme ve vektör girdisi. Byte'lar D4 çıktısı olmalıdır.
    #[must_use]
    pub fn from_bytes(bytes: Zeroizing<[u8; HASH_LEN]>) -> Self {
        Self(bytes)
    }

    /// Ham kök — yalnız crate içi (türetim ve oturum katmanı).
    pub(crate) fn expose(&self) -> &[u8; HASH_LEN] {
        &self.0
    }
}

impl fmt::Debug for Epoch1Root {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Epoch1Root({REDACTED})")
    }
}

/// Yön çifti trafik anahtarları (D2 epoch-0, D5 epoch-1). Rol eşlemesi çağıranındır:
/// initiator `send = a2b`, `recv = b2a`; responder tersi (ccr §35.2.1 D2).
pub struct TrafficKeys {
    /// A→B anahtarı (context = [`DIR_A2B`]).
    pub a2b: Zeroizing<[u8; KEY_LEN]>,
    /// B→A anahtarı (context = [`DIR_B2A`]).
    pub b2a: Zeroizing<[u8; KEY_LEN]>,
}

impl fmt::Debug for TrafficKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TrafficKeys {{ a2b: {REDACTED}, b2a: {REDACTED} }}")
    }
}

/// D1: `init_root = KDF("SR-SESSION-INIT-V3", ss_e ‖ ss_s, session_context_v1, 32)`.
///
/// `ss_e = DH(sender_eph, recipient_static)` (per-INIT ephemeral confidentiality),
/// `ss_s = DH(sender_static, recipient_static)` (sender authentication). İkisi de çağıran
/// katmanda contributory denetiminden geçmiş olmalıdır (modül dokümanı, ÖN-KOŞUL).
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d1_init_root(
    ss_e: &[u8; DH_LEN],
    ss_s: &[u8; DH_LEN],
    session_context_v1: &[u8; SESSION_CONTEXT_V1_LEN],
) -> Result<InitRoot, KdfError> {
    kdf::<HASH_LEN>(
        LABEL_INIT,
        &[ss_e.as_slice(), ss_s.as_slice()],
        session_context_v1,
    )
    .map(InitRoot)
}

/// D2: epoch-0 trafik anahtarları `KDF("SR-SESSION-TRAFFIC-V3", init_root, dir, 32)`.
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d2_traffic_e0(init_root: &InitRoot) -> Result<TrafficKeys, KdfError> {
    traffic_keys(init_root.expose())
}

/// D3: `session_id_internal = KDF("SR-SESSION-ID-V3", init_root, session_context_v1, 16)`.
///
/// Güvenlik sırrı DEĞİLDİR (identity CAD-56): AAD bağlamasıdır, wire'da taşınmaz; bu yüzden
/// düz dizi döner.
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d3_session_id_internal(
    init_root: &InitRoot,
    session_context_v1: &[u8; SESSION_CONTEXT_V1_LEN],
) -> Result<[u8; SESSION_ID_INTERNAL_LEN], KdfError> {
    let sid = kdf::<SESSION_ID_INTERNAL_LEN>(
        LABEL_SESSION_ID,
        &[init_root.expose().as_slice()],
        session_context_v1,
    )?;
    Ok(*sid)
}

/// D4: `epoch_1_root = KDF("SR-SESSION-EPOCH-V3", init_root ‖ ss_e2, u16_be(1), 32)`,
/// `ss_e2 = dh_ee ‖ dh_se`.
///
/// Bayt sırası iki uçta aynıdır (identity §15.6-resp-3):
/// responder `dh_ee = DH(bob_eph_sk, sender_eph_pub)`, `dh_se = DH(bob_eph_sk, sender_static_pub)`;
/// initiator `dh_ee = DH(sender_eph_sk, responder_eph_pub)`,
/// `dh_se = DH(sender_static_sk, responder_eph_pub)`. Tam FS bu kökten itibaren (ee katkısı).
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d4_epoch_1_root(
    init_root: &InitRoot,
    dh_ee: &[u8; DH_LEN],
    dh_se: &[u8; DH_LEN],
) -> Result<Epoch1Root, KdfError> {
    kdf::<HASH_LEN>(
        LABEL_EPOCH,
        &[
            init_root.expose().as_slice(),
            dh_ee.as_slice(),
            dh_se.as_slice(),
        ],
        &EPOCH_1_CONTEXT,
    )
    .map(Epoch1Root)
}

/// D5: epoch-1 trafik anahtarları `KDF("SR-SESSION-TRAFFIC-V3", epoch_1_root, dir, 32)`.
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d5_traffic_e1(epoch_1_root: &Epoch1Root) -> Result<TrafficKeys, KdfError> {
    traffic_keys(epoch_1_root.expose())
}

/// D6: SESSION_INIT mühür anahtarı `k_init = KDF("SR-SESSION-INIT-DATA-V3", init_root, 0x01, 32)`.
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d6_k_init(init_root: &InitRoot) -> Result<Zeroizing<[u8; KEY_LEN]>, KdfError> {
    kdf::<KEY_LEN>(
        LABEL_INIT_DATA,
        &[init_root.expose().as_slice()],
        &[DIR_A2B],
    )
}

/// D7: SESSION_INIT_RESPONSE mühür anahtarı
/// `k_resp = KDF("SR-SESSION-RESP-DATA-V3", epoch_1_root, 0x02, 32)`.
///
/// # Errors
///
/// [`KdfError::OutputLength`] — sabit L ile erişilemez.
pub fn d7_k_resp(epoch_1_root: &Epoch1Root) -> Result<Zeroizing<[u8; KEY_LEN]>, KdfError> {
    kdf::<KEY_LEN>(
        LABEL_RESP_DATA,
        &[epoch_1_root.expose().as_slice()],
        &[DIR_B2A],
    )
}

/// D2/D5 ortak gövdesi: aynı label, kök farklı; context = yön kodu.
fn traffic_keys(root: &[u8; HASH_LEN]) -> Result<TrafficKeys, KdfError> {
    Ok(TrafficKeys {
        a2b: kdf::<KEY_LEN>(LABEL_TRAFFIC, &[root.as_slice()], &[DIR_A2B])?,
        b2a: kdf::<KEY_LEN>(LABEL_TRAFFIC, &[root.as_slice()], &[DIR_B2A])?,
    })
}

/// TEK KANONİK SARMALAYICI (ccr §35.2.1, INV-KDF-1):
/// `PRK = Extract(salt = label, IKM = parts[0] ‖ parts[1] ‖ …)`; `OKM = Expand(PRK, context, L)`.
///
/// `label` yalnız `LABEL_*` sabitlerinden gelir (`'static`); PRK sıfırlanır, OKM `Zeroizing`.
fn kdf<const L: usize>(
    label: &'static [u8],
    secret_parts: &[&[u8]],
    context: &[u8],
) -> Result<Zeroizing<[u8; L]>, KdfError> {
    const { assert!(L <= MAX_OKM_LEN, "RFC 5869: L <= 255 * HashLen") };
    let (mut prk, expander) = extract(label, secret_parts);
    prk.as_mut_slice().zeroize();
    let mut okm = Zeroizing::new([0u8; L]);
    expander
        .expand(context, okm.as_mut_slice())
        .map_err(|_| KdfError::OutputLength)?;
    Ok(okm)
}

/// RFC 5869 §2.2 Extract = HMAC-BLAKE2s-256(key = salt, msg = IKM) — sarmalayıcının ilk yarısı.
/// PRK'yi sıfırlamak çağıranın işidir. Ayrı fonksiyon olmasının tek nedeni V-KDF-6'nın ürün HMAC
/// yolunu doğrudan sınayabilmesidir; ürün kodunda yalnız [`kdf`] çağırır.
fn extract(salt: &[u8], ikm_parts: &[&[u8]]) -> (Output<Blake2s256>, SimpleHkdf<Blake2s256>) {
    let mut ctx = SimpleHkdfExtract::<Blake2s256>::new(Some(salt));
    for &part in ikm_parts {
        ctx.input_ikm(part);
    }
    ctx.finalize()
}

#[cfg(test)]
mod vectors {
    //! `vectors/ccr-kdf-v1` (Python referansı `ref/crypto/kdf.py`'den üretilir) ↔ bu modül.
    //! V-KDF-1..6 vakaları bayt-özdeş; V-KDF-7 (üç kaynak eşitliği) bu testlerin yeşil olmasıyla
    //! sağlanır. Beklenen değerlerin TAMAMI vektör dosyasından gelir; burada gömülü byte yoktur.

    use super::*;
    use kama_vectors::{Case, Expected, Input, RejectResult, VectorFile};

    const SUITE_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vectors/ccr-kdf-v1/ccr-kdf-v1.json"
    );
    const KIND_DERIVE: &str = "derive";
    const KIND_HMAC: &str = "hmac";
    const KIND_HMAC_CANDIDATE: &str = "hmac-candidate";
    const V_KDF_6_REASON: &str = "v-kdf-6-blake2s-keyed-is-not-hmac";

    const ID_INIT_ROOT: &str = "ccr-kdf-v1.v-kdf-1.init-root";
    const ID_K_A2B_E0: &str = "ccr-kdf-v1.v-kdf-2.k-a2b-e0";
    const ID_K_B2A_E0: &str = "ccr-kdf-v1.v-kdf-2.k-b2a-e0";
    const ID_SESSION_ID: &str = "ccr-kdf-v1.v-kdf-3.session-id-internal";
    const ID_TRUNC128: &str = "ccr-kdf-v1.v-kdf-3.trunc128-of-l32";
    const ID_EPOCH_1_ROOT: &str = "ccr-kdf-v1.v-kdf-4.epoch-1-root";
    const ID_K_A2B_E1: &str = "ccr-kdf-v1.v-kdf-4.k-a2b-e1";
    const ID_K_B2A_E1: &str = "ccr-kdf-v1.v-kdf-4.k-b2a-e1";
    const ID_K_INIT: &str = "ccr-kdf-v1.v-kdf-5.k-init";
    const ID_K_RESP: &str = "ccr-kdf-v1.v-kdf-5.k-resp";

    /// INV-KDF-2 pin kümesi: vektördeki her label bunlardan biri olmalı.
    const ALL_LABELS: [&[u8]; 6] = [
        LABEL_INIT,
        LABEL_TRAFFIC,
        LABEL_SESSION_ID,
        LABEL_EPOCH,
        LABEL_INIT_DATA,
        LABEL_RESP_DATA,
    ];

    fn suite() -> VectorFile {
        let file =
            kama_vectors::load_file(SUITE_PATH).expect("ccr-kdf-v1 vektör dosyası yüklenmeli");
        // Vakum kapısı: tanımadığımız bir `kind` varsa o vakalar sessizce atlanmaz, test düşer.
        file.require_kinds(&[KIND_DERIVE, KIND_HMAC, KIND_HMAC_CANDIDATE])
            .expect("ccr-kdf-v1 dosyasında bu tüketicinin tanımadığı bir kind var");
        file
    }

    fn case<'a>(file: &'a VectorFile, id: &str) -> &'a Case {
        file.cases
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("{id}: vaka dosyada yok"))
    }

    fn has_field(case: &Case, name: &str) -> bool {
        let Input::Structured(map) = &case.input else {
            panic!("{}: yapısal girdi bekleniyordu", case.id);
        };
        map.contains_key(name)
    }

    /// Yapısal girdiden hex alanı.
    fn field(case: &Case, name: &str) -> Vec<u8> {
        let Input::Structured(map) = &case.input else {
            panic!("{}: yapısal girdi bekleniyordu", case.id);
        };
        let text = map
            .get(name)
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("{}: '{name}' alanı yok ya da dizge değil", case.id));
        kama_vectors::hex(text).unwrap_or_else(|e| panic!("{}: '{name}' hex değil: {e:?}", case.id))
    }

    /// Sabit boylu hex alanı.
    fn fixed<const N: usize>(case: &Case, name: &str) -> [u8; N] {
        let bytes = field(case, name);
        <[u8; N]>::try_from(bytes.as_slice())
            .unwrap_or_else(|_| panic!("{}: '{name}' {N} bayt değil ({})", case.id, bytes.len()))
    }

    /// `ss_e2_hex` (64 B) → (`dh_ee`, `dh_se`).
    fn split_ss_e2(case: &Case) -> ([u8; DH_LEN], [u8; DH_LEN]) {
        let ss_e2: [u8; 2 * DH_LEN] = fixed(case, "ss_e2_hex");
        let (ee, se) = ss_e2.split_at(DH_LEN);
        let dh_ee: [u8; DH_LEN] = ee.try_into().expect("ilk yarı DH_LEN bayt");
        let dh_se: [u8; DH_LEN] = se.try_into().expect("ikinci yarı DH_LEN bayt");
        (dh_ee, dh_se)
    }

    fn expected_wire(case: &Case) -> &[u8] {
        match &case.expected {
            Expected::Wire(bytes) => bytes.as_slice(),
            Expected::Rejected { .. } => panic!("{}: pozitif beklenen değer bekleniyordu", case.id),
        }
    }

    /// `out_len_hex` (tek bayt) ya da yoksa L = 32 (dosya `description`'ı).
    fn out_len(case: &Case) -> usize {
        if !has_field(case, "out_len_hex") {
            return HASH_LEN;
        }
        let bytes = field(case, "out_len_hex");
        let [len] = bytes.as_slice() else {
            panic!("{}: out_len_hex tek bayt değil", case.id);
        };
        usize::from(*len)
    }

    /// Vakanın label'ı `LABEL_*` sabitlerinden biri (INV-KDF-2) — sabitin kendisini döndürür.
    fn known_label(case: &Case) -> &'static [u8] {
        let label = field(case, "label_hex");
        ALL_LABELS
            .into_iter()
            .find(|known| *known == label.as_slice())
            .unwrap_or_else(|| panic!("{}: label LABEL_* sabitlerinden biri değil", case.id))
    }

    fn pin(case: &Case, what: &str, got: &[u8], want: &[u8]) {
        assert_eq!(got, want, "{}: {what} sabitle uyuşmuyor", case.id);
    }

    /// Bir derive vakasının (secret parçaları, context) çifti. Girdi şekli düzensizliği:
    /// D1 vakası `ss_e/ss_s/session_context_v1`, D4 vakası `init_root/ss_e2` taşır.
    fn derive_inputs(case: &Case) -> (Vec<Vec<u8>>, Vec<u8>) {
        if has_field(case, "ss_e_hex") {
            let parts = vec![field(case, "ss_e_hex"), field(case, "ss_s_hex")];
            (parts, field(case, "session_context_v1_hex"))
        } else if has_field(case, "init_root_hex") {
            let parts = vec![field(case, "init_root_hex"), field(case, "ss_e2_hex")];
            (parts, field(case, "context_hex"))
        } else {
            (vec![field(case, "secret_hex")], field(case, "context_hex"))
        }
    }

    /// Sarmalayıcıyı çalışma zamanı L'siyle çağırır (L yalnız 16 ya da 32).
    fn wrapper(label: &'static [u8], parts: &[&[u8]], context: &[u8], len: usize) -> Vec<u8> {
        match len {
            SESSION_ID_INTERNAL_LEN => kdf::<SESSION_ID_INTERNAL_LEN>(label, parts, context)
                .expect("sabit L")
                .to_vec(),
            HASH_LEN => kdf::<HASH_LEN>(label, parts, context)
                .expect("sabit L")
                .to_vec(),
            other => panic!("vektörde beklenmeyen L = {other}"),
        }
    }

    fn init_root_from(case: &Case, name: &str) -> InitRoot {
        InitRoot::from_bytes(Zeroizing::new(fixed(case, name)))
    }

    fn epoch_1_root_from(case: &Case, name: &str) -> Epoch1Root {
        Epoch1Root::from_bytes(Zeroizing::new(fixed(case, name)))
    }

    /// D2/D5 vakasında context'e göre yön anahtarını seçer.
    fn pick_direction(case: &Case, keys: &TrafficKeys) -> Vec<u8> {
        let context = field(case, "context_hex");
        if context == [DIR_A2B] {
            keys.a2b.to_vec()
        } else if context == [DIR_B2A] {
            keys.b2a.to_vec()
        } else {
            panic!("{}: context bir yön kodu değil", case.id)
        }
    }

    fn run_derive_wrapper(file: &VectorFile) -> usize {
        let mut consumed = 0;
        for case in file.cases.iter().filter(|c| c.kind == KIND_DERIVE) {
            let (parts, context) = derive_inputs(case);
            let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
            let okm = wrapper(known_label(case), &refs, &context, out_len(case));
            assert_eq!(
                okm.as_slice(),
                expected_wire(case),
                "{}: kdf() çıktısı vektörden farklı",
                case.id
            );
            consumed += 1;
        }
        assert!(consumed > 0, "derive vakası yok (vakum)");
        consumed
    }

    fn run_named_d_functions(file: &VectorFile) -> usize {
        let mut consumed = 0;
        for case in file.cases.iter().filter(|c| c.kind == KIND_DERIVE) {
            let label = field(case, "label_hex");
            let expected = expected_wire(case);
            let d_flag = case.flags.first().map(String::as_str);
            let (got, want): (Vec<u8>, &[u8]) = match d_flag {
                Some("d1") => {
                    pin(case, "label", &label, LABEL_INIT);
                    let root = d1_init_root(
                        &fixed(case, "ss_e_hex"),
                        &fixed(case, "ss_s_hex"),
                        &fixed(case, "session_context_v1_hex"),
                    )
                    .expect("D1");
                    (root.expose().to_vec(), expected)
                }
                Some("d2") => {
                    pin(case, "label", &label, LABEL_TRAFFIC);
                    let keys = d2_traffic_e0(&init_root_from(case, "secret_hex")).expect("D2");
                    (pick_direction(case, &keys), expected)
                }
                Some("d3") => {
                    pin(case, "label", &label, LABEL_SESSION_ID);
                    let sid = d3_session_id_internal(
                        &init_root_from(case, "secret_hex"),
                        &fixed(case, "context_hex"),
                    )
                    .expect("D3");
                    // L=16 vakası tam eşit; L=32 vakasının ilk 16 B'si (prefix-identity).
                    (sid.to_vec(), &expected[..SESSION_ID_INTERNAL_LEN])
                }
                Some("d4") => {
                    pin(case, "label", &label, LABEL_EPOCH);
                    pin(
                        case,
                        "context",
                        &field(case, "context_hex"),
                        &EPOCH_1_CONTEXT,
                    );
                    let (dh_ee, dh_se) = split_ss_e2(case);
                    let root =
                        d4_epoch_1_root(&init_root_from(case, "init_root_hex"), &dh_ee, &dh_se)
                            .expect("D4");
                    (root.expose().to_vec(), expected)
                }
                Some("d5") => {
                    pin(case, "label", &label, LABEL_TRAFFIC);
                    let keys = d5_traffic_e1(&epoch_1_root_from(case, "secret_hex")).expect("D5");
                    (pick_direction(case, &keys), expected)
                }
                Some("d6") => {
                    pin(case, "label", &label, LABEL_INIT_DATA);
                    pin(case, "context", &field(case, "context_hex"), &[DIR_A2B]);
                    let key = d6_k_init(&init_root_from(case, "secret_hex")).expect("D6");
                    (key.to_vec(), expected)
                }
                Some("d7") => {
                    pin(case, "label", &label, LABEL_RESP_DATA);
                    pin(case, "context", &field(case, "context_hex"), &[DIR_B2A]);
                    let key = d7_k_resp(&epoch_1_root_from(case, "secret_hex")).expect("D7");
                    (key.to_vec(), expected)
                }
                other => panic!("{}: tanınmayan D bayrağı {other:?}", case.id),
            };
            assert_eq!(
                got.as_slice(),
                want,
                "{}: adlı D fonksiyonu vektörden farklı",
                case.id
            );
            consumed += 1;
        }
        assert!(consumed > 0, "derive vakası yok (vakum)");
        consumed
    }

    /// V-KDF-6 / INV-KDF-3: ürünün Extract yolu (HMAC-BLAKE2s-256) pozitif vektörü üretir ve
    /// BLAKE2s-keyed adayını ÜRETMEZ.
    fn run_inv_kdf_3(file: &VectorFile) -> usize {
        let mut consumed = 0;
        for case in file.cases.iter().filter(|c| c.kind == KIND_HMAC) {
            let message = field(case, "message_hex");
            let (prk, _) = extract(&field(case, "key_hex"), &[message.as_slice()]);
            assert_eq!(
                prk.as_slice(),
                expected_wire(case),
                "{}: ürün HMAC yolu HMAC-BLAKE2s-256 değil",
                case.id
            );
            consumed += 1;
        }
        for case in file.cases.iter().filter(|c| c.kind == KIND_HMAC_CANDIDATE) {
            let message = field(case, "message_hex");
            let (prk, _) = extract(&field(case, "key_hex"), &[message.as_slice()]);
            let candidate = field(case, "candidate_hex");
            assert_ne!(
                prk.as_slice(),
                candidate.as_slice(),
                "{}: ürün HMAC yolu BLAKE2s-keyed çıktısı veriyor (INV-KDF-3)",
                case.id
            );
            match &case.expected {
                Expected::Rejected { result, reason } => {
                    assert_eq!(*result, RejectResult::Reject, "{}: karar", case.id);
                    assert_eq!(reason.as_str(), V_KDF_6_REASON, "{}: gerekçe", case.id);
                }
                Expected::Wire(_) => panic!("{}: negatif vaka bekleniyordu", case.id),
            }
            consumed += 1;
        }
        consumed
    }

    /// V-KDF-1..5: her `derive` vakası tek kanonik sarmalayıcıyla bayt-özdeş.
    #[test]
    fn derive_cases_match_wrapper() {
        run_derive_wrapper(&suite());
    }

    /// Her `derive` vakası adlı D1–D7 fonksiyonuyla bayt-özdeş; label (INV-KDF-2) ve context
    /// sabitleri vektörle pinli.
    #[test]
    fn derive_cases_match_named_d_functions() {
        run_named_d_functions(&suite());
    }

    /// Uçtan uca zincir (D1 → D2/D3/D6, D1 → D4 → D5/D7) ve vakalar arası eşitsizlikler:
    /// INV-NONCE-DIR-1, INV-NONCE-EPOCH-1, D6/D7 domain-separation, D3 prefix-identity.
    #[test]
    fn cross_case_separation() {
        let file = suite();
        let c1 = case(&file, ID_INIT_ROOT);
        let ctx: [u8; SESSION_CONTEXT_V1_LEN] = fixed(c1, "session_context_v1_hex");
        let init_root =
            d1_init_root(&fixed(c1, "ss_e_hex"), &fixed(c1, "ss_s_hex"), &ctx).expect("D1");
        assert_eq!(init_root.expose().as_slice(), expected_wire(c1), "D1");
        for id in [
            ID_K_A2B_E0,
            ID_K_B2A_E0,
            ID_SESSION_ID,
            ID_TRUNC128,
            ID_K_INIT,
        ] {
            let secret = field(case(&file, id), "secret_hex");
            assert_eq!(
                secret.as_slice(),
                init_root.expose().as_slice(),
                "{id}: zincir D1"
            );
        }
        for id in [ID_SESSION_ID, ID_TRUNC128] {
            let context = field(case(&file, id), "context_hex");
            assert_eq!(context.as_slice(), ctx.as_slice(), "{id}: D3 context");
        }

        let c6 = case(&file, ID_EPOCH_1_ROOT);
        let chained = field(c6, "init_root_hex");
        assert_eq!(
            chained.as_slice(),
            init_root.expose().as_slice(),
            "D4 zincir D1"
        );
        let (dh_ee, dh_se) = split_ss_e2(c6);
        let epoch_1_root = d4_epoch_1_root(&init_root, &dh_ee, &dh_se).expect("D4");
        assert_eq!(epoch_1_root.expose().as_slice(), expected_wire(c6), "D4");
        for id in [ID_K_A2B_E1, ID_K_B2A_E1, ID_K_RESP] {
            let secret = field(case(&file, id), "secret_hex");
            assert_eq!(
                secret.as_slice(),
                epoch_1_root.expose().as_slice(),
                "{id}: zincir D4"
            );
        }

        let e0 = d2_traffic_e0(&init_root).expect("D2");
        let e1 = d5_traffic_e1(&epoch_1_root).expect("D5");
        let k_init = d6_k_init(&init_root).expect("D6");
        let k_resp = d7_k_resp(&epoch_1_root).expect("D7");
        let sid = d3_session_id_internal(&init_root, &ctx).expect("D3");
        let l32 = kdf::<HASH_LEN>(LABEL_SESSION_ID, &[init_root.expose().as_slice()], &ctx)
            .expect("D3 L=32");

        let exact: [(&str, &[u8]); 7] = [
            (ID_K_A2B_E0, e0.a2b.as_slice()),
            (ID_K_B2A_E0, e0.b2a.as_slice()),
            (ID_K_A2B_E1, e1.a2b.as_slice()),
            (ID_K_B2A_E1, e1.b2a.as_slice()),
            (ID_K_INIT, k_init.as_slice()),
            (ID_K_RESP, k_resp.as_slice()),
            (ID_SESSION_ID, sid.as_slice()),
        ];
        for (id, got) in exact {
            assert_eq!(got, expected_wire(case(&file, id)), "{id}: zincir çıktısı");
        }

        // V-KDF-2 / INV-NONCE-DIR-1.
        assert_ne!(e0.a2b.as_slice(), e0.b2a.as_slice(), "k_A2B_e0 == k_B2A_e0");
        assert_ne!(e1.a2b.as_slice(), e1.b2a.as_slice(), "k_A2B_e1 == k_B2A_e1");
        // V-KDF-4 / INV-NONCE-EPOCH-1.
        assert_ne!(e1.a2b.as_slice(), e0.a2b.as_slice(), "k_A2B_e1 == k_A2B_e0");
        assert_ne!(e1.b2a.as_slice(), e0.b2a.as_slice(), "k_B2A_e1 == k_B2A_e0");
        // V-KDF-5 / D6-D7 domain-separation.
        assert_ne!(k_init.as_slice(), e0.a2b.as_slice(), "k_init == k_A2B_e0");
        assert_ne!(k_resp.as_slice(), e1.b2a.as_slice(), "k_resp == k_B2A_e1");
        // V-KDF-3 prefix-identity: L=16 ≡ Trunc128(L=32).
        assert_eq!(
            &l32[..SESSION_ID_INTERNAL_LEN],
            sid.as_slice(),
            "Trunc128(L=32) != L=16"
        );
        assert_eq!(
            l32.as_slice(),
            expected_wire(case(&file, ID_TRUNC128)),
            "D3 L=32"
        );
    }

    /// V-KDF-6 (INV-KDF-3): HMAC-BLAKE2s-256 ≠ BLAKE2s-keyed, ürün Extract yolu üzerinden.
    #[test]
    fn inv_kdf_3_hmac_is_not_keyed_blake2s() {
        assert!(run_inv_kdf_3(&suite()) > 0, "V-KDF-6 vakası yok (vakum)");
    }

    /// Dosyadaki HER vaka koşuldu (filtreleyip azını koşmak yeşil görünmesin).
    #[test]
    fn every_case_is_consumed() {
        let file = suite();
        let derive = run_derive_wrapper(&file);
        assert_eq!(
            derive + run_inv_kdf_3(&file),
            file.case_count(),
            "koşulan vaka sayısı dosyanın beyan ettiğinden farklı"
        );
        assert_eq!(
            run_named_d_functions(&file),
            derive,
            "her derive vakası adlı D fonksiyonuna da dağıtılmalı"
        );
    }

    /// Sır taşıyan tiplerin Debug çıktısı içerik sızdırmaz.
    #[test]
    fn debug_is_redacted() {
        let file = suite();
        let c1 = case(&file, ID_INIT_ROOT);
        let init_root = d1_init_root(
            &fixed(c1, "ss_e_hex"),
            &fixed(c1, "ss_s_hex"),
            &fixed(c1, "session_context_v1_hex"),
        )
        .expect("D1");
        let keys = d2_traffic_e0(&init_root).expect("D2");
        assert_eq!(format!("{init_root:?}"), format!("InitRoot({REDACTED})"));
        assert_eq!(
            format!("{keys:?}"),
            format!("TrafficKeys {{ a2b: {REDACTED}, b2a: {REDACTED} }}")
        );
    }
}
