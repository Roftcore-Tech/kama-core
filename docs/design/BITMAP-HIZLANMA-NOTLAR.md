# Bitmap hızlanma — Bulut-B çalışma notu (devralınabilir)

Taban `125b9afa`. İz `yerel/ham-izler-netem-v1` @ `844fb172`, `netem-v1/kancali-1/1/v1_taban_t1/aday/rep1`.

## Bulgu 1 (ölçüldü): toparlanmada çift alt sınırı (yol a) hiç değer üretemez
- 60,5–84,5 s, `cap/MID_ifb0.jsonl` out: art arda (aralık < 1,5 × ser) 18 çiftin tamamında
  aralık / (ip_len / 12 500) = 0,881–0,897 (medyan 0,889). `B_vb0 in`: 14 çiftin 12'si < 0,9.
- Alıcı kuralı `receiver.rs` `PairRate::squeezed`: `g < 0,9 × (b + 28) / beyan_hat` → sıkışmış,
  ardındaki çift de atılır. Beyan hat 12 500 = gerçek hat → gerçek çiftlerin hepsi atılıyor,
  `pair_rate` = NONE, `on_pair_rate` tavanı açamıyor; geriye en küçük adımlı yoklama kalıyor
  (6 223 → 6 608 → 7 427, sonra `probe_failed`).
- Fiziksel neden: tbf `burst 1200b`; hat göndericiden hızlıyken kova boşta dolar, birinci datagram
  kovadaki fazlayı bırakır, ikincisi tam serileşmeden (≈ %88) önce çıkar. Fikstür FIFO'su tam
  serileştiği için 6/6 geçiyordu.
- Hesap betiği: aşağıdaki python (Bulut-B oturumu), girdi `cap/MID_ifb0.jsonl`, `cap/B_vb0.jsonl`.

## Sıradaki adım
- Düzeltme adayı: sıkışma sınamasının payını kova fazlasını kapsayacak şekilde ayarlamak ya da
  birinci datagramın boyuna göre kova payını düşmek; karşı örnekler: dürüst 6 250 hat, boşalan
  aşağı akış kuyruğu, 5 ms toplu teslim (209 kB/s) hâlâ reddedilmeli.
- Sıra 15 `rise` alarmı: alanın değeri kayıtta yok.
