use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{self, Command};
use std::thread;
use std::time::Duration as StdDuration;

use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::json;

const DATA_FILE: &str = "dotfiles/docs/data.yaml";
const TO: &str = "free514dom@proton.me";
const FROM: &str = "Calendar Bot <bot@sa514sa.top>";
const LEAD_MIN: i64 = 0;
const INTERVAL_SEC: u64 = 30;
const ENV_FILE: &str = ".config/caln/env";
const SENT_FILE: &str = ".config/caln/sent.log";
const CATCH_UP_MIN: i64 = 120;
const SHANGHAI_OFFSET_SEC: i32 = 8 * 60 * 60;
const SHANGHAI_TZ_LABEL: &str = "Asia/Shanghai (UTC+08:00)";

/// data.yaml 的结构:顶层一个 events 列表
#[derive(Debug, Deserialize)]
struct Schedule {
    #[serde(default)]
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct Event {
    /// 形如 "2026-06-01 15:30"
    time: String,
    title: String,
}

/// 运行期配置:从环境变量或 $HOME/.config/caln/env 读取密钥
struct Config {
    api_key: String,
    file: String,
    env_file: String,
    sent_file: String,
}

fn load_config() -> Config {
    let home = env::var("HOME").unwrap_or_else(|_| {
        eprintln!("致命错误:找不到 HOME 环境变量。");
        process::exit(1);
    });
    let env_file = format!("{home}/{ENV_FILE}");
    let api_key = env::var("RESEND_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_api_key_file(&env_file))
        .unwrap_or_default();

    Config {
        api_key,
        file: format!("{home}/{DATA_FILE}"),
        env_file,
        sent_file: format!("{home}/{SENT_FILE}"),
    }
}

fn read_api_key_file(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "RESEND_API_KEY" {
            continue;
        }

        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .or_else(|| {
                value
                    .strip_prefix('\'')
                    .and_then(|value| value.strip_suffix('\''))
            })
            .unwrap_or(value);

        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn ensure_env_file(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    if !path.exists() {
        fs::write(path, "RESEND_API_KEY=\n").map_err(|e| format!("创建密钥文件失败: {e}"))?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("设置密钥文件权限失败: {e}"))?;
    Ok(())
}

fn ensure_event_file(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建事件目录失败: {e}"))?;
    }
    if !path.exists() {
        fs::write(path, "events: []\n").map_err(|e| format!("创建事件文件失败: {e}"))?;
    }
    Ok(())
}

fn ensure_sent_file(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建状态目录失败: {e}"))?;
    }
    if !path.exists() {
        fs::write(path, "").map_err(|e| format!("创建发送记录失败: {e}"))?;
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("设置发送记录权限失败: {e}"))?;
    Ok(())
}

fn load_events(path: &str) -> Result<Vec<Event>, String> {
    ensure_event_file(path)?;
    let content = fs::read_to_string(path).map_err(|e| format!("读取 {path} 失败: {e}"))?;
    let sched: Schedule =
        serde_yaml::from_str(&content).map_err(|e| format!("解析 YAML 失败: {e}"))?;
    Ok(sched.events)
}

fn event_key(ev: &Event) -> String {
    format!("{}\t{}", ev.time.trim(), ev.title.trim())
}

fn load_sent(path: &str) -> Result<HashSet<String>, String> {
    ensure_sent_file(path)?;
    let content = fs::read_to_string(path).map_err(|e| format!("读取发送记录失败: {e}"))?;
    Ok(content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn mark_sent(path: &str, key: &str) -> Result<(), String> {
    ensure_sent_file(path)?;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(path)
        .map_err(|e| format!("打开发送记录失败: {e}"))?;
    writeln!(file, "{key}").map_err(|e| format!("写入发送记录失败: {e}"))
}

fn shanghai_tz() -> FixedOffset {
    FixedOffset::east_opt(SHANGHAI_OFFSET_SEC).expect("valid Shanghai UTC+8 offset")
}

fn now_shanghai() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&shanghai_tz())
}

/// 计算事件的触发时刻,固定按上海时间解析
fn parse_trigger(ev: &Event) -> Option<DateTime<FixedOffset>> {
    let naive = NaiveDateTime::parse_from_str(ev.time.trim(), "%Y-%m-%d %H:%M").ok()?;
    shanghai_tz().from_local_datetime(&naive).single()
}

fn send_email(cfg: &Config, subject: &str, body: &str) -> Result<(), String> {
    if cfg.api_key.is_empty() {
        return Err(format!("找不到 RESEND_API_KEY,请写入 {}", cfg.env_file));
    }
    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://api.resend.com/emails")
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .json(&json!({
            "from": FROM,
            "to": TO,
            "subject": subject,
            "text": body,
        }))
        .send()
        .map_err(|e| format!("HTTP 请求失败: {e}"))?;

    let status = res.status();
    if status.is_success() {
        Ok(())
    } else {
        let text = res.text().unwrap_or_default();
        Err(format!("Resend 返回错误 {status}: {text}"))
    }
}

fn reminder_body(ev: &Event) -> String {
    format!(
        "你的日程「{}」时间到了。\n事件时间(上海时间):{}",
        ev.title, ev.time
    )
}

fn current_user_name() -> Option<String> {
    for key in ["USER", "LOGNAME"] {
        if let Ok(value) = env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    let output = Command::new("id").arg("-un").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let user = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if user.is_empty() { None } else { Some(user) }
}

fn service_unit_name() -> Option<String> {
    current_user_name().map(|user| format!("caln@{user}.service"))
}

fn systemd_service_state() -> Option<(String, String)> {
    let unit = service_unit_name()?;
    let output = Command::new("systemctl")
        .arg("is-active")
        .arg(&unit)
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        None
    } else {
        Some((unit, stdout))
    }
}

fn restart_systemd_service() -> Result<String, String> {
    let unit = service_unit_name().ok_or_else(|| "找不到当前用户名".to_string())?;
    let output = Command::new("systemctl")
        .arg("restart")
        .arg(&unit)
        .output()
        .map_err(|e| format!("无法运行 systemctl: {e}"))?;

    if output.status.success() {
        Ok(unit)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("systemctl restart {unit} 失败"))
        } else {
            Err(stderr)
        }
    }
}

