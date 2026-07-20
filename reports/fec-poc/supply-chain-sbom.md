# FEC-POC — Supply-Chain SBOM (K-FEC-SUPPLYCHAIN-1)

> Kapsam: `kama-fec-codec`'in **normal (shipped) dependency-graph'ı**. Kabul-kriteri §14 K-FEC-SUPPLYCHAIN-1:
> exact-crate-version + checksum + license + SBOM + vendored/audited-source. registry-teyit (§0).

## 1. Doğrudan FEC-bağımlılığı

| crate | version | SPDX license | crates.io checksum (sha256) |
|---|---|---|---|
| reed-solomon-simd | 3.1.0 | `MIT AND BSD-3-Clause` | `cffef0520d30fbd4151fb20e262947ae47fb0ab276a744a19b6398438105a072` |

Spec §0 registry-teyit ile **byte-for-byte eşleşir** (ADR-16: reed-solomon-erasure trait arkasında; burada ErasureCodec
adaptörü = `RsSimdCodec`). raw-shard-API + erasure-only + sabit `original_count/recovery_count/shard_size`.

## 2. Transitif (reed-solomon-simd → normal edges)

| crate | version | SPDX license | checksum (sha256) |
|---|---|---|---|
| cpufeatures | 0.2.17 | `MIT OR Apache-2.0` | `59ed5838eebb26a2bb2e58f6d5b5316989ae9d08bab10e0e6d103e656d1b0280` |
| fixedbitset | 0.5.7 | `MIT OR Apache-2.0` | `1d674e81391d1e1ab681a28d99df07927c6d4aa5b027d7da16ba32d1d21ecd99` |
| once_cell | 1.21.4 | `MIT OR Apache-2.0` | `—` (workspace-ortak, önceden-mevcut) |

Şema-tarafı (`kama-transport-schema`) yalnız `thiserror 2.0.18` (`MIT OR Apache-2.0`) taşır; kripto/RS BİLMEZ.

## 3. Lisans-değerlendirmesi

- **Copyleft YOK.** Tüm graf permissive (MIT / Apache-2.0 / BSD-3-Clause). Savunma-dağıtım (musl statik-binary,
  ADR-19) için kısıt-üretmez. `MIT AND BSD-3-Clause` (reed-solomon-simd) = iki-permissive-lisansın birlikte-geçerliliği,
  attribution-koşulu (her ikisinin telif-bildirimi korunur).

## 4. Shipped-DEĞİL (dev/build-only, wire'a girmez)

`readme-rustdocifier 0.1.1` (`MIT`, checksum `08ad765b…`) ve `cpufeatures 0.3.0` normal-graf DIŞINDA (reed-solomon-simd
dev/doctest tarafı) — `cargo tree -p kama-fec-codec --edges normal` çıktısında **görünmez** → binary'ye linklenmez.

## 5. Durum

| Kriter | Durum |
|---|---|
| exact-version pin (Cargo.lock commit'li) | ✓ |
| checksum kayıtlı | ✓ (Cargo.lock `checksum =` alanları) |
| license permissive + copyleft-yok | ✓ |
| SBOM (bu-belge) | ✓ |
| vendored/audited-source | ○ AÇIK — D1: `cargo audit` + `cargo deny` CI-kapısı (CLAUDE.md §7); vendoring kararı D1-CI |

**HÜKÜM:** K-FEC-SUPPLYCHAIN-1 — version/checksum/license/SBOM **KARŞILANDI**; `cargo audit`/`deny` CI-entegrasyonu +
vendoring kararı D1-CI-adımına bırakıldı (registry-immutability + checksum-pin bugün-yeterli-taban).
