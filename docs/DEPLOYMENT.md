# 联网日麻部署

## 本地构建

```bash
cargo build --release -p riichi-server
cd tauri-app
npm ci
npm run build
npm run tauri build
```

`npm run tauri build` 会根据 [`src-tauri/tauri.conf.json`](../tauri-app/src-tauri/tauri.conf.json)
生成当前平台的安装包。Windows 和 Linux 构建应在对应平台或 CI runner 上执行；Linux
桌面构建需要 Tauri 的 GTK/WebKitGTK 开发依赖，例如 `pkg-config`、GTK、WebKitGTK、
Pango 和 Cairo。当前仓库已验证前端生产构建；在缺少这些系统库的环境中，原生构建会
在 `glib-sys`/`gdk-sys` 链接阶段失败。

## GitHub Actions Windows 构建

仓库中的 [`.github/workflows/windows-client.yml`](../.github/workflows/windows-client.yml)
会在 `main`/`master` 推送或手动触发时，在 `windows-latest` runner 上构建 Tauri Windows
安装包，并将 `.exe` 和 `.msi` 上传为 Actions artifact。

手动构建步骤：

1. 将本分支推送到 GitHub。
2. 打开仓库的 **Actions → Build Windows client**。
3. 点击 **Run workflow**。
4. 等 workflow 成功后，在运行详情页的 **Artifacts** 下载 `riichi-mahjong-windows-*`。

## Linux 服务器

将 release 二进制复制到 `/opt/riichi-mahjong/bin/riichi-server`，创建运行用户和目录：

```bash
sudo useradd --system --home /var/lib/riichi-mahjong --shell /usr/sbin/nologin riichi
sudo install -d -o riichi -g riichi /opt/riichi-mahjong/bin /var/lib/riichi-mahjong /etc/riichi-mahjong
sudo install -m 0755 target/release/riichi-server /opt/riichi-mahjong/bin/riichi-server
sudo install -m 0644 deploy/server.env.example /etc/riichi-mahjong/server.env
sudo install -m 0644 deploy/riichi-server.service /etc/systemd/system/riichi-server.service
sudo systemctl daemon-reload
sudo systemctl enable --now riichi-server
curl http://127.0.0.1:3000/health
```

服务默认只监听 `127.0.0.1:3000`，由 Nginx 对外提供 HTTPS 和 WSS。若没有反向代理，
可以在 `server.env` 中将 `RIICHI_SERVER_ADDR` 改为公开监听地址，但生产环境必须使用
TLS 终止层保护房间 token 和 WebSocket 连接。

## HTTPS/WSS

将 [`deploy/nginx-riichi.conf.example`](../deploy/nginx-riichi.conf.example) 复制到
Nginx 配置目录，替换域名和证书路径，再执行：

```bash
sudo nginx -t
sudo systemctl reload nginx
```

客户端服务器地址填写 `https://mahjong.example.com`；客户端会自动将 WebSocket 协议
转换为 `wss://`。证书可使用 Certbot/Let's Encrypt 获取和续期。

## 日志与排错

服务输出进入 systemd journal：

```bash
journalctl -u riichi-server -f
systemctl status riichi-server
```

当前版本使用内存房间；重启服务器会结束所有房间和对局，这是 MVP 的明确限制。
