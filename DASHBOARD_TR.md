# Snack 0.6.4 - Animated Dashboard

`snack` (argümansız) açıldığında animasyonlu Tokyo Night ana menü gösterilir.
`snack /path/to/file` veya `snack /path/to/project` doğrudan editörü açar.

- Yukarı/Aşağı veya j/k: menüde gezin
- Enter: seç
- N: Yeni dosya
- F: Dosya aç
- O: Klasör aç
- M veya F9: Extensions Marketplace
- S: Ayarlar
- Q veya Esc: Çık

Başlangıç ekranında programın kurulduğu dizin otomatik açılmaz. Argümansız
çalıştırmada varsayılan çalışma dizini HOME olarak alınır.

Bazzite üzerinde geliştirme: Distrobox içinde `cargo test` ve
`cargo build --release`. Host sistemine kurmak için distro paketleri ayrıca
hazırlanmalıdır; bu ZIP bir RPM/DEB/PKGBUILD içermez.

NOT: Kaynak kodun cargo test derlemesi bu ortamda çalıştırılamadı.
