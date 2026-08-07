//! TC-03 + TC-04 — `protector.rs` sınır ve guard yolları (SR-STR-CORE-001 §5.1, BUL-05).
//!
//! **Kapsanan bulgu:** BUL-05. STR §3.4 `protector.rs`'te üç sıfır-sayımlı construct-site raporlar
//! (`SealInput`, `PnCapReached`, `WireSizeError::EmptySymbol`) ve ayrıca `EpochKeysMissing`'in iki
//! site'ta `[113, 113, 0]` örüntüsü verdiğini — satır çalışıyor, hata dalı çalışmıyor.
//!
//! **TC-03 gerekçesi:** `PnCapReached`, ChaCha20-Poly1305 nonce tekrarına karşı **son yapısal
//! bariyerdir**; bu şifrede nonce tekrarı katastrofiktir (keystream tekrarı + Poly1305 anahtar
//! sızıntısı). Sayarak 2⁶² pakete ulaşmak imkânsız olduğu için test yazılmadığı anlaşılıyor —
//! ancak state doğrudan kurularak sınanabilir. Bariyer test edilmezse gelecekteki bir off-by-one
//! (`>` yerine `>=`) sessizce geçer.
//!
//! **Oracle:** cap−1 → `Ok`; cap → `Err(PnCapReached)`; cap sonrası hiçbir seal başarılı olmaz ve
//! hiçbir byte üretilmez.
//!
//! ---
//! **[ÖLÇÜLDÜ — bu dosya] YAPISAL BULGU (TC-04/a):** `open()` yolundaki `EpochKeysMissing`
//! site'ına **ulaşılamaz.** `open()` sırası şudur: epoch-kabul kontrolü (`recv_epoch_accepts`)
//! anahtar kontrolünden ÖNCE gelir. Epoch-1 anahtarı yokken `recv_epoch_accepts[1]` de
//! `false`'tur (ikisi de yalnız `install_epoch1_keys()` ile birlikte `true` olur), dolayısıyla
//! epoch-1 datagramı `EpochNotAccepted(1)` alır — `EpochKeysMissing` DEĞİL. Aynısı
//! `discard_epoch0_recv()` sonrası epoch-0 için de geçerli. Yani bu site iki bayrağın
//! senkronizasyonu bozulmadıkça sıfır sayımlı KALACAKTIR ve kapsam hedefine yazılmamalıdır.
//! Aşağıdaki testler hem gerçek davranışı (`EpochNotAccepted`) hem de erişilebilir olan
//! seal-tarafı `EpochKeysMissing` site'ını sabitler. [BEKLİYOR — Abkarada: iki bayrağın tek
//! kaynaktan türetilmesi mi, yoksa mevcut çift-bayrak + ulaşılamaz-guard mı korunacak?]

use kama_noise_wire::error::WireError;
use kama_noise_wire::protector::{KamaNoiseWireSizer, WireSizeError};
use kama_noise_wire::{KamaNoiseWireProtector, SealArgs, SEND_PN_CAP};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket};
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0102_0304;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn protector() -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(H_EXT, &key(0x11), &key(0x22))
}

fn shard(symbol_len: usize) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Source,
            profile_id: 0x03,
            block_id: 7,
            esi: 3,
            k_actual: 8,
        },
        symbol: vec![0xAAu8; symbol_len],
    }
}

fn args(wire_pn: u64, dh_epoch: u8) -> SealArgs {
    SealArgs {
        wire_pn,
        dh_epoch,
        send_generation: 0,
    }
}

// ── TC-03 (a): SEND_PN_CAP sınırı ───────────────────────────────────────────

#[test]
fn seal_at_cap_minus_one_succeeds() {
    let mut p = protector();
    let s = p.seal(&shard(64), args(SEND_PN_CAP - 1, 0)).expect("cap-1");
    assert_eq!(s.meta.full_wire_pn, SEND_PN_CAP - 1);
    assert_eq!(s.bytes.len(), 101);
}

#[test]
fn seal_at_cap_is_rejected() {
    let mut p = protector();
    assert_eq!(
        p.seal(&shard(64), args(SEND_PN_CAP, 0)).unwrap_err(),
        WireError::PnCapReached
    );
}

