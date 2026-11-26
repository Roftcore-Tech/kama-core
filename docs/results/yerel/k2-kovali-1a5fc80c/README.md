# K2 — düzeltilmiş baş `1a5fc80c`: kırmızı → yeşil kanıtı ve kovalı dar karşılaştırma

Benzetimdir; kabul koşusu değildir.

| Dosya | İçerik |
|---|---|
| `kirmizi-474daa44.txt` | `474daa44` (yalnız testler; karar kodu `ee168d3d` ile aynı): `cargo test --locked --release -p adaptive-transport --lib -- line_guard:: pair_probe_unmeasured_ack_tests --test-threads=1` → 48 geçti, **18 kırmızı**, 1 ignored |
| `yesil-1a5fc80c.txt` | `1a5fc80c` (düzeltme): aynı komut → 68 geçti, 0 kırmızı, 1 ignored (EK 6 şart testi: sahip kararı bekliyor) |
| `ham.txt` | `1a5fc80c`'de `bash scripts/ktp2/f0/k2/kos.sh`: gerçek `Sender` × ürün `Receiver` × kovalı darboğaz; iki vaka × deney açık / kapalı. İlk satır `K2BASLIK` (kaynak SHA, ikili sha256, rustc). Dosya sha256: `bc3278209f3ea77df9719e184d80bbc90d0497048a09b94d4ac0cea06bbf9c43` |

- `ham.txt` satırlarının sha256'sı (`satirlar_sha256`) düzeltme öncesi kayıtla (`../k2-kovali-f55ca1de/ham.txt`) AYNI: `36de189e…`. Yani bu iki vakada düzeltme hiçbir `K2ROW` / `K2OZET` satırını değiştirmedi; bu koşularda ölçüm taşımayan / cephesi ilerlemeyen ACK ve ikinci yoklama denemesi hiç oluşmuyor. İki kusurun kanıtı dar testlerdir (üst iki dosya).
- Bağımsız yeniden üretim: Bulut-B `f55ca1de`'yi kendi ortamında koştu, aynı `satirlar_sha256` (#175 7176978739).
- Tam paket (`cargo test -p adaptive-transport --no-fail-fast`) `1a5fc80c`'de: yalnız taban dalda da kırmızı olan dört test kırmızı (`start_budget_sim::a_queue_drop_hidden_in_the_first_window_is_not_closed`; `starved_line_sim::{all_lost_ptt_bundle_within_the_1s_bound, b_real_capacity_drop_is_detected_and_the_queue_drains, d2_queue_overflow_still_converges_and_stops_overflowing}`).
