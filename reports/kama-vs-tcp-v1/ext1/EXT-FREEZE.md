# kama-vs-tcp-v1 — EK-1 (ext1) ÖN-KAYIT ve DONDURMA

Tarih: 2026-09-20 23:40 · v1 sonuç commit'i `b8745e30` GÖRÜLDÜKTEN SONRA tasarlandı (bu bir EK'tir; v1 manifesti/sonuçları DEĞİŞMEZ, satırları yeniden kullanılmaz).
Sahip talimatı: v1 raporunun "Yapılmadı" listesindeki koşulabilir kalemler koşulsun. **Bu dosya + manifestler + analizör ölçümden ÖNCE commit'lenir.**

## Kapsam
| blok | ne | hücreler |
|---|---|---|
| Q | **QUIC kolu** (`bench --transport quinn` = quinn-reliable-stream, CUBIC, mesaj başına bi-stream; TLS 1.3 quinn'de KAPATILAMAZ) | IID simetrik {10,30,50}% × RTT {low,med} + {20,30}% × RTT high · small+large |
| B | **bant ekseni** `rate 256kbit` (iki yön) | IID simetrik {10,30}% × RTT {low,med} · small+large |
| G | **burst** Gilbert–Elliott (DR-ARQ-PRODUCT-TAIL-1 parametrizasyonu `loss gemodel p 50%`, p=50L/(100−L)), yalnız İLERİ yön | L {10,20}% × RTT {low,med,high} · small+large |

Üç kol (`kama-v1-full`, `tcp-std`, `quinn`) HER hücrede ardışık ve dönüşümlü (taze satırlar). N=30/kol/hücre/replika; R1,R2 ayrık seed (alan `KVT1X`); seed kollar arasında ortak.
Horizon/deadline formülü v1 ile AYNI: H=⌈10 s+10·RTT+10·serialize⌉, D=3×(serialize+1.5·RTT), serialize=yük·8/rate. Satır: replika başına 3 240 → **6 480**.

**KOŞULMAYANLAR ve nedeni:** KCP (crate yok; `baselines/` yalnız-std kuralı; ADR-06 "KCP yok") · terminal-tail (ürün yolunda konumsal düşürme yok; çıplak-Sender OFF/ON
grid'i TAMAMLANMIŞ — yeniden koşmak yasak: OFF 8/360, ON 329/360) · RF/jammer (veri yok) · TLS'li TCP (yeni bağımlılık gerekir).

## Hipotez (sonuçtan ÖNCE)
1. **Q:** yüksek kayıpta sıra Kama ≥ QUIC ≥ TCP; düşük kayıpta (%10) üç kol ayırt edilemez. QUIC'in mesaj başına akışı TCP'nin HoL'unu taşımadığından large yükte TCP'den iyi olmalı.
2. **B:** 256 kbit'te large yük serialize ≈ 3.1 s → horizon 42–44 s'ye uzar; %30 kayıpta makas v1'deki rate'siz hücreye göre DARALMAZ.
3. **G:** aynı ortalama kayıpta burst, IID'ye göre kolları birbirine YAKLAŞTIRIR (Faz-5 T2b N=30'da PARITY vermişti); Kama lehine ayrışma beklenmiyor.
Çürütülürse AYNEN raporlanır.

## v1 harness'ına göre DEĞİŞİKLİKLER (ek için; v1 sonuçlarını etkilemez)
* `kvt_exec.py`: `quinn` kolu eklendi; **`set_netem` artık sil+ekle** (`replace` rate'i KORUYORDU — rate'li hücreden rate'siz hücreye geçişte 32 000 B/s kaldı; fail-closed doğrulama
  duman koşusunda YAKALADI, satır üretilmedi. v1'de rate hiç kullanılmadığı ve her v1 satırı sayısal doğrulandığı için v1 ETKİLENMEDİ); harness tavanı horizon+65→+110 s (v1'de 1 satır rc=124).
* `kvt_netem_check.py`: gemodel (p, r) sayısal doğrulaması. · `kvt_campaign.sh`: `KVT_PREFIX`, `KVT_WORKERS`.
* **6 işçi** (v1: 4). v1'de 4 işçiyle yük ortalaması 0.2–1.3 idi; 6 işçi AYRICA doğrulanmadı → `loadavg1>2.0` işareti + duyarlılık tablosu geçerli. Kollar aynı işçide dönüşümlü.
* Tel maliyeti yöntemi v1 ile aynı (va0 veth bayt farkı, kurulum+ısıtma DAHİL) → QUIC'te TLS el sıkışması baytları dahildir.
* Analiz: `kvt_analyze_ext.py` — v1 kuralları (Wilson %95, ayrışma şartı, horizon metriği), üç kol için İKİLİ hükümler (K↔T, K↔Q, Q↔T).

## Donmuş kimlikler (sha256)
| nesne | sha256 |
|---|---|
| `ext1/manifest-R1.csv` | `af8db284136cf3d61ae2e190db663dc5fe37f8c41aa198644e3d3c9df438b615` |
| `ext1/manifest-R2.csv` | `3f887e70665476a4f0c57c2d93e3944a712c4406edb8de4ce34f19aa05f55e19` |
| `kvt_analyze_ext.py` | `d658a420cb26ef243069734da04276089d22c9b5ae54d841fafadee4ba9cadfe` |
| `kvt_analyze.py` (v1; wilson/pct buradan) | `46ae83ee137e376f97432e4156a50bbbc105994c4867034f3b878649edbc3d1c` |
| `kvt_exec.py` | `6dc04c69d90d54535dd8a8220e729118636fe33277d0e5456867cec732b95232` |
| `kvt_manifest_ext.py` | `7713fe64f91051aa18302885f898e8390956431575cbabb6352db7b916e3570d` |
| `kvt_netem_check.py` | `5bacdb4382dc125cc3e087620e15e3cf07daff3465f0b226b9765dc3c3544f5c` |
| `kvt_row.py` | `2352d251e00646d70e998cfc20938102eaba04b952021a8689105afabd120ca8` |
| `target/release/bench` (kama-v1-full + quinn kolları) | `7a4e2d5c5e5966b3f4ef862919b7b0a7f2ca6e48d3aeb9b1b9a6dcda32f492d3` |
| `baselines/target/release/tcp_adapter` | `46d07cbb0c7029ae01ceb6727c16980be0396c0a3742eac0300c2a13ec71db38` |

Duman (`smoke_result.csv`): üç blok × üç kol koştu; bant→burst geçişinde netem doğrulaması 0 hata. Kalibrasyon v1'den geçerli (IID hücreleri aynı); gemodel ve rate hücreleri
ayrıca kalibre EDİLMEDİ — satır başı netem sayaç farkları (gerçekleşen drop/sent) her satırda kayıtlıdır (Tehdit olarak raporlanır).
