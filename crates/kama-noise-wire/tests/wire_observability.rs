//! TC-11 · TC-12 · TC-13 · TC-14 — Wire gözlemlenebilirliği (SR-STR-CORE-001 §5.3).
//!
//! **Kapsanan bulgular:** BUL-09 (wire'ın gözlemlenebilir yüzeyini sabitleyen test yok) ve
//! BUL-10 (reddetme sınıflarının dışarıdan ayırt edilebilirliği).
//!
//! **Bu dosya projenin tehdit modelinin DOĞRUDAN test karşılığıdır.** Proje metadata sızıntısını
//! birinci sınıf düşman sinyali sayar; buna karşılık bugüne kadar wire'da hangi byte'ların
//! cleartext olduğunu sabitleyen, k/r oranının paket boyutuna yansıyıp yansımadığını ölçen veya
//! `h_ext` rotasyonunun gözlemlenebilir tanıtıcıyı gerçekten değiştirdiğini kanıtlayan tek bir
//! assertion yoktu. "Wire'da ne görünüyor" bu projede bir INVARIANT'tır; dolayısıyla test edilir.
//!
//! **Kapsam dışı (açıkça):** Zamanlama kanalı. Bu dosya yalnız BYTE-düzeyi ve DURUM-düzeyi
//! gözlemlenebilirliği ölçer. Timing side-channel ayrı bir çalışmadır. [BEKLİYOR — Abkarada]

use kama_noise_wire::aad::AAD_DS;
use kama_noise_wire::error::{PnReject, WireError};
use kama_noise_wire::outer::{ExtendedOuter, OUTER_HDR_LEN, TAG_LEN};
use kama_noise_wire::protector::KamaNoiseWireSizer;
use kama_noise_wire::{HandleAssign, KamaNoiseWireProtector, SealArgs};
use kama_transport_schema::inner_kind::ShardKind;
use kama_transport_schema::{LogicalShardHeader, UnsealedTransportPacket, EXTENDED_V1};
use zeroize::Zeroizing;

const H_EXT: u32 = 0x0102_0304;

fn key(s: u8) -> Zeroizing<[u8; 32]> {
    Zeroizing::new([s; 32])
}

fn sender(h_ext: u32) -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(h_ext, &key(0x11), &key(0x22))
}

fn receiver(h_ext: u32) -> KamaNoiseWireProtector {
    KamaNoiseWireProtector::new(h_ext, &key(0x22), &key(0x11))
}

fn shard(
    kind: ShardKind,
    block_id: u32,
    esi: u16,
    k_actual: u8,
    e: usize,
) -> UnsealedTransportPacket {
    UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind,
            profile_id: 0x03,
            block_id,
            esi,
            k_actual,
        },
        symbol: vec![0xA5u8; e],
    }
}

fn args(wire_pn: u64) -> SealArgs {
    SealArgs {
        wire_pn,
        dh_epoch: 0,
        send_generation: 0,
    }
}

/// `needle` datagram içinde geçiyor mu (naif alt-dizi araması).
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

// ════════════════════════════════════════════════════════════════════════════
// TC-11 — Cleartext yüzey sabitleme (surface pin)
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cleartext_surface_is_exactly_the_first_ten_bytes() {
    // §2.6 EXTENDED-OUTER: h_ext(4) | wire_format(1) | wire_flags(1) | pn_trunc(4) = 10B cleartext.
    // Kalan her byte ciphertext veya tag'dir. Bu, bir refactor'ın inner header'ı yanlışlıkla
    // cleartext'e taşımasına karşı tek kapıdır.
    let mut a = sender(H_EXT);
    let pkt = shard(ShardKind::Source, 0xA5A5, 0x005A, 0x3C, 64);
    let d = a.seal(&pkt, args(0x0000_0005)).expect("seal").bytes;

    assert_eq!(d.len(), 26 + 11 + 64, "sealed FEC @E=64");

    // İlk 10 byte BYTE-BYTE pinli.
    assert_eq!(&d[0..4], &H_EXT.to_be_bytes(), "h_ext");
    assert_eq!(d[4], EXTENDED_V1, "wire_format");
    assert_eq!(d[5], 0x00, "wire_flags (dh_epoch=0, reserved=0)");
    assert_eq!(&d[6..10], &5u32.to_be_bytes(), "pn_trunc");

    // 10. byte'tan sonrası parse edilebilir bir header OLMAMALI: cleartext yüzey burada BİTER.
    let (outer, ct, tag) = ExtendedOuter::parse(&d).expect("outer parse");
    assert_eq!(outer.h_ext, H_EXT);
    assert_eq!(outer.dh_epoch, 0);
    assert_eq!(outer.pn_trunc, 5);
    assert_eq!(ct.len(), d.len() - OUTER_HDR_LEN - TAG_LEN);
    assert_eq!(tag.len(), TAG_LEN);
}

