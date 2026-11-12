# Snack 0.6.1 — Resmi GitHub eklenti pazaryeri

Bu surumde resmi eklenti deposu: `criteric/snack-plugins`.

## Derleme (Zorin OS)

```bash
sudo apt update
sudo apt install -y curl git gh lua5.4 zenity
cargo test
cargo build --release
./target/release/snackctl --version
```

## Eklenti olustur ve yerelde test et

```bash
./target/release/snackctl validate examples/criteric.insert-timestamp-0.1.0.yummy
./target/release/snackctl install examples/criteric.insert-timestamp-0.1.0.yummy
./target/release/snack
```

Snack icinde Ctrl+P > `criteric.insert-timestamp` > ilk calistirma izni icin YES yaz. Eklenti imlecin oldugu yere tarih/saat ekler.

## GitHub'a yayinla

Ilk once `snack-plugins-registry` paketindeki dosyalari **https://github.com/criteric/snack-plugins** deposunun `main` dalina yukle (index.json root konumunda olmali). Repo herkese acik olmali ki kullanicilar arayip indirebilsin.

```bash
gh auth login
./target/release/snackctl publish examples/criteric.insert-timestamp-0.1.0.yummy --dry-run
./target/release/snackctl publish examples/criteric.insert-timestamp-0.1.0.yummy
```

Yayin basarili olursa `snackctl publish` once Release asset olusturur, sonra depodaki `index.json` dosyasini gunceller. Bu iki ayri GitHub islemi oldugu icin ikincisi basarisizsa birincisi geride kalabilir.

Daha sonra:

```bash
./target/release/snackctl search timestamp
./target/release/snackctl install criteric.insert-timestamp
./target/release/snackctl update
```

`snackctl search` resmi indeks disinda GitHub topluluk depolarini da ayri ve dogrulanmamis olarak listeler.

**Sinirlar:** Sadece Lua `.yummy` eklentisi Snack icinde calistirilabilir. WASM/native icin yurutme motoru henüz yoktur; bir Lua plugin kullanıcı haklarıyla calisir. SHA-256 dosya butunlugunu denetler ama guvenlik garantisi degildir. Bu pakette GitHub'a herhangi bir veri yuklenmedi; yayinlama komutunu senin `gh` oturumunla calistirman gerekir.
