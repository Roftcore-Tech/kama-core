# K2 kovalı dar karşılaştırma — düzeltme ÖNCESİ kayıt (`f55ca1de`)

Benzetimdir; kabul koşusu değildir. Gerçek `Sender` × ürün `Receiver` × kovalı darboğaz (jeton kovası 1 200 B, IP boyu sayılır, 64 paket kuyruk); iki vaka (gerçek toparlanma; hat 6 250 B/s'de kalır) × deney açık / kapalı × hat {kova = esas, fifo = teşhis} × kayıp {0, binde 200} × tohum 1–3 × yük {PTT'siz, tek iletim PTT}.

- Kaynak: `f55ca1de50352db8c58c5e83ad9c27b8bd4e3f15` (`line_guard.rs` bu başta `ee168d3d` ile aynı: ACK sayımı ve tek atış hataları DÜZELTİLMEMİŞ).
- Üretim: `bash scripts/ktp2/f0/k2/kos.sh <çıktı>` (temiz ağaç; iki koşu bayt-aynı).
- `ham.txt` ilk satırı `K2BASLIK`: kaynak SHA, test ikilisinin sha256'sı, rustc, satırların sha256'sı. Dosyanın sha256'sı: `61260b4e16232c76c25eca80d9903f873394bed8ec99b5c145d28ac56ce6301d`.
- Kayıt biçimi: #175 yorum 7175574778 ve eki 7175586319.
