# PARK LİSTESİ

Bilinçli olarak ertelenmiş kararlar. Her kalem: ne, hangi tetikle geri gelir, hangi kapıyı beklettiği.
Bir kalem buradaysa "unutuldu" değil "şimdi değil" demektir. Kalem kapanınca satır silinmez, üstü çizilir.

| # | kalem | tetik | beklettiği kapı | kaynak |
|---|---|---|---|---|
| P-1 | **Beta — toplu initiate (grup / multi-device / sealed sender) → KS-DOS-1 yeniden kalibrasyonu** | ccr §35.15 T1–T3 (grup fan-out · multi-device · sealed sender) | yok | KS-DOS-1 kapanışı, 2026-09-24 |
| P-2 | **Plane 0 handshake hız sınırı — bind öncesi kaynak = UDP 4-tuple** (INIT kapsamından çıkarıldı; INIT bind sonrası gelir) | M6 (veriyle) | yok | ccr §35.15 `[KARAR — M6]`, 2026-09-24 |
| P-3 | **Ürün notu — Google servissiz savunma dağıtımında arka plan erişilebilirliği:** "ağ dönünce anında bağlan" ancak foreground service ile tutar; doze'da prob kadansı OS'undur | ürün / Android kararı | yok (ürün/Android) | DR-CONNECT-1 H12, 2026-10-03 |
| P-4 | **Link/power sinyali + UniFFI bağlaması** (link up/down · ön plan/arka plan/doze · kullanıcı etkileşimi) — DR-CONNECT-1 K5: bugün `client-core`/`adaptive-transport`'ta yok. O gelene kadar prob/heartbeat kadansı ÖN-PLAN davranışıdır (8 s); askıya alma sinyali yoksa askıya alma da yoktur | ürün / Android (ADR-15 coarse-grained UniFFI) | yok | transport-tdd §10 [AMENDMENT-10] madde 6, 2026-10-04 |
| ~~P-5~~ | ~~**Tema E — iki ACK ailesinden hangisi ürün ACK'i**~~ **KAPANDI 2026-10-04 (K3):** ürün ACK'i engine `ACK_FRAME_V2`; noise-wire `Control::Ack/Sack` silindi; onay taşıyıcısı boş batch. Park'ta bekleyen bir kalem değildi; kapanış kaydı olarak düşüldü | — | — | DR-CONNECT-1 günün paketi K3 |
