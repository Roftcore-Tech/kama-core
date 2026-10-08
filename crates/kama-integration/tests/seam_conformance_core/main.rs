//! **conformance-core** — sözleşmenin gözlenebilir davranışı (KM-SEAM-AT-001 §6).
//!
//! Bu ikilideki her test hedef-agnostiktir: gövdesi `common/seam/contract.rs` içinde TEK kopya
//! generic `fn`'dir, aşağıdaki makro çağrıları onu hedef başına ayrı `#[test]` olarak sarar.
//! Sahte ile gerçek arasında "ayrı ama benzer" test yoktur — aynı koddur.
//!
//! # İki hedef (L-T1, 2026-10-08)
//!
//! ~~`mock` kayıtlı; `real` DEĞİL (`GAP-SEAM-REAL-1`)~~ *[2026-10-08: kapandı]*. Gerçek adapter
//! indi ve bu dosyaya eklenen şey öngörülen iki satırdır (aşağıda). Süit gövdelerine dokunulmadı;
//! tek ekleme, bir hedefin koşamadığı satırı KİMLİKLİ beyan etmesidir (`SeamTarget::ROW_GAPS` →
//! `SEAM-ROW-GAP` sentinel'i → runner `real_gap` çapraz denetimi).

#[path = "../common/seam/harness.rs"]
mod harness;

seam_mechanism_suite!(mock_mechanism, harness::target::MockTarget);
seam_contract_suite!(mock_contract, harness::target::MockTarget);
seam_mechanism_suite!(real_mechanism, harness::real::RealTarget);
seam_contract_suite!(real_contract, harness::real::RealTarget);