#[test]
fn fec_metadata_does_not_appear_anywhere_on_the_wire() {
    // Tanınabilir değerlerle bir shard mühürle ve o byte dizilerinin sealed datagram'da
    // BULUNMADIĞINI assert et. §2 Cryptex: FEC-semantiği (block_id/esi/k_actual/profile/kind)
    // TAMAMEN ciphertext'in içindedir; outer yalnız routing/PN/epoch + tag sızdırır.
    //
    // [KOD] tv_fec.rs başlığı 0x20-cleartext-framing yolunun EXTENDED-seal'e migre edildiğini
    // (METADATA-LEAK-01) kaydeder; bu test o migrasyonun KALICILIĞINI korur.
    let mut a = sender(H_EXT);
    let block_id: u32 = 0xA5A5_1234;
    let esi: u16 = 0x5A5A;
    let k_actual: u8 = 0x3C;
    let profile_id: u8 = 0x03;

    let pkt = UnsealedTransportPacket::FecShard {
        header: LogicalShardHeader {
            kind: ShardKind::Repair,
            profile_id,
            block_id,
            esi,
            k_actual,
        },
        // Symbol içeriği de tanınabilir olsun: sızarsa yakalansın.
        symbol: vec![0xC3u8; 64],
    };
    let d = a.seal(&pkt, args(0x0000_0007)).expect("seal").bytes;

    assert!(
        !contains(&d, &block_id.to_be_bytes()),
        "block_id wire'da cleartext görünüyor"
    );
    assert!(
        !contains(&d, &esi.to_be_bytes()),
        "esi wire'da cleartext görünüyor"
    );
    // Tek byte'lık alanlar için: 11 byte'lık inner header dizisinin TAMAMI aranır
    // (tek byte tesadüfen ciphertext'te bulunabilir; dizi bulunamaz).
    let inner_hdr = [
        0x01, // INNER_V1
        ShardKind::Repair.codepoint(),
        0x00, // flags
        profile_id,
        block_id.to_be_bytes()[0],
        block_id.to_be_bytes()[1],
        block_id.to_be_bytes()[2],
        block_id.to_be_bytes()[3],
        esi.to_be_bytes()[0],
        esi.to_be_bytes()[1],
        k_actual,
    ];
    assert!(
        !contains(&d, &inner_hdr),
        "inner header dizisi wire'da cleartext bulundu — §2 Cryptex ihlali"
    );
    // Symbol içeriği de sızmamalı (16 byte'lık pencere yeterli kanıt).
    assert!(
        !contains(&d, &[0xC3u8; 16]),
        "symbol içeriği wire'da cleartext görünüyor"
    );
    // AAD domain-separator wire'da YOK (AAD authenticated ama transmit edilmez).
    assert!(
        !contains(&d, AAD_DS),
        "AAD domain-separator wire'a yazılmış"
    );
}

#[test]
fn source_and_repair_are_indistinguishable_on_the_wire() {
    // Kritik: shard TÜRÜ (source mü repair mi) pasif gözlemciye FEC stratejisini verir.
    // İkisi de aynı cleartext yüzeye ve aynı sealed uzunluğa sahip olmalı; fark yalnız
    // ciphertext'in içindedir.
    let mut a = sender(H_EXT);
    let src = a
        .seal(&shard(ShardKind::Source, 7, 3, 8, 64), args(1))
        .expect("seal source")
        .bytes;
    let rep = a
        .seal(&shard(ShardKind::Repair, 7, 3, 8, 64), args(2))
        .expect("seal repair")
        .bytes;

    assert_eq!(
        src.len(),
        rep.len(),
        "source/repair uzunluk farkı SIZINTIDIR"
    );
    // Cleartext yüzeyde yalnız pn_trunc farklı olmalı (o zaten monoton ve gözlenebilir).
    assert_eq!(&src[0..6], &rep[0..6], "cleartext yüzeyde tür farkı var");
}

