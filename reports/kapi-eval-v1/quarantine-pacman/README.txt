KARANTINA (2026-08-23 18:40) — 636-paketlik pacman güncellemesi (16:51 + 17:30 + ~18:3x pencereleri)
sonuç-üreten koşularla çakıştı (CPU/IO çekişmesi = timing-kirlenmesi riski; kanal-toleransları
İHLAL EDİLMEDİ ama temiz-koşullar-altında-tekrar ucuz ve hakem-temiz).
- eval/: SEC-4 u=0.9 duyarlılık hücresinin son 14 koşusu (rep6 kısmi + rep7-9) — primary DEĞİL.
  Güncelleme bitince aynı manifest+tohumlarla yeniden koşulacak; orijinaller burada saklı (silinmez).
- expl/: atılan h1 hücresi (F4-trim, manifest-dışı; 2 INVALID = trim-durdurma artefaktı) +
  güncelleme sırasında koşan h2_cell başlangıcı (yeniden koşulacak).
Primary hükümler (1b39a4f0) bu pencereden ETKİLENMEDİ (primary hücreler 22 Ağustos'ta bitmişti).
