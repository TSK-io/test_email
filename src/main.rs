use std::env;
use std::fs;
use std::process;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDateTime, TimeZone};
use serde::Deserialize;
use serde_json::json;

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

/// 运行期配置,全部来自环境变量
struct Config {
    api_key: String,
    file: String,
    to: String,
    from: String,
    lead_min: i64,
    interval_sec: u64,
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// 展开开头的 "~/" 为 $HOME
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

fn load_config() -> Config {
    let home = env::var("HOME").unwrap_or_default();
    let default_file = format!("{home}/dotfiles/docs/data.yaml");
    Config {
        // 这里不强制,各子命令按需校验(list 不需要 key 也能用)
        api_key: env::var("RESEND_API_KEY").unwrap_or_default(),
        file: expand_tilde(&env_or("CAL_FILE", &default_file)),
        to: env_or("CAL_TO", "free514dom@proton.me"),
        from: env_or("CAL_FROM", "Calendar Bot <bot@sa514sa.top>"),
        lead_min: env_or("CAL_LEAD_MIN", "0").parse().unwrap_or(0),
        interval_sec: env_or("CAL_INTERVAL_SEC", "30").parse().unwrap_or(30),
    }
}

fn load_events(path: &str) -> Result<Vec<Event>, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("读取 {path} 失败: {e}"))?;
    let sched: Schedule = serde_yaml::from_str(&content).map_err(|e| format!("解析 YAML 失败: {e}"))?;
    Ok(sched.events)
}

/// 计算事件的触发时刻(事件时间减去提前量),按服务器本地时区解析
fn parse_trigger(ev: &Event, lead_min: i64) -> Option<DateTime<Local>> {
    let naive = NaiveDateTime::parse_from_str(ev.time.trim(), "%Y-%m-%d %H:%M").ok()?;
    let local = Local.from_local_datetime(&naive).single()?;
    Some(local - ChronoDuration::minutes(lead_min))
}

fn send_email(cfg: &Config, subject: &str, body: &str) -> Result<(), String> {
    if cfg.api_key.is_empty() {
        return Err("找不到 RESEND_API_KEY 环境变量".to_string());
    }
    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://api.resend.com/emails")
        .header("Authorization", format!("Bearer {}", cfg.api_key))
        .json(&json!({
            "from": cfg.from,
            "to": cfg.to,
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

fn reminder_body(ev: &Event, lead_min: i64) -> String {
    if lead_min <= 0 {
        format!("你的日程「{}」时间到了。\n事件时间:{}", ev.title, ev.time)
    } else {
        format!(
            "提醒:你的日程「{}」将于 {} 开始(提前 {} 分钟通知)。",
            ev.title, ev.time, lead_min
        )
    }
}

/// 守护进程:轮询循环,自己做调度,不依赖 cron/systemd
fn run_daemon(cfg: &Config) {
    if cfg.api_key.is_empty() {
        eprintln!("致命错误:找不到 RESEND_API_KEY 环境变量,无法发送邮件。");
        process::exit(1);
    }
    println!(
        "日历提醒守护进程已启动:\n  事件文件 = {}\n  收件人   = {}\n  发件人   = {}\n  提前量   = {} 分钟\n  轮询间隔 = {} 秒",
        cfg.file, cfg.to, cfg.from, cfg.lead_min, cfg.interval_sec
    );

    // 启动时刻;启动前已过的事件不补发
    let mut last = Local::now();
    let interval = Duration::from_secs(cfg.interval_sec.max(1));

    loop {
        thread::sleep(interval);
        let now = Local::now();

        // 每拍都重新读 YAML —— 改/加事件无需重启
        let events = match load_events(&cfg.file) {
            Ok(ev) => ev,
            Err(e) => {
                // 不推进 last,下一拍窗口自动覆盖这段时间
                eprintln!("[{}] 读取事件失败,稍后重试:{}", now.format("%Y-%m-%d %H:%M:%S"), e);
                continue;
            }
        };

        for ev in &events {
            let trigger = match parse_trigger(ev, cfg.lead_min) {
                Some(t) => t,
                None => {
                    eprintln!("跳过无法解析时间的事件:time=\"{}\" title=\"{}\"", ev.time, ev.title);
                    continue;
                }
            };
            // 左开右闭窗口 (last, now]:每个事件只触发一次
            if trigger > last && trigger <= now {
                let subject = format!("📅 日历提醒:{}", ev.title);
                let body = reminder_body(ev, cfg.lead_min);
                match send_email(cfg, &subject, &body) {
                    Ok(()) => println!("[{}] 已发送提醒:{}", now.format("%Y-%m-%d %H:%M:%S"), ev.title),
                    Err(e) => eprintln!(
                        "[{}] 发送失败({}):{}",
                        now.format("%Y-%m-%d %H:%M:%S"),
                        ev.title,
                        e
                    ),
                }
            }
        }

        last = now;
    }
}

fn cmd_list(cfg: &Config) {
    let events = match load_events(&cfg.file) {
        Ok(ev) => ev,
        Err(e) => {
            eprintln!("错误:{e}");
            process::exit(1);
        }
    };
    let now = Local::now();
    println!("事件文件:{}", cfg.file);
    println!("当前时间:{}", now.format("%Y-%m-%d %H:%M:%S %z"));
    println!("提前量:{} 分钟", cfg.lead_min);
    println!();
    if events.is_empty() {
        println!("(没有事件)");
        return;
    }
    for ev in &events {
        match parse_trigger(ev, cfg.lead_min) {
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
    println!("正在向 {} 发送测试邮件...", cfg.to);
    let subject = "📅 日历提醒 — 测试邮件";
    let body = "这是 calendar-notify 的测试邮件。如果你收到了,说明 Resend 密钥、发件域名和收件人都配置正确。";
    match send_email(cfg, subject, body) {
        Ok(()) => println!("✅ 测试邮件发送成功!"),
        Err(e) => {
            eprintln!("❌ 测试邮件发送失败:{e}");
            process::exit(1);
        }
    }
}

fn print_help() {
    println!(
        "calendar-notify — 日历提醒守护进程\n\n\
         用法:\n  \
         calendar-notify [run]   启动守护进程(默认)\n  \
         calendar-notify list    列出事件及触发时刻\n  \
         calendar-notify test    立即发送一封测试邮件\n\n\
         环境变量:\n  \
         RESEND_API_KEY    (必填) Resend API 密钥\n  \
         CAL_FILE          事件 YAML 路径(默认 ~/dotfiles/docs/data.yaml)\n  \
         CAL_TO            收件人(默认 free514dom@proton.me)\n  \
         CAL_FROM          发件人(默认 \"Calendar Bot <bot@sa514sa.top>\")\n  \
         CAL_LEAD_MIN      提前多少分钟提醒(默认 0)\n  \
         CAL_INTERVAL_SEC  轮询间隔秒数(默认 30)"
    );
}

fn main() {
    let cfg = load_config();
    let cmd = env::args().nth(1).unwrap_or_else(|| "run".to_string());
    match cmd.as_str() {
        "run" => run_daemon(&cfg),
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