#[test]
fn cap_boundary_is_inclusive_ge_not_gt() {
    // Off-by-one kapısı. Kod `>=` kullanıyor; biri `>` yaparsa TAM CAP değeri geçerdi ve
    // renewal bir paket geç tetiklenirdi. Üç komşu değeri birlikte pinliyoruz.
    let mut p = protector();
    assert!(p.seal(&shard(64), args(SEND_PN_CAP - 2, 0)).is_ok());
    assert!(p.seal(&shard(64), args(SEND_PN_CAP - 1, 0)).is_ok());
    assert_eq!(
        p.seal(&shard(64), args(SEND_PN_CAP, 0)).unwrap_err(),
        WireError::PnCapReached
    );
    assert_eq!(
        p.seal(&shard(64), args(SEND_PN_CAP + 1, 0)).unwrap_err(),
        WireError::PnCapReached
    );
    assert_eq!(
        p.seal(&shard(64), args(u64::MAX, 0)).unwrap_err(),
        WireError::PnCapReached
    );
}

#[test]
fn no_bytes_are_produced_after_cap() {
    // Cap'e ulaşıldıktan sonra hiçbir seal başarılı olmamalı ve hiçbir byte üretilmemeli.
    // "SESSION_RENEW_REQUIRED tetiklenmeli" kuralının wire-yüzü budur.
    let mut p = protector();
    assert!(p.seal(&shard(64), args(SEND_PN_CAP - 1, 0)).is_ok());
    for pn in [SEND_PN_CAP, SEND_PN_CAP + 1, u64::MAX] {
        match p.seal(&shard(64), args(pn, 0)) {
            Err(WireError::PnCapReached) => {}
            other => panic!("pn={pn}: {other:?}"),
        }
    }
}

#[test]
fn cap_check_precedes_monotonicity_check() {
    // Sıra sözleşmesi: cap kontrolü monotonluk kontrolünden ÖNCE gelir. Cap üstü bir PN,
    // aynı zamanda monotonluk ihlali de olsa `PnCapReached` almalı — teşhis mesajının
    // doğru bariyeri işaret etmesi renewal mantığı için gerekli.
    let mut p = protector();
    p.seal(&shard(64), args(SEND_PN_CAP - 1, 0)).expect("seal");
    assert_eq!(
        p.seal(&shard(64), args(SEND_PN_CAP, 0)).unwrap_err(),
        WireError::PnCapReached,
        "cap kontrolü monotonluktan sonraya kaymış"
    );
}

// ── TC-03 (b): SealInput yolları ────────────────────────────────────────────

#[test]
fn seal_with_empty_symbol_is_seal_input() {
    // E ≥ 11 invariant'ının seal-tarafı. `parse_inner`'ın "empty symbol" kapısıyla simetrik
    // olmalı: gönderilemeyen bir şey alınabilir de olmamalı.
    let mut p = protector();
    assert_eq!(
        p.seal(&shard(0), args(1, 0)).unwrap_err(),
        WireError::SealInput("empty symbol")
    );
}

#[test]
fn seal_with_non_monotonic_pn_is_seal_input() {
    // NONCE96 = LE64(full_pn), epoch nonce'a girmez → aynı epoch'ta PN tekrarı doğrudan
    // nonce tekrarıdır. Guard `<=` olmalı; `<` olsaydı EŞİT PN geçer ve keystream tekrarlanırdı.
    let mut p = protector();
    p.seal(&shard(64), args(10, 0)).expect("ilk seal");
    for pn in [0u64, 1, 9, 10] {
        assert_eq!(
            p.seal(&shard(64), args(pn, 0)).unwrap_err(),
            WireError::SealInput("non-monotonic wire_pn (nonce-reuse guard)"),
            "pn={pn} kabul edildi (nonce-reuse)"
        );
    }
    assert!(p.seal(&shard(64), args(11, 0)).is_ok());
}

#[test]
fn pn_floor_is_per_epoch_not_global() {
    // last_sealed_pn epoch BAŞINA tutulur. Epoch-1 yeni cipher-state'tir ve PN 0'dan başlar
    // (§4 EPOCH-PN-INIT) — epoch-0'daki yüksek PN, epoch-1'in düşük PN'ini bloklamamalı.
    // Guard global olsaydı epoch geçişi sonrası gönderim tıkanırdı.
    let mut p = protector();
    p.install_epoch1_keys(&key(0x33), &key(0x44));
    p.seal(&shard(64), args(1000, 0)).expect("epoch-0 pn=1000");
    assert!(
        p.seal(&shard(64), args(0, 1)).is_ok(),
        "epoch-1 PN tabanı epoch-0'dan miras alınmış"
    );
}

// ── TC-03 (c): WireSizeError::EmptySymbol ───────────────────────────────────

