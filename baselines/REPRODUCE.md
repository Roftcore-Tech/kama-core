# KAMA ↔ TCP kampanyası — TEKRAR PAKETİ

Bu dizin, `reports/kama-vs-tcp-v1/` altındaki donmuş manifesti temiz bir Linux makinede (rootless)
yeniden koşup **aynı CSV şemasını** üretmek için gereken her şeyi taşır. Çizim/istatistik kütüphanesi
gerekmez; Python yalnız stdlib.

## Gereksinimler

| ne | sürüm (ölçümün yapıldığı) | not |
|---|---|---|
| Linux çekirdeği | 7.0.12 | `sch_netem` modülü; netem `seed` parametresi için ≥ 6.6 |
| iproute2 (`tc`, `ip`) | 7.2.0 | `tc -j`, `netem … seed N` |
| util-linux (`unshare`, `nsenter`) | — | **rootless**: `unshare -rnm` (kullanıcı ad alanı açık olmalı) |
| `ping` | iputils | bağlantı kapısı |
| Python | 3.14 (yalnız stdlib) | 3.9+ yeterli |
| Rust | rustc 1.96.0 | yalnız adapter'ları derlemek için; **bağımlılık yok** |
| `ethtool` | GEREKMEZ | offload'lar `campaign/kvt_offload.py` ile (SIOCETHTOOL ioctl) kapatılıp geri okunur |

root, docker, paket kurulumu **gerekmez**.

## Kollar ve binary'ler

| kol | binary | kaynak |
|---|---|---|
| `tcp-std` | `baselines/target/release/tcp_adapter` | bu dizin (`cargo build --release`; yalnız `std`) |
| kalibrasyon | `baselines/target/release/udp_probe` | bu dizin |
| `kama-v1-full` | `target/release/bench` (`--transport kama-product`) | ana repo; **önceden derlenmiş binary + sha256 kabul edilir** (`reports/kama-vs-tcp-v1/FREEZE.md`) |
| `abl-off` / `abl-on` | `target/release/examples/dr_arqtail_product` | ana repo (bileşen ablasyonu; ürün DEĞİL) |

Binary yolları ortam değişkeniyle değiştirilebilir: `KVT_BENCH`, `KVT_TCP`, `KVT_ABL`.

## Tek komut

```bash
# 1) adapter'ları derle (makine güvenliği: tek cargo, 2 iş)
( cd baselines && CARGO_BUILD_JOBS=2 cargo build --release )
# 2) Kama binary'leri: ya hash'i FREEZE.md ile eşleşen hazır binary'yi target/release/ altına koy,
#    ya da:  CARGO_BUILD_JOBS=2 cargo build --release -p bench --bin bench
#            CARGO_BUILD_JOBS=2 cargo build --release -p adaptive-transport --example dr_arqtail_product
# 3) koş (çıktı dizini KALICI diskte olsun):
bash baselines/campaign/kvt_campaign.sh reports/kama-vs-tcp-v1 "$HOME/kvt-out"
# 4) analiz:
python3 baselines/campaign/kvt_analyze.py \
  --manifest R1=reports/kama-vs-tcp-v1/manifest-R1.csv --manifest R2=reports/kama-vs-tcp-v1/manifest-R2.csv \
  $(for r in R1 R2; do for w in 0 1 2 3; do echo --result $r=$HOME/kvt-out/$r/w$w/result.csv; done; done) \
  --out "$HOME/kvt-out/analysis"
```

Beklenen süre: 4 işçiyle ≈ 6–7 saat (R1 ≈ 3 saat + R2 ≈ 3 saat). Koşu **devam-edilebilir**: yarıda
kesilirse aynı komut kaldığı yerden sürer (üretilmiş satır yeniden koşulmaz, üstüne yazılmaz).
Uzun koşuda oturum kapanmasına karşı: `setsid systemd-inhibit --what=sleep:idle bash …`.

## Ön-uçuş (kampanyadan ÖNCE; sonuçları `reports/kama-vs-tcp-v1/preflight/`)

```bash
unshare -rnm python3 baselines/campaign/kvt_calibrate.py  OUT/calibration   # kanal kalibrasyonu (≈ 5 dk)
unshare -rnm python3 baselines/campaign/kvt_tcp_sanity.py OUT/tcp-sanity    # Mathis + nc (≈ 12 dk; nc varsa)
```

Kalibrasyonda `accept=0` çıkan hücre kampanyadan çıkarılmalıdır. Mathis oranı 0.3–3 dışındaysa
DUR ve nedenini araştır (offload, kuyruk limiti, rate; kısa akış = slow-start rejimi).

## Dosyalar

```
src/bin/tcp_adapter.rs      TCP kolu (çerçeve u32 len ‖ u64 id ‖ yük; teslim = uygulama-ACK'inin gönderene varışı)
src/bin/udp_probe.rs        kalibrasyon probe'u
campaign/kvt_manifest.py    deney tanımı → manifest (a-priori formüller burada belgeli)
campaign/kvt_exec.py        yürütücü (fail-closed netem doğrulama, seed, sayaçlar, devam-edilebilir)
campaign/kvt_run.sh         rootless sarmalayıcı        campaign/kvt_campaign.sh  tek-komut koşucu
campaign/kvt_topo.sh        3-netns topoloji (scripts/netem/kapi3_harness.sh'ı kullanır)
campaign/kvt_offload.py     TSO/GSO/GRO kapat + doğrula  campaign/kvt_netem_check.py  tc dökümü ↔ spec
campaign/kvt_row.py         ham CSV satırı               campaign/kvt_analyze.py      donmuş analiz
campaign/kvt_calibrate.py   Faz 0.5 kalibrasyon          campaign/kvt_tcp_sanity.py   Faz 0.5 TCP sağlamaları
```

## Çıktı şeması (`result.csv`, 40 sütun)

`run_id, replica, group, cell, loss_pct, rtt_class, owd_ms, rate, workload, msgs, payload, arm, mode, rep,
horizon_ms, deadline_ms, status, delivered, complete_horizon, complete_deadline, t_complete_ms, wire_bytes,
wire_tx_bytes, wire_rx_bytes, fwd_pkts, fwd_drops, rev_pkts, rev_drops, netem_limit, netem_seed_fwd,
netem_seed_rev, loadavg1, load_flag, warmup_residue, setup_ns, connect_failed, profile_id, rc, seq_no, unix_ts`

* `wire_bytes` = istemci veth arayüzü (`va0`) `ip -s link` bayt sayaçlarının koşu öncesi/sonrası farkı (tx+rx) — her kolda aynı yöntem.
* `fwd_/rev_ pkts,drops` = orta-kutu netem qdisc sayaç FARKLARI (`packets` = iletilen, `drops` = düşürülen; sayaçlar `replace`te sıfırlanmaz → fark alınır).
* Tekrarlanabilirlik: netem seed'lidir (satır başına `netem_seed_*`); aynı seed aynı paket-indeksi kayıp dizisini üretir, ancak protokol zamanlaması dizinin hangi pakete düştüğünü değiştirebilir → sonuçlar **istatistiksel** olarak tekrarlanır, bit-bit değil.
