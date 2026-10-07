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
//!
//! # Katman kuralı (INV-NEIGHBOR-1)
//!
//! Application yalnız `KamaTransport`'u çağırır. Crypto'ya ve Plane 1'e (hub-RPC) erişimi YOKTUR;
//! bu crate'in bağımlılık grafiğinde `kama-contracts` dışında hiçbir katman crate'i bulunamaz
//! (`scripts/ci/layer_check.py`). Application **retry yapmaz** (S1 K8): `send` başarısızsa
//! kullanıcıya yansıtır ya da bekler.

#![forbid(unsafe_code)]

pub mod codec;
pub mod messaging;
