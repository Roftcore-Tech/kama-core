# Yerel ham izler — netem taban koşuları (senaryo v1)

Yalnız ölçüm çıktısı; birleştirilmez. Ürün kodu: `yerel/poc-entegrasyon` @ `73715edbede79bd17b5cc11408947b65f6b7b1fd` (aday `1a4f9efc` + #251). #250 (kuyruk tahmini) YOK.

- Kip: iki `f0_flows` süreci (ayrı ağ ad alanları), cc-port, `KAMA_ACK_FORMAT=bitmap`, yön hızı profili aynalı (12 500 / 62), PTT'siz.
- Senaryo: `netem-v1/netem-senaryo-v1.md` (koordinatör, #175 7096381261): 12 500 → 20 s 6 250 → 60 s 12 500 → 85,0–85,5 s 6 250 → 100 s; dosya 1,2 MB; metin 2 s'de bir.
- Hat: ileri darboğaz `ifb0` (tbf + pfifo 64, IP boyu) → `mb0` netem (300 ms + %20, `seed` = tohum): **kayıp darboğazdan SONRA**. Dönüş `ma0` tek netem 62 B/s, 300 ms, kayıpsız. Hız basamağında aynı kuyruk korunur (araç kanıtı: #250 yorum 7096775628; `netem-v1/sonda/`).
- El sıkışma ve sağlama dosyaları netem DIŞINDA (Unix soketi / dosya).
- Kayıp deseni sıralı çekirdek RNG'si; fikstürün anahtarlı kaybıyla satır satır karşılaştırılmaz.

## Dizinler
- `netem-v1/1`, `2`, `3`: taban koşuları, tohum 1–3. `v1_taban_t<N>/aday/rep1/` altında: `alice.json(l)`, `bob.json(l)` (f0_flows kayıtları; `ts.s[]` canlı ayna örnekleri), `kuyruk.jsonl` (darboğaz kuyruğu örnekleyicisi, ≤ 100 ms), `basamak.jsonl` (hız basamağının uygulandığı an ve öncesi/sonrası sayaçlar), `tc-d-qdisc-show.txt`, `realized_pre/post.json`, `cap/` (tel yakalamaları: hat öncesi/sonrası), `kayit/` (düğüm başlangıç ve pilot özetleri), `cmd.txt`.
- `netem-v1/kesmesiz-1`: aynı senaryo, hız basamağı YOK, tohum 1 (kısa daralma ölçütünün karşılaştırma kolu).
- `netem-v1/arac`: koşu ve analiz betikleri, hücre dosyaları, scratch düzenek (`hdir/`: depo `kapi3_harness.sh` + iki kademeli hat, örnekleyici, basamak uygulayıcısı).
- `netem-v1/sonda`: araç kanıtı (hız, kuyruk 64, kayıp sırası, basamakta kuyruk sürekliliği).
- `analiz-*.txt/json`, `uclu.*`, `metin-tablo.txt`: Yerel ölçüm ajanının YARIM KALAN ön hesapları. **Doğrulanmadı; kaynak sayılmaz.** Ayrıştırma Bulut-A ve Bulut-B'de.

## Bu kayıtta OLMAYAN alanlar (hükmü bağlar)
- ACK başına LineGuard tavanı (`lg` satırı) yok: `ts.s[].pr` pacer kovasının hızıdır, hat tavanı değil. CC ve hızlanma hükmü bu kayıttan verilemez.
- ACK tür sayacı yok (kip tel kanıtı dolaylı).
- Bob'da ilk geçerli kanıtın oluştuğu an yok: "düşüş → ACK çıkışı" süresi kanıt bekleme / bütçe / dolu dönüşe yol verme olarak **ayrıştırılamaz**.
- Alındının Bob'dan çıkış anı yok.
Bu alanlar `yerel/f0-olcum-kancalari` dalında ekleniyor.

Makine yolları `<scratch>`, `<wt>`, `<home>` ile değiştirildi.