// ════════════════════════════════════════════════════════════════════════════
// TC-12 — k/r ayırt edilemezliği
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn parity_ratio_does_not_leak_through_per_shard_length() {
    // [KOD] engine-spec METADATA-LEAK-01 bunu EN HASSAS alan olarak işaretler: parity oranı,
    // kayıp/emergency koşulunu pasif gözlemciye sızdırır. Alpha'da KABUL EDİLMİŞ bir risktir —
    // ancak kabul edilen riskin BÜYÜKLÜĞÜ ölçülmemiştir. Bu test sızıntının hangi kanaldan
    // aktığını nicelendirir: uzunluk mu, sayı mı?
    //
    // Ölçüm: aynı profil (E sabit) altında k=8,r=2 ve k=8,r=6 bloklarının per-shard sealed_len
    // dağılımları AYNI olmalı. Fark yalnız SHARD SAYISINDA kalmalı.
    let p = sender(H_EXT);
    const E: usize = 64;

    let mut lens_r2: Vec<usize> = Vec::new();
    let mut lens_r6: Vec<usize> = Vec::new();

    // k=8, r=2 → 10 shard
    for esi in 0..8u16 {
        lens_r2.push(
            p.sealed_len(&shard(ShardKind::Source, 1, esi, 8, E))
                .unwrap(),
        );
    }
    for esi in 0..2u16 {
        lens_r2.push(
            p.sealed_len(&shard(ShardKind::Repair, 1, esi, 8, E))
                .unwrap(),
        );
    }
    // k=8, r=6 → 14 shard
    for esi in 0..8u16 {
        lens_r6.push(
            p.sealed_len(&shard(ShardKind::Source, 2, esi, 8, E))
                .unwrap(),
        );
    }
    for esi in 0..6u16 {
        lens_r6.push(
            p.sealed_len(&shard(ShardKind::Repair, 2, esi, 8, E))
                .unwrap(),
        );
    }

    // (1) Her iki blokta da TÜM shard'lar aynı uzunlukta — sıfır varyans.
    let uniq_r2: std::collections::BTreeSet<_> = lens_r2.iter().collect();
    let uniq_r6: std::collections::BTreeSet<_> = lens_r6.iter().collect();
    assert_eq!(
        uniq_r2.len(),
        1,
        "r=2 bloğunda uzunluk varyansı var: {uniq_r2:?}"
    );
    assert_eq!(
        uniq_r6.len(),
        1,
        "r=6 bloğunda uzunluk varyansı var: {uniq_r6:?}"
    );
    assert_eq!(
        uniq_r2, uniq_r6,
        "r oranı per-shard uzunluğa yansıyor — DOĞRUDAN SIZINTI"
    );

    // (2) KAYIT: sızıntı SAYIDA kalıyor. Bu, Beta+ padding/dummy çalışmasının hedefini tanımlar.
    //     Ölçmeden minimize edilemez.
    assert_eq!(lens_r2.len(), 10, "k=8,r=2 → 10 shard");
    assert_eq!(lens_r6.len(), 14, "k=8,r=6 → 14 shard");
    println!(
        "[TC-12 ÖLÇÜM] per-shard len sabit={} B · shard sayısı r2={} r6={} → \
         gözlemciye kalan kanal: SAYI (fark {} paket), UZUNLUK DEĞİL",
        lens_r2[0],
        lens_r2.len(),
        lens_r6.len(),
        lens_r6.len() - lens_r2.len()
    );
}

#[test]
fn k_actual_does_not_leak_through_length() {
    // K_actual (bloktaki gerçek source sayısı) da uzunluğa yansımamalı. Model-A'da zero-pad
    // slot'lar GÖNDERİLMEZ, yani sayı kanalı zaten açık; ama UZUNLUK kanalı kapalı kalmalı.
    let p = sender(H_EXT);
    let lens: Vec<usize> = (1..=8u8)
        .map(|k| {
            p.sealed_len(&shard(ShardKind::Source, 1, 0, k, 64))
                .unwrap()
        })
        .collect();
    let uniq: std::collections::BTreeSet<_> = lens.iter().collect();
    assert_eq!(uniq.len(), 1, "k_actual uzunluğa yansıyor: {lens:?}");
}

