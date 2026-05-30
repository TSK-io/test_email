# caln

极简日历提醒守护进程:读 YAML 里的日程，到点用 [Resend](https://resend.com) 发邮件提醒。单文件静态二进制，无依赖，systemd 托管。

## 安装

```bash
curl -fsSL https://raw.githubusercontent.com/TSK-io/calendar-cli/main/install.sh | bash
```

脚本会下载最新 Release 二进制到 `/usr/local/bin/caln`，自动从环境变量或 shell 配置里抓取 `RESEND_API_KEY` 写入 `/etc/caln/env`，并生成开机自启的 systemd 服务。若环境里没有密钥：

```bash
RESEND_API_KEY=你的密钥 bash -c "$(curl -fsSL https://raw.githubusercontent.com/TSK-io/calendar-cli/main/install.sh)"
```

卸载：

```bash
curl -fsSL https://raw.githubusercontent.com/TSK-io/calendar-cli/main/install.sh | bash -s -- uninstall
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

服务管理：`systemctl status caln`，日志：`journalctl -u caln -f`。

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

由 GitHub Actions 编译（x86_64 musl 静态链接）。推送 `v*` tag 即发布 `caln-linux` 到 Release。