#[test]
fn sealed_len_with_empty_symbol_is_wire_size_error() {
    // sealed_len pacer-admission'da seal-ÖNCESİ çağrılır. Boyutlanamayan paket için tipli hata
    // dönmeli; 0 veya 37 dönmesi pacer'ı yanlış bütçelendirir.
    let p = protector();
    assert_eq!(p.sealed_len(&shard(0)), Err(WireSizeError::EmptySymbol));
    // Sınır: 1 byte symbol boyutlanabilir.
    assert_eq!(p.sealed_len(&shard(1)), Ok(26 + 11 + 1));
}

#[test]
fn sizer_and_seal_agree_on_rejection() {
    // Sözleşme simetrisi: `sealed_len` boyutlayamıyorsa `seal` de üretememeli, ve tersi.
    // Aksi hâlde pacer bir paketi kabul eder, seal düşer ve bütçe muhasebesi kayar.
    let mut p = protector();
    for e in [0usize, 1, 64, 1024] {
        let pkt = shard(e);
        match (p.sealed_len(&pkt), p.seal(&pkt, args(e as u64 + 1, 0))) {
            (Ok(want), Ok(sealed)) => assert_eq!(sealed.bytes.len(), want, "E={e}"),
            (Err(_), Err(_)) => {}
            (a, b) => panic!("E={e}: sizer/seal uyumsuz — sizer={a:?} seal={b:?}"),
        }
    }
}

// ── TC-04: EpochKeysMissing ─────────────────────────────────────────────────

#[test]
fn seal_on_epoch1_without_keys_is_epoch_keys_missing() {
    // Erişilebilir site: epoch-1 send-cipher'ı `install_epoch1_keys` çağrılmadan yoktur.
    // Fail-closed kanıtı: sessizce epoch-0'a düşmek veya sıfır anahtarla devam etmek
    // KABUL EDİLEMEZ — ikisi de gizliliği tamamen kaybettirir.
    let mut p = protector();
    assert_eq!(
        p.seal(&shard(64), args(1, 1)).unwrap_err(),
        WireError::EpochKeysMissing
    );
}

#[test]
fn no_silent_fallback_to_epoch0_on_missing_keys() {
    // Reddedilen epoch-1 seal, epoch-0 state'ini KİRLETMEMELİ: PN tabanı kaymamalı.
    let mut p = protector();
    assert!(p.seal(&shard(64), args(5, 1)).is_err());
    // epoch-0 hâlâ pn=1'den başlayabilmeli (epoch-1 reddi epoch-0 tabanını yükseltmedi).
    assert!(
        p.seal(&shard(64), args(1, 0)).is_ok(),
        "epoch-1 reddi epoch-0 PN tabanını kirletti"
    );
}

#[test]
fn seal_on_epoch1_succeeds_after_install() {
    let mut p = protector();
    p.install_epoch1_keys(&key(0x33), &key(0x44));
    let s = p.seal(&shard(64), args(1, 1)).expect("epoch-1 seal");
    assert_eq!(s.meta.dh_epoch, 1);
}

#[test]
fn open_on_epoch1_without_keys_is_epoch_not_accepted() {
    // [ÖLÇÜLDÜ] Modül-notu TC-04/a: open() yolundaki EpochKeysMissing site'ı ERİŞİLEMEZ —
    // epoch-kabul bayrağı anahtar kontrolünden önce düşer. Bu test GERÇEK davranışı pinler.
    // Kod sırası değişip EpochKeysMissing dönmeye başlarsa burada kırmızı yanar ve
    // modül notu ile birlikte güncellenmesi gerekir.
    let mut sender = KamaNoiseWireProtector::new(H_EXT, &key(0x33), &key(0x44));
    sender.install_epoch1_keys(&key(0x33), &key(0x44));
    let datagram = sender.seal(&shard(64), args(1, 1)).expect("seal").bytes;

    let mut receiver = protector(); // epoch-1 anahtarı YOK
    assert_eq!(
        receiver.open(&datagram).unwrap_err(),
        WireError::EpochNotAccepted(1),
        "open() epoch-1'i EpochKeysMissing ile değil EpochNotAccepted ile reddetmeli"
    );
}

#[test]
fn open_on_discarded_epoch0_is_epoch_not_accepted() {
    // Aynı yapısal sebep: discard sonrası recv_epoch_accepts[0]=false VE recv_cipher[0]=None
    // birlikte set edilir; kabul bayrağı önce düştüğü için EpochKeysMissing'e ulaşılmaz.
    let mut sender = KamaNoiseWireProtector::new(H_EXT, &key(0x22), &key(0x11));
    let datagram = sender.seal(&shard(64), args(1, 0)).expect("seal").bytes;

    let mut receiver = protector();
    receiver.discard_epoch0_recv();
    assert_eq!(
        receiver.open(&datagram).unwrap_err(),
        WireError::EpochNotAccepted(0)
    );
}
