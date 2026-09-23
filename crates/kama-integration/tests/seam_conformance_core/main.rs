//! **conformance-core** — sözleşmenin gözlenebilir davranışı (KM-SEAM-AT-001 §6).
//!
//! Bu ikilideki her test hedef-agnostiktir: gövdesi `common/seam/contract.rs` içinde TEK kopya
//! generic `fn`'dir, aşağıdaki makro çağrıları onu hedef başına ayrı `#[test]` olarak sarar.
//! Sahte ile gerçek arasında "ayrı ama benzer" test yoktur — aynı koddur.
//!
//! # Bugün tek hedef koşuyor
//!
//! `mock` kayıtlı; `real` DEĞİL (`GAP-SEAM-REAL-1`, bkz. `seam_real_adapter`). Gerçek adapter
//! indiğinde bu dosyaya eklenecek olan TEK şey iki satırdır:
//!
//! ```ignore
//! seam_mechanism_suite!(real_mechanism, harness::target::RealTarget);
//! seam_contract_suite!(real_contract, harness::target::RealTarget);
//! ```
//!
//! Süit gövdelerine dokunulmaz — iskeletin bütün iddiası budur.

#[path = "../common/seam/harness.rs"]
mod harness;

seam_mechanism_suite!(mock_mechanism, harness::target::MockTarget);
seam_contract_suite!(mock_contract, harness::target::MockTarget);
