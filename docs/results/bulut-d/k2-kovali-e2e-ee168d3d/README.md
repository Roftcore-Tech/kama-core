# K2′ bağımsız düzenek — ham kayıt (taban `ee168d3d`)

Kart #175 7175575398. Düzenek: `crates/adaptive-transport/tests/k2_kovali_e2e.rs` (gerçek `Sender`,
ürün `Receiver`, tbf kovası 1 200 B + 64 paket FIFO + 300 ms; teşhis kolu saf FIFO). Kayıt biçimi
#175 7175574778 + EK 7175586319.

Komut:

```
cargo test --release -p adaptive-transport --test k2_kovali_e2e -- --ignored --nocapture --test-threads=1
```

`ham.txt`: ilk satır `K2BASLIK` (#175 7175939123: kaynak sha, temiz ağaç, test, komut, test ikilisinin
sha256'sı, rustc, iki koşunun bayt-aynılığı, satırların sha256'sı), ardından `K2ROW` / `K2OZET` satırları (2 kol × 2 hat × 2 kayıp × 3 tohum × 2 deney = 48 hücre).
Benzetim; kabul sayılmaz. Kayıp 0 hücrelerinde tohum kaybı seçmediği için üç tohum aynıdır.
`kapi_reddi` bu başta kancasız (`null`); `etkin_pencere` = `Sender::diag_cc` penceresi (geri
bildirimi aç uçuş tabanı dahil değil).

Üretim: `bash docs/results/bulut-d/k2-kovali-e2e-ee168d3d/kos.sh <çıktı>` (temiz ağaçta derler, test
ikilisini iki kez koşar, satırları karşılaştırır, başlığı yazar). Başlıktaki `sha` ikilinin derlendiği
commit'tir; `ham.txt`'nin kendisi bir sonraki commit'te eklenir.