/// 守护进程:轮询循环,自己做调度,不依赖 cron/systemd
fn run_daemon(cfg: &Config) {
    if cfg.api_key.is_empty() {
        eprintln!(
            "找不到 RESEND_API_KEY,请运行 `caln init` 后编辑 {}。",
            cfg.env_file
        );
        process::exit(1);
    }
    println!(
        "日历提醒守护进程已启动:\n  事件文件 = {}\n  发送记录 = {}\n  收件人   = {}\n  发件人   = {}\n  时区     = {}\n  提前量   = {} 分钟\n  轮询间隔 = {} 秒\n  漏发补发 = 最近 {} 分钟",
        cfg.file, cfg.sent_file, TO, FROM, SHANGHAI_TZ_LABEL, LEAD_MIN, INTERVAL_SEC, CATCH_UP_MIN
    );

    let mut sent = match load_sent(&cfg.sent_file) {
        Ok(sent) => sent,
        Err(e) => {
            eprintln!("读取发送记录失败,本次运行仍会继续:{e}");
            HashSet::new()
        }
    };
    let interval = StdDuration::from_secs(INTERVAL_SEC);

    loop {
        let now = now_shanghai();
        let catch_up_after = now - ChronoDuration::minutes(CATCH_UP_MIN);

        // 每拍都重新读 YAML —— 改/加事件无需重启
        let events = match load_events(&cfg.file) {
            Ok(ev) => ev,
            Err(e) => {
                eprintln!(
                    "[{}] 读取事件失败,稍后重试:{}",
                    now.format("%Y-%m-%d %H:%M:%S"),
                    e
                );
                thread::sleep(interval);
                continue;
            }
        };

        for ev in &events {
            let trigger = match parse_trigger(ev) {
                Some(t) => t,
                None => {
                    eprintln!(
                        "跳过无法解析时间的事件:time=\"{}\" title=\"{}\"",
                        ev.time, ev.title
                    );
                    continue;
                }
            };

            let key = event_key(ev);
            if sent.contains(&key) || trigger > now || trigger < catch_up_after {
                continue;
            }

            let subject = format!("📅 日历提醒:{}", ev.title);
            let body = reminder_body(ev);
            match send_email(cfg, &subject, &body) {
                Ok(()) => {
                    println!(
                        "[{}] 已发送提醒:{}",
                        now.format("%Y-%m-%d %H:%M:%S"),
                        ev.title
                    );
                    if let Err(e) = mark_sent(&cfg.sent_file, &key) {
                        eprintln!(
                            "[{}] 已发送但记录状态失败({}):{}",
                            now.format("%Y-%m-%d %H:%M:%S"),
                            ev.title,
                            e
                        );
                    }
                    sent.insert(key);
                }
                Err(e) => eprintln!(
                    "[{}] 发送失败({}):{}",
                    now.format("%Y-%m-%d %H:%M:%S"),
                    ev.title,
                    e
                ),
            }
        }

        thread::sleep(interval);
    }
}