#[test]
fn sealed_length_is_a_pure_function_of_symbol_size() {
    // Genel hüküm: sealed_len YALNIZ symbol uzunluğunun fonksiyonudur. Header alanlarının
    // hiçbiri (kind/profile/block/esi/k_actual) boyutu etkilemez. Bu, TC-12'nin genellemesi.
    let p = sender(H_EXT);
    let base = p
        .sealed_len(&shard(ShardKind::Source, 0, 0, 1, 64))
        .unwrap();
    for kind in [ShardKind::Source, ShardKind::Repair] {
        for block_id in [0u32, 1, 0xFFFF_FFFF] {
            for esi in [0u16, 1, 0xFFFF] {
                for k in [1u8, 8, 255] {
                    assert_eq!(
                        p.sealed_len(&shard(kind, block_id, esi, k, 64)).unwrap(),
                        base,
                        "header alanı boyutu etkiliyor: {kind:?}/{block_id}/{esi}/{k}"
                    );
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// TC-13 — h_ext bağlanabilirliği ve rotasyon
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn h_ext_is_cleartext_and_constant_for_the_session_lifetime() {
    // `h_ext` cleartext outer'ın İLK 4 BYTE'ıdır ve oturum boyunca sabittir. Yani pasif
    // gözlemci için KALICI BİR KORELASYON TANITICISIDIR: aynı h_ext'i taşıyan tüm datagramlar
    // aynı oturuma aittir. Bu kabul edilmiş bir tasarım sonucudur; test onu KAYDA GEÇİRİR.
    let mut a = sender(H_EXT);
    for pn in 1..=32u64 {
        let d = a
            .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(pn))
            .expect("seal")
            .bytes;
        assert_eq!(
            &d[0..4],
            &H_EXT.to_be_bytes(),
            "pn={pn}: h_ext oturum içinde değişti"
        );
    }
}

#[test]
fn pn_trunc_is_cleartext_and_monotonic() {
    // İkinci kalıcı gözlem kanalı: pn_trunc cleartext ve monoton. Gözlemci bundan paket
    // SAYISINI ve (boşluklardan) KAYIP ORANINI türetebilir.
    // [BEKLİYOR — Abkarada, AÇIK-7] Bu Alpha'da kayıtlı bir kabul mü?
    let mut a = sender(H_EXT);
    let mut seen: Vec<u32> = Vec::new();
    for pn in [1u64, 2, 3, 10, 11, 100] {
        let d = a
            .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(pn))
            .expect("seal")
            .bytes;
        seen.push(u32::from_be_bytes([d[6], d[7], d[8], d[9]]));
    }
    assert_eq!(
        seen,
        vec![1u32, 2, 3, 10, 11, 100],
        "pn_trunc cleartext okunabiliyor"
    );
    assert!(seen.windows(2).all(|w| w[0] < w[1]), "pn_trunc monoton");
}

#[test]
fn rotating_h_ext_actually_changes_the_observable_identifier() {
    // Rotasyon mekanizması kodda mevcut (HANDLE_ASSIGN yeni h_ext taşır). Ancak rotasyonun
    // GÖZLEMLENEBİLİR TANITICIYI FİİLEN DEĞİŞTİRDİĞİNE dair test yoktu.
    // **Rotasyon varsa ve etkisizse, korunma YANILSAMASI üretir.**
    let old_h = 0x1111_1111u32;
    let new_h = 0x2222_2222u32;

    let assign_old = HandleAssign {
        session_attempt_id: 1,
        init_counter: 100,
        h_legacy: 0xAAAA_AAAA,
        h_ext: old_h,
    };
    let assign_new = HandleAssign {
        session_attempt_id: 2,
        init_counter: 101,
        h_legacy: 0xBBBB_BBBB,
        h_ext: new_h,
    };
    assert_ne!(
        assign_old.h_ext, assign_new.h_ext,
        "rotasyon aynı h_ext'i atıyor"
    );
    assert_ne!(
        assign_old.replay_identity(),
        assign_new.replay_identity(),
        "rotasyon aynı replay-identity'yi üretiyor"
    );

    // Wire üzerinde tanıtıcı fiilen değişiyor mu?
    let mut a_old = sender(old_h);
    let mut a_new = sender(new_h);
    let d_old = a_old
        .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(1))
        .expect("seal")
        .bytes;
    let d_new = a_new
        .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(1))
        .expect("seal")
        .bytes;

    assert_eq!(&d_old[0..4], &old_h.to_be_bytes());
    assert_eq!(&d_new[0..4], &new_h.to_be_bytes());
    assert_ne!(&d_old[0..4], &d_new[0..4], "rotasyon sonrası tanıtıcı aynı");

    // Ciphertext de değişmeli: h_ext AAD'nin parçasıdır → aynı plaintext + aynı PN olsa bile
    // tag farklı olur. Aksi hâlde gözlemci tag'ler üzerinden oturumları bağlayabilirdi.
    assert_ne!(
        &d_old[OUTER_HDR_LEN..],
        &d_new[OUTER_HDR_LEN..],
        "h_ext AAD'ye girmiyor — ciphertext/tag rotasyondan etkilenmiyor"
    );
}

#[test]
fn datagram_with_old_h_ext_is_rejected_after_rotation() {
    // Rotasyonun ANLAMI eski tanıtıcının geçersizleşmesidir. Eski h_ext'li bir datagram
    // yeni protector tarafından reddedilmeli; kabul edilirse rotasyon kâğıt üstünde kalır.
    let old_h = 0x1111_1111u32;
    let new_h = 0x2222_2222u32;

    let mut a_old = sender(old_h);
    let stale = a_old
        .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(1))
        .expect("seal")
        .bytes;

    let mut b_new = receiver(new_h);
    match b_new.open(&stale) {
        Err(WireError::InnerParse("h_ext mismatch")) => {}
        other => panic!("eski h_ext'li datagram reddedilmedi: {other:?}"),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// TC-14 — Reddetme ayırt edilemezliği
// ════════════════════════════════════════════════════════════════════════════

/// Reddetme sınıfları — `h_ext`'i BİLEN ama anahtarı BİLMEYEN bir gözlemcinin üretebileceği
/// geçersiz datagram türleri.
fn invalid_datagram_classes(valid: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    let mut out: Vec<(&'static str, Vec<u8>)> = Vec::new();

    out.push(("kısa (25B)", valid[..25].to_vec()));

    let mut fmt = valid.to_vec();
    fmt[4] = 0x02; // bilinmeyen wire_format
    out.push(("bilinmeyen format", fmt));

    let mut flags = valid.to_vec();
    flags[5] |= 0b1000_0000; // reserved flag
    out.push(("reserved flag", flags));

    let mut old_pn = valid.to_vec();
    old_pn[6..10].copy_from_slice(&0u32.to_be_bytes()); // pencere-dışı geçmiş
    out.push(("pencere-dışı PN", old_pn));

    let mut future_pn = valid.to_vec();
    future_pn[6..10].copy_from_slice(&0x00FF_FFFFu32.to_be_bytes()); // future-gap
    out.push(("future-gap PN", future_pn));

    let mut bad_tag = valid.to_vec();
    let n = bad_tag.len();
    bad_tag[n - 1] ^= 0xFF; // geçersiz tag
    out.push(("geçersiz tag", bad_tag));

    let mut bad_ct = valid.to_vec();
    bad_ct[OUTER_HDR_LEN] ^= 0xFF; // ciphertext bozma
    out.push(("bozuk ciphertext", bad_ct));

    out
}

#[test]
fn no_rejection_class_emits_bytes_or_advances_state() {
    // [KOD] `open()` PN-precheck'i AEAD doğrulamasından ÖNCE yapar. Bu maliyet açısından
    // standarttır ve pencerenin AEAD-fail'de ilerlemediği zaten test edilmiştir
    // (`sprint1_core::aead_fail_does_not_advance_window`).
    //
    // Buradaki soru farklıdır: `h_ext`'i bilen KİMLİKSİZ bir gözlemci, reddetme sınıflarını
    // birbirinden AYIRT EDEBİLİYOR MU? `error.rs` reject-reason'ların İÇERİK sızdırmadığını
    // belirtir; burada sızabilecek olan içerik değil DURUMDUR.
    //
    // **Oracle:** Hiçbir reddetme sınıfı için wire'a byte çıkmaz ve state izi oluşmaz.
    // `open()` `Result` döndürür — yanıt üretmez — dolayısıyla byte kanalı yapısal olarak
    // kapalıdır. Gözlemlenebilir tek state, `recv_expected_pn`'dir.
    let mut a = sender(H_EXT);
    let valid = a
        .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(1))
        .expect("seal")
        .bytes;

    let mut b = receiver(H_EXT);
    let pn_before = b.recv_expected_pn(0);

    for (name, bad) in invalid_datagram_classes(&valid) {
        let res = b.open(&bad);
        assert!(res.is_err(), "{name}: geçersiz datagram KABUL EDİLDİ");
        assert_eq!(
            b.recv_expected_pn(0),
            pn_before,
            "{name}: reddetme recv-penceresini ilerletti (state izi)"
        );
    }

    // Meşru paket hâlâ açılmalı — guard'lar meşru trafiği bloklamamış olmalı.
    assert!(
        b.open(&valid).is_ok(),
        "reddetme selinden sonra meşru paket açılamadı"
    );
}

#[test]
fn rejection_reason_taxonomy_is_recorded_not_asserted_equal() {
    // **[ÖLÇÜLDÜ — bu dosya] BUL-10'un NİCELENDİRİLMESİ.** Reddetme sınıfları İÇ teşhis için
    // AYRI variant'lar döndürür. Bu test onların ne olduğunu KAYDA GEÇİRİR — eşitlemeye
    // ÇALIŞMAZ, çünkü iç teşhis ayrımı kasıtlıdır ve değerlidir.
    //
    // Kritik ayrım şudur: bu variant'lar DIŞARIYA çıkmadığı sürece (yanıt yok, state izi yok)
    // gözlemci onları göremez. Yukarıdaki test byte/state kanalının kapalı olduğunu kanıtlar.
    // Bu test, ileride biri reject-reason'ı bir RST/NACK paketine koymaya kalkarsa hangi
    // bilginin sızacağını görünür kılar.
    let mut a = sender(H_EXT);
    let valid = a
        .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(1000))
        .expect("seal")
        .bytes;
    let mut b = receiver(H_EXT);
    b.open(&valid).expect("pencereyi ilerlet");

    let mut taxonomy: Vec<(&str, String)> = Vec::new();
    for (name, bad) in invalid_datagram_classes(&valid) {
        let err = b.open(&bad).unwrap_err();
        taxonomy.push((name, format!("{err:?}")));
    }
    for (name, err) in &taxonomy {
        println!("[TC-14 TAKSONOMİ] {name:<22} → {err}");
    }

    // Pinlenen hüküm: PN reddi ile AEAD reddi İÇERİDE ayrıştırılabiliyor.
    // Bu, expected_pn penceresinin haritalanabilirliği açısından kayıt altına alınan risktir.
    let has_pn_reject = taxonomy.iter().any(|(_, e)| e.contains("PnReject"));
    let has_aead_fail = taxonomy.iter().any(|(_, e)| e.contains("AeadFail"));
    assert!(
        has_pn_reject && has_aead_fail,
        "beklenen iç taksonomi değişmiş: {taxonomy:?}"
    );
}

#[test]
fn pn_reject_variants_are_reachable_and_distinct() {
    // BUL-10'un dayanağı: `PnReject`'in üç variant'ı da fiilen üretilebiliyor. Bunlar
    // `expected_pn` penceresinin YAPISINI kodlar; dışarı sızarlarsa gözlemci pencereyi
    // anahtarsız haritalayabilir.
    let mut a = sender(H_EXT);
    let mut b = receiver(H_EXT);

    // Pencereyi ilerlet.
    for pn in 1..=5u64 {
        let d = a
            .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(pn))
            .expect("seal")
            .bytes;
        b.open(&d).expect("open");
    }

    // Duplicate: aynı datagramı tekrar besle.
    let dup = a
        .seal(&shard(ShardKind::Source, 1, 0, 8, 64), args(6))
        .expect("seal")
        .bytes;
    b.open(&dup).expect("ilk kez açılmalı");
    assert_eq!(
        b.open(&dup).unwrap_err(),
        WireError::PnReject(PnReject::Duplicate),
        "duplicate variant üretilemedi"
    );

    // FutureGap: EXTENDED_PN_PARAMS.future_gap_max = 2^13.
    let mut far = dup.clone();
    far[6..10].copy_from_slice(&(6u32 + (1 << 14)).to_be_bytes());
    assert_eq!(
        b.open(&far).unwrap_err(),
        WireError::PnReject(PnReject::FutureGap),
        "future-gap variant üretilemedi"
    );

    // Pencere ilerlemedi mi? (Reddedilen sondalar iz bırakmamalı.)
    assert_eq!(b.recv_expected_pn(0), 7, "reddetmeler pencereyi kaydırdı");
}
