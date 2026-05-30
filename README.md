# caln

个人用日历提醒守护进程。它只做一件事:从固定的 YAML 文件读取事件,到点后通过 Resend 给固定邮箱发提醒。

固定值:

- 收件人:`free514dom@proton.me`
- 发件人:`Calendar Bot <bot@sa514sa.top>`
- 事件文件:`$HOME/dotfiles/docs/data.yaml`
- 密钥文件:`$HOME/.config/caln/env`
- 时区:`Asia/Shanghai (UTC+08:00)`
- 轮询间隔:`30` 秒

## 安装

从 GitHub Release 下载最新 `.deb`:

```bash
sudo apt install ./caln_*_amd64.deb
```

安装脚本会自动创建:

- `$HOME/.config/caln/env`
- `$HOME/dotfiles/docs/data.yaml`
- `caln@$USER.service`

然后填入 Resend API key:

```bash
nano ~/.config/caln/env
sudo systemctl restart caln@$USER.service
```

`env` 内容:

```env
RESEND_API_KEY=你的_resend_key
```

## 使用

```bash
caln init
caln list
caln test
caln run
```

事件格式:

```yaml
events:
  - time: "2026-06-01 15:30"
    title: "交房租"
```

时间固定按上海时间 `Asia/Shanghai (UTC+08:00)` 解释。已经过去的事件不会补发。

## 发布

本项目不要求本地打包。发布只需要改 `Cargo.toml` 版本,然后推送匹配 tag:

```bash
git tag v0.1.6
git push origin main v0.1.6
```

GitHub Actions 会编译 Linux musl 二进制、生成 `.deb`、计算 SHA256,并把文件上传到对应 Release。
