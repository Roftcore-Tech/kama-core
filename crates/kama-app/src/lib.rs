//! Kama Application katmanı — iskelet (B1.0).
//!
//! Application ↔ Transport seam'i (S1) bu crate'te DEĞİLDİR: trait ve tipleri `kama-contracts`
//! crate'indedir (`kama_contracts::s1`, ortak tutamaçlar `kama_contracts::handles`); bkz.
//! `docs/seams/KM-SEAM-AT-001.md` §1 + §3. Sözleşme hiçbir katmana ait olmadığı için oraya taşındı
//! (R-7(b), 2026-10-05): burada dursaydı onu implemente eden Transport bu crate'e — alt katman üst
//! katmana — bağımlı olmak zorunda kalırdı.
//!
//! Mesaj codec'i (AMP, `specs/application-message-protocol.md`): [`codec`] (B2.1). Metin + alındı
//! (TEXT / RECEIPT; eş başına `msg_id`, dedup, outbox): [`messaging`] (B2.2). Dosya, PTT ve
//! bağlantı akışları B2.3–B2.5 kartlarıyla buraya gelir.
//! PTT yaşam döngüsü (`PTT_OPEN` / `PTT_CLOSE` + `MEDIA_BUNDLE` kabı): [`ptt`] (B2.4).
//!
//! Dosya aktarımı (`FILE_*`; parçalama, alıcı birleştirme, bütün-dosya özeti): [`file_transfer`]
//! (B2.3).
//!
//! Enjekte edilen monoton saat (`MonotonicClock` + `SystemClock`; PTT-R4, K4G-09 zamanlayıcıları):
//! [`clock`] (K4G-08).
//!
//! # Katman kuralı (INV-NEIGHBOR-1)
//!
//! Application yalnız `KamaTransport`'u çağırır. Crypto'ya ve Plane 1'e (hub-RPC) erişimi YOKTUR;
//! bu crate'in bağımlılık grafiğinde `kama-contracts` dışında hiçbir katman crate'i bulunamaz
//! (`scripts/ci/layer_check.py`). Tek ham primitif `blake2`'dir: DAR istisna, yalnız
//! `FILE_BEGIN.whole_file_hash` (Abkarada 2026-10-08). Application **retry yapmaz** (S1 K8): `send`
//! başarısızsa kullanıcıya yansıtır ya da bekler.

#![forbid(unsafe_code)]

pub mod clock;
pub mod codec;
pub mod file_transfer;
pub mod messaging;
pub mod ptt;
pub mod store;
