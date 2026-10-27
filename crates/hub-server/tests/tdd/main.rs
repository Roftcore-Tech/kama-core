//! KM-HUB-TDD-001 v0.6.1 §7 — hub'ın davranış testleri; sözleşme S4 v1.3.1. Her test adı TDD
//! kimliğini taşır (`t_b1_…` = T-B1). Hepsi IO'suz çekirdeğe karşı sahte saat ve bellek içi soketle
//! koşar; her datagramdan sonra tablo değişmezleri denetlenir (`World::deliver`). Gerçek UDP + ikili +
//! probe `tests/udp_loopback.rs`'te; bellek ölçümü `tests/dos_memory.rs`'te.

mod harness;

mod bind;
mod config;
mod data;
mod heartbeat;
mod memory;
mod resilience;
mod store;
mod wire_layout;
