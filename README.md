# caln

极简日历提醒守护进程:读 YAML 里的日程，到点用 [Resend](https://resend.com) 发邮件提醒。Release 提供 `.deb` 包，安装后得到 `caln` CLI 和 systemd 用户服务。

## 安装

从 GitHub Release 下载最新的 `caln_*_amd64.deb`，然后安装：

```bash
sudo apt install ./caln_*_amd64.deb
```

配置 Resend 密钥：

```bash
mkdir -p ~/.config/caln
chmod 700 ~/.config/caln
printf 'RESEND_API_KEY=%s\n' '你的密钥' > ~/.config/caln/env
chmod 600 ~/.config/caln/env
```

启用服务：

```bash
systemctl --user daemon-reload
systemctl --user enable --now caln
```

需要无人登录也自动运行时，再执行一次 `loginctl enable-linger "$USER"`。

卸载：

```bash
systemctl --user disable --now caln
sudo apt remove caln
```

## 配置事件

编辑事件文件（默认 `~/dotfiles/docs/data.yaml`，或用 `CAL_FILE` 指定）。守护进程每轮重读，改完即时生效、无需重启：

```yaml
events:
  - time: "2026-06-01 15:30"
    title: 团队周会
  - time: "2026-06-02 09:00"
    title: 牙医预约
```

## 命令

```bash
caln          # 启动守护进程(= caln run)
caln list     # 列出事件及触发时刻
caln test     # 立即发一封测试邮件
```

服务管理：

```bash
systemctl --user status caln
journalctl --user -u caln -f
```

## 环境变量

| 变量 | 说明 | 默认 |
|---|---|---|
| `RESEND_API_KEY` | Resend API 密钥（必填） | — |
| `CAL_FILE` | 事件 YAML 路径 | `~/dotfiles/docs/data.yaml` |
| `CAL_TO` | 收件人 | `free514dom@proton.me` |
| `CAL_FROM` | 发件人 | `Calendar Bot <bot@sa514sa.top>` |
| `CAL_LEAD_MIN` | 提前多少分钟提醒 | `0` |
| `CAL_INTERVAL_SEC` | 轮询间隔秒数 | `30` |

## 构建

由 GitHub Actions 编译（x86_64 musl 静态链接）并打包。推送 `v*` tag 即发布：

- `caln_<version>_amd64.deb`
- `SHA256SUMS`

需要本地打包时使用同一脚本：

```bash
TARGET=x86_64-unknown-linux-musl ./packaging/build-deb.sh
```
