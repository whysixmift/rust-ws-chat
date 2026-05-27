# Rust WebSocket Chat (Axum + Tokio)

## Struktur

- `src/main.rs`: backend WebSocket + broadcast.
- `static/index.html`: frontend chat realtime.
- `Cargo.toml`: dependencies.

## Menjalankan Lokal

1. Install Rust:
   - `curl https://sh.rustup.rs -sSf | sh`
2. Build dan run:
   - `cargo run`
3. Buka:
   - `http://127.0.0.1:8080`

## Protocol Pesan

Format JSON:

```json
{
  "type": "message",
  "sender": "Julian",
  "text": "Halo semua"
}
```

## Deployment di Server Nest

### 1) Build binary release

```bash
cargo build --release
```

Binary ada di:

`target/release/rust-ws-chat`

### 2) Buka port di server

Contoh jika app bind di `8080`:

```bash
sudo ufw allow 8080/tcp
```

Jika pakai reverse proxy (disarankan), yang dibuka publik cukup `80/443`.

### 3) Reverse proxy WebSocket

#### Opsi Nginx

```nginx
server {
    listen 80;
    server_name chat.example.com;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # WebSocket upgrade
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

Reload:

```bash
sudo nginx -t && sudo systemctl reload nginx
```

#### Opsi Caddy

```caddy
chat.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

Caddy otomatis handle HTTPS + WebSocket upgrade.

### 4) Jalankan sebagai systemd (24/7)

Buat file `/etc/systemd/system/rust-ws-chat.service`:

```ini
[Unit]
Description=Rust Axum WebSocket Chat
After=network.target

[Service]
User=www-data
Group=www-data
WorkingDirectory=/opt/rust-ws-chat
ExecStart=/opt/rust-ws-chat/target/release/rust-ws-chat
Restart=always
RestartSec=3
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

Aktifkan:

```bash
sudo systemctl daemon-reload
sudo systemctl enable rust-ws-chat
sudo systemctl start rust-ws-chat
sudo systemctl status rust-ws-chat
```

Logs:

```bash
journalctl -u rust-ws-chat -f
```

## Catatan Security

- Frontend memakai `textContent`, bukan `innerHTML`, untuk cegah XSS.
- Backend juga melakukan sanitasi karakter HTML dasar.
- Batasi panjang `sender` dan `text` untuk mengurangi abuse.
