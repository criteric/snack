# Snack 0.6.2 — Bölgesel sağ tık menüleri

**Amaç:** Sağ tık yaptığın yere uygun komutlar gösterilir; menülerdeki komutlar gerçek fonksiyonlara bağlıdır.

| Sağ tıklanan alan | Komut örnekleri |
|---|---|
| Kod editörü | Kes, Kopyala, Yapıştır, Tümünü Seç, Satırı Seç, Satırı Çoğalt, Satırı Sil, Yoruma Al/Çıkar, Geri Al/Yinele, Bul/Değiştir, Kaydet, Dosya Yolunu Kopyala |
| Sol dosya ağacında **dosya** | Aç, burada yeni dosya/klasör, adı/yolu kopyala, yeniden adlandır, sil, dosya yöneticisinde göster |
| Sol dosya ağacında **klasör** | Aç, genişlet/daralt, içine yeni dosya/klasör, yol/isim kopyala, yeniden adlandır, sil (yalnızca boşsa) |
| Dosya ağacının boş kısmı | Yeni dosya, yeni klasör, başka klasör aç, yenile, Git'e geç |
| Dosya sekmesi | Kaydet, Farklı Kaydet, dosya yolunu kopyala, kapat, diğerlerini kapat, sağdakileri kapat |
| Git paneli | Değişen dosyayı aç, yolunu kopyala, dış dosya yöneticisinde göster, Git listesini yenile |
| Alt terminal paneli | Çıktı/komut kopyala, komut satırına yapıştır, çıktıyı temizle, komut çalıştır, odağı değiştir, gizle |
| Hata paneli | Hataları kopyala/temizle, LSP'den öneri iste, paneli gizle |
| Durum çubuğu | Dosya ağacı, Git, terminal, problemler, ayarlar, dosya yolunu kopyala |

## Metin seçimi ve pano

- Sol fareyi basılı sürükle veya `Shift + Ok` kullanarak metin seç.
- Sağ tıkladığın konum seçili metindeyse seçim kaybolmaz.
- `Ctrl+C` / `Ctrl+X` / `Ctrl+V` / `Ctrl+A` kısayolları var.
- Seçim yoksa **Kopyala / Kes**, bulunduğun satır üzerinde çalışır.
- Zorin'de pano desteği için `sudo apt install xclip` (Wayland: `wl-clipboard`) önerilir. Program yine de kendi iç panosunu kullanabilir.
- Uzun sağ tık menüsünde fare tekerleğiyle kaydırabilirsin.
- Silme, **DELETE** yazarak onay gerektirir; değişmiş sekmeler sessizce kapatılmaz.

## Derleme

```bash
cd snack-0.6.2
cargo test
cargo build --release
./target/release/snack
```

`cargo test` tüm Rust testlerini; `./build-smoke.sh` ek derleme ve CLI kontrollerini çalıştırır. Bu ortamda Rust derleyicisi mevcut olmadığından **paket önceden derlenmiş olarak doğrulanmadı**; karşılaştığın ilk `error[E...]` çıktısını gönderirsen doğrudan düzeltebiliriz.

`snackctl`, mevcut `.yummy` paketler ve Marketplace komutları bu pakette tutuldu. GTK dosya seçici için `zenity` gereklidir.
