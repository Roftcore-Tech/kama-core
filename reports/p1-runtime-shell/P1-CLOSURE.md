# P1 — Transport Runtime Kabuğu (KAPANIŞ)

**Statü:** FAZ-1 + FAZ-2 TAMAM. Tasarım `docs/design/P1-RUNTIME-SHELL.md` (D1-D10); kod
`crates/client-core/{seam,wire,runtime}.rs` + kapılar. Güvenlik AYRI düzlem (SoC, `1b38936d`):
kabuk opak datagram taşır; **hiçbir güvenlik iddiası yapılmaz** (IdentitySeam stub).

## PRODUCT-RUNTIME-INTEGRATION-PENDING durum tablosu

| item | durum | kanıt |
|---|---|---|
| 1. Ürün crate/binary sahibi | ✅ (önceden) | client-core composition root |
| 2. UDP recv/send döngüsü | ✅ | `runtime.rs` tokio tek-task select!; bench-kanıtlı kadanslar |
| 3. Bağlantı yaşam-döngüsü | ✅ (v0) | Running→Draining(cap)→Closed; peer-tablosu cap + idle-emeklilik + slot-yeniden-kullanımı (test) |
| 4. Noise handshake | ⛔ B-hattı (güvenlik düzlemi; P1 kapsamı DEĞİL) | seam trait hazır |
| 5. Split yön-anahtarları | ⛔ B-hattı | `install_session_with_directional_keys` seam'i mevcut |
| 6. Sender ↔ ReceiverControlCore | **KISMİ — karar: B-hattıyla tamamlanır** (aşağıda) | dal-noktası `unknown_frames` sayacıyla görünür |
| 7. Outbound datagram drain | ✅ | kararlar + ACK'ler seam.seal→socket; hata=kayıp (INV-RATE-1) |
| 8. Süreç-düzeyi e2e | ✅ | loopback 7/7 (CI) + iki-netns netem kapısı GREEN: %5/20ms, %20/40ms, sürekli-yük 500@%10/40ms |
| 9. Shutdown/hata/kaynak-sınırı | ✅ | aşağıdaki doğrulama listesi |

### Item 6 kararı (kapsam dürüstlüğü)
Kontrol düzlemi (0x19 probe → 0x1A echo) kama-noise-wire SEALED formatında yaşar ve kütüphane-düzeyi
kablolaması ZATEN kanıtlı (kama-integration CLOSURE-1B: directional-key, 3-tekrar pin). UDP-düzeyi
kablolama, veri düzleminin de aynı sealed formata geçmesini gerektirir — o geçiş TAM OLARAK B-hattının
(Noise/Split, S3) işi. IdentitySeam v0-çerçevesi üstüne stub-anahtarlı bir kontrol dalı kurmak atılacak
bir kimera olurdu (bootstrap default-OFF: dal uykuda kalırdı). Karar: v0'da tanınmayan çerçeveler
`unknown_frames` sayacıyla GÖRÜNÜR (sessiz yutma yok); tam kablolama B-hattı S3'te aynı seam'den.

## Item 9 doğrulama listesi (hepsi testli/kapılı)

| doğrulama | kanıt |
|---|---|
| Orta-koşu peer ölümü: asılı kalma yok, drain cap'li, residue typed | `client_survives_server_death_mid_run` |
| Shutdown'da işlenmemiş offer sessiz kaybolmaz | `unaccepted_offers` sayacı (testin yakaladığı boşluk) |
| Teslim-kanalı taşması: ACK yolu etkilenmez, taşan sayılır, kanal bozulmaz | `delivery_overflow_is_counted_not_grown` |
| Idle-peer emekliliği + slot çevrimi | `idle_peer_is_retired_and_slot_reusable` |
| Typed sınırlar: QueueFull / UnitTooLarge / peer-cap fail-closed | 3 test (FAZ-1) |
| Kaynak-sınırı: tepe-RSS < 64 MiB, sürekli yükte büyüme yok | netem kapısı RSS-assert (ölçülen ~4.0 MB @ 500 birim) |

## Kapıların yakaladığı kusurlar (kapı-değeri kanıtı; hepsi merge-öncesi düzeltildi)
1. Store-miss retransmit'i göndermemek motora yapay kayıp sinyaliydi → başlık-only emisyon.
2. `Receiver` seq-dedup yapar; birim-dedup kabuğun işi (ilk netem koşusunda 26 duplicate-teslim
   app'e sızdı) → per-peer bounded ilk-teslim penceresi (8192; E2E envelope nihai otorite).
3. Erken çıkan server client'ın kuyruk-ACK yolunu öldürüyordu → örnek linger.
4. Shutdown'da kuyruktaki offer'ların sessiz kaybı → `unaccepted_offers`.

## Sınırlılıklar
- Wire-v0 bench-soyu çerçeve; ccr §35 finalizasyonu B-hattı entegrasyonunda (seam arkasında — D5).
- Teslim-dedup penceresi 8192 (pencere-dışı çok-geç duplicate teorik; E2E envelope §4 nihai dedup).
- `drain_cap` varsayılanı 3 s (bench deseni); ürün profili başına ayarlanabilir.
- Kabuk tek-peer-client / çok-peer-server modeli; hub-mesh çoklu-yol P2+ işi.