fn cmd_init(cfg: &Config) {
    if let Err(e) = ensure_env_file(&cfg.env_file)
        .and_then(|_| ensure_event_file(&cfg.file))
        .and_then(|_| ensure_sent_file(&cfg.sent_file))
    {
        eprintln!("初始化失败:{e}");
        process::exit(1);
    }

    println!("已初始化 caln:");
    println!("  密钥文件:{}", cfg.env_file);
    println!("  事件文件:{}", cfg.file);
    println!("  发送记录:{}", cfg.sent_file);
    println!("  收件人:{}", TO);
    println!("  时区:{}", SHANGHAI_TZ_LABEL);
    println!();
    println!("下一步:");
    println!("  1. 在密钥文件里填入 RESEND_API_KEY");
    println!("  2. 运行 `caln test` 发送测试邮件");
    println!("  3. 运行 `caln run` 启动提醒");
}

fn cmd_list(cfg: &Config) {
    let events = match load_events(&cfg.file) {
        Ok(ev) => ev,
        Err(e) => {
            eprintln!("错误:{e}");
            process::exit(1);
        }
    };
    let now = now_shanghai();
    println!("事件文件:{}", cfg.file);
    println!("发送记录:{}", cfg.sent_file);
    println!("当前上海时间:{}", now.format("%Y-%m-%d %H:%M:%S %z"));
    println!("提前量:{} 分钟", LEAD_MIN);
    println!("漏发补发:{} 分钟", CATCH_UP_MIN);
    if let Some((unit, state)) = systemd_service_state() {
        println!("守护服务:{} {}", unit, state);
    }
    println!();
    if events.is_empty() {
        println!("(没有事件)");
        return;
    }
    for ev in &events {
        match parse_trigger(ev) {
            Some(trigger) => {
                let status = if trigger > now { "未来" } else { "已过" };
                println!(
                    "[{}] {} | 触发于 {} | {}",
                    status,
                    ev.time,
                    trigger.format("%Y-%m-%d %H:%M"),
                    ev.title
                );
            }
            None => println!("[??] {} | (时间解析失败) | {}", ev.time, ev.title),
        }
    }
}

fn cmd_test(cfg: &Config) {
    println!("正在向 {} 发送测试邮件...", TO);
    let subject = "📅 日历提醒 — 测试邮件";
    let body = "这是 caln 的测试邮件。如果你收到了,说明 Resend 密钥、发件域名和收件人都配置正确。";
    match send_email(cfg, subject, body) {
        Ok(()) => {
            println!("✅ 测试邮件发送成功!");
            match restart_systemd_service() {
                Ok(unit) => println!("已重启守护服务:{unit}"),
                Err(e) => println!("提示:测试邮件成功,但自动重启守护服务失败:{e}"),
            }
        }
        Err(e) => {
            eprintln!("❌ 测试邮件发送失败:{e}");
            process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "caln — 日历提醒守护进程\n\n\
         用法:\n  \
         caln [run]   启动守护进程(默认)\n  \
         caln init    创建密钥文件和事件文件\n  \
         caln list    列出事件及触发时刻\n  \
         caln test    立即发送一封测试邮件\n\n\
         密钥:\n  \
         RESEND_API_KEY    环境变量,或 $HOME/.config/caln/env\n\n\
         固定值:\n  \
         事件 YAML 路径  $HOME/dotfiles/docs/data.yaml\n  \
         发送记录路径    $HOME/.config/caln/sent.log\n  \
         收件人          free514dom@proton.me\n  \
         发件人          Calendar Bot <bot@sa514sa.top>\n  \
         时区            Asia/Shanghai (UTC+08:00)\n  \
         提前量          0 分钟\n  \
         轮询间隔        30 秒\n  \
         漏发补发        最近 120 分钟"
    );
}

fn main() {
    let cfg = load_config();
    let cmd = env::args().nth(1).unwrap_or_else(|| "run".to_string());
    match cmd.as_str() {
        "run" => run_daemon(&cfg),
        "init" => cmd_init(&cfg),
        "list" => cmd_list(&cfg),
        "test" => cmd_test(&cfg),
        "-h" | "--help" | "help" => print_help(),
        other => {
            eprintln!("未知命令:{other}\n");
            print_help();
            process::exit(2);
        }
    }
}
