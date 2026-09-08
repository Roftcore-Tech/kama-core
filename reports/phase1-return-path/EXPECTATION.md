# FAZ-1 — DÖNÜŞ-YOLU GÖZLEMİ · BEKLENTİ KAYDI (koşudan ÖNCE)

**Soru:** ürün runtime'ı, ileri-yön bozulmasını dönüş-yolu bozulmasından **runtime durumunda**
ayırt edebiliyor mu? (Faz-1 çıkış ölçütü. Performans ölçümü DEĞİL — ateşleme/ayrım kanıtı.)

**Çürütme koşulu:** R1'de `return_impairment` yükselirse ya da R2/R3/R5'te `forward_loss` sıfırdan
belirgin sapıyorsa ayrım YOKTUR ve Faz-1 kapanmaz.

**Bütçe:** 6 hücre × 1 koşu × ≤120 s. Karar: Faz-1 kapanır / kapanmaz.

**Hücreler (kapi3 iki-netns; fwd = veri yönü, rev = ACK yönü):**

| hücre | fwd | rev | öngörü |
|---|---|---|---|
| R0 sağlıklı simetrik | 100 ms, 1 mbit | 100 ms, 1 mbit | `forward_loss≈0` · `health=Healthy` · `bw` yüksek · `dir_ev=true` |
| R1 yalnız ileri bozulma | 100 ms, **loss 20 %**, 1 mbit | 100 ms, 1 mbit | **`forward_loss` 150–350‰** · `return_impairment` düşük · `health=Healthy` |
| R2 yalnız dönüş bozulma | 100 ms, 1 mbit | 100 ms, **loss 40 %**, 1 mbit | **`forward_loss≈0`** · `ack_interval` R0'dan büyük · `bw` R0'dan küçük |
| R3 asimetrik bant | 100 ms, 1 mbit | 100 ms, **64 kbit** | `forward_loss≈0` · ölçülen `bw` düşük · ayrım korunur |
| R4 asimetrik gecikme | **20 ms**, 1 mbit | **200 ms**, 1 mbit | `forward_loss≈0` · `ack_interval` yüksek · `health` Healthy/Degraded |
| R5 ölü geri bildirim | 100 ms, 1 mbit | 100 ms, **loss 95 %** | `forward_loss≈0` · `health=Degraded|Dead` · `continuity` düşük |

**Kritik invaryant (INV-RETURN-1):** hiçbir hücrede dönüş bozulması `forward_loss`'a
yazılmayacak. R1'in `forward_loss`'u ile R2/R3/R4/R5'inki arasında **açık bir ayrım** olmalı.

**Kapsam dışı (bilinçli):** ölçülen `return_bw`'nin EAI/confirm-ufkuna veya ACK decimation'a
BAĞLANMASI. Spec o gövdeyi (§3.3-E FULL) ADIM-6'ya erteler; Faz-1 yalnız ölçer ve gösterir.
