# caln

极简日历提醒守护进程:读 `$HOME/dotfiles/docs/data.yaml`，到点用 [Resend](https://resend.com) 发邮件到 `free514dom@proton.me`。Release 提供 `.deb` 包，安装后得到 `caln` CLI 和开机自启的 systemd 服务。

## 安装

先配置 Resend 密钥：

```bash
mkdir -p ~/.config/caln
chmod 700 ~/.config/caln
printf 'RESEND_API_KEY=%s\n' '你的密钥' > ~/.config/caln/env
chmod 600 ~/.config/caln/env
```

从 GitHub Release 下载最新的 `caln_*_amd64.deb`，然后安装：

```bash
sudo apt install ./caln_*_amd64.deb
```

安装脚本会自动启用并启动 `caln@$USER.service`。如果你在 root shell 里安装，先退出 root，用自己的用户执行 `sudo apt install`。

卸载：

```bash
sudo apt remove caln
```

## 配置事件

编辑事件文件 `$HOME/dotfiles/docs/data.yaml`。守护进程每轮重读，改完即时生效、无需重启：

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
sudo systemctl status "caln@$USER"
sudo journalctl -u "caln@$USER" -f
```

## 固定配置(仅个人工具直接固定值)

| 项 | 值 |
|---|---|
| 环境变量 | 只读取 `RESEND_API_KEY` |
| 事件文件 | `$HOME/dotfiles/docs/data.yaml` |
| 收件人 | `free514dom@proton.me` |
| 发件人 | `Calendar Bot <bot@sa514sa.top>` |
| 提前量 | `0` 分钟 |
| 轮询间隔 | `30` 秒 |

## 发布

不要在本地或服务器上编译。推送 `v*` tag 后，GitHub Actions 会编译 x86_64 musl 静态二进制、打包并发布：

- `caln_<version>_amd64.deb`
- `SHA256SUMS`

版本号必须和 `Cargo.toml` 一致，例如：

```bash
git tag v0.1.2
git push origin v0.1.2
```
