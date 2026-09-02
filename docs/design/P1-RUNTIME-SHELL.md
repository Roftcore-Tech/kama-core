# P1 — Transport Runtime Kabuğu (client-core) — Tasarım

**Statü:** ONAYLI PLAN (Abkarada "P1'i başlat", 2026-08-26) → implementasyon. Kapsam:
`PRODUCT-RUNTIME-INTEGRATION-PENDING` item **2,3,6,7,8,9** (4-5 = güvenlik düzlemi, AYRI workstream
— SoC düzeltmesi `1b38936d`). Keşif: 4 read-only harita (udp_runner/ingress/bench-ref/engine-API).

## Kararlar (haritalardan)

| # | karar | gerekçe |
|---|---|---|
| D1 | `udp_runner.rs` REFERANS-ONLY (kod alınmaz) | kendini "bench-runner, üretim-yoluna-bağlı-değil" ilan ediyor; blocking-std; feature-gated; test-only impairment iç içe |
| D2 | Döngü iskeleti = bench `kama.rs`'in kanıtlanmış şekli | kampanya bu döngüde koştu: tek-task `select!`, PACE 1ms→`Pto` (pompa), RTPROP 200ms, ACK 20ms (alıcı), tek `Instant`/taraf, `elapsed_ns` damgası |
| D3 | `next_wakeup_ns()`'e TEK-kaynak-uyku YOK; 1 ms Pto pompası esas | EK4-A2 tarihi; pompa idempotent + kampanya-kanıtlı. (Gelecek optimizasyon: pompa + wakeup hibrid) |
| D4 | Güvenlik dikişi = `SecuritySeam` trait (datagram seal/open), `IdentitySeam` stub (+16B tag, bench TAG_STUB soyu) | INV-ENGINE-SOC: motor opak birim; Noise/TLS/PQC B-hattında bu trait'e takılır. ReceiverControlCore zaten aynı şekli kanıtlıyor (`install_session_with_directional_keys`) |
| D5 | Wire-v0 = bench-soyu çerçeve (DATA 0x00: seq16+unit64+payload; ACK 0x10/0x14 `ack.rs`) | ccr §35 SESSION_DATA finalizasyonu güvenlik-düzlemi entegrasyonunda (B-hattı + P4 kama-contract); v0 dikişin ARKASINDA — swap noktası seam |
| D6 | Alıcı = `adaptive_transport::Receiver` / eş (peer) — sınırlı tablo + idle-emeklilik | bench'in sınırsız `HashMap<SocketAddr,Receiver>`'ı SR-AUDIT-001 ihlali; cap + patience-tick eklenir |
| D7 | Tüm tahsisler sınırlı + typed-refusal | offer-kanalı bounded; unit-store bounded (ack'te düşer); peer-cap fail-closed; teslim-kanalı bounded (`try_send`, sayaçlı) — motor `tx_queue`'yu SINIRLAMAZ (harita-3), kabuk sınırlar |
| D8 | Yaşam döngüsü: `Running → Draining(cap'li) → Closed`; drain = pif→0 VEYA süre-tavanı | motor birim terk etmez → drain MUTLAKA cap'li (bench 337-362 deseni) |
| D9 | Hata sınıfı ayrımı korunur: ağ gürültüsü=Ok(Dropped), invariant=Err (stop-the-line) | harita-1: blanket-log-and-continue YASAK |
| D10 | ReceiverControlCore/Host kablolaması P1-FAZ-2'de (kontrol-düzlemi dalı + echo-drain); bu faz veri/ACK düzlemi | core kendi AEAD'ını taşıyor (gerçek protector, enjekte anahtar) — IdentitySeam'le iki ayrı kripto düzlemi karışmasın; anahtar enjeksiyonu seam işi |

## Modüller (client-core)

```
seam.rs     SecuritySeam trait + IdentitySeam (stub; Noise B-hattında NoiseWireSeam)
wire.rs     v0 çerçeve codec (fixed-width BE, ADR-05; varint YOK)
runtime.rs  ClientEndpoint / ServerEndpoint + görev döngüleri + RuntimeError (thiserror)
```

## Sınırlar (SR-AUDIT-001, bounds-before-alloc)

| kaynak | sınır | aşımda |
|---|---|---|
| app offer kanalı | `offer_queue` (default 1024) | `try_send` → `RuntimeError::QueueFull` (typed) |
| unit-store (unit_id→payload) | `max_inflight_units` (default 4096) | offer alınmaz (doğal backpressure) |
| seq→unit haritası | unit-store disipliniyle (ack'te birimin TÜM seq'leri düşer) | — |
| server peer tablosu | `max_peers` (default 64, SESSION_COUNT_CAP hizası) | yeni peer datagram'ı typed-drop + sayaç (sessiz-eviction YOK) |
| teslim kanalı | bounded (default 1024) | `try_send` başarısız → `delivery_dropped` sayacı (app drenajı sözleşmesi) |

## Kapılar (item 8/9)

1. **In-process loopback e2e (CI, saniyeler):** gerçek UDP 127.0.0.1, N birim → hepsi teslim+ACK;
   shutdown-drain temiz; bounded-refusal typed.
2. **netem süreç-düzeyi kapısı:** `examples/p1_endpoint` (client/server rolleri) + iki-netns %5 kayıp
   + 40 ms RTT → N=100 birim %100 teslim (tail-ON kuyruk dahil), kaynak-sınırı ihlali yok.

## Sıralama — SONUÇ (2026-08-26): FAZ-1 + FAZ-2 TAMAM (`reports/p1-runtime-shell/P1-CLOSURE.md`)
Item-6 kararı: kontrol-düzlemi UDP-kablolaması B-hattı S3 ile tamamlanır (kapanış raporu §item-6;
kütüphane-düzeyi kanıt CLOSURE-1B'de mevcut; v0'da dal-noktası `unknown_frames` ile görünür).

FAZ-1 (bu commit dizisi): seam + wire + runtime (client+server) + loopback e2e + example + netem kapısı.
FAZ-2: ReceiverControlCore kontrol-düzlemi dalı + patience/idle emeklilik incelikleri + shutdown/kaynak
doğrulaması genişletmesi (item 9 tamamı) + kapanış raporu.
