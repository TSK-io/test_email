use std::env;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 从环境变量获取 Resend 的 API Key
    let api_key = env::var("RESEND_API_KEY")
        .expect("致命错误：找不到 RESEND_API_KEY 环境变量！");


    // 2. 构建 HTTP 客户端
    let client = reqwest::Client::new();

    println!("正在通过 443 端口 API 发送邮件...");

    // 3. 构造请求并发送
    let res = client.post("https://api.resend.com/emails")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&json!({
            // 发件人：一旦你在 Resend 验证了域名，就可以随便编前缀了！
            "from": "Rust 机器人 <bot@sa514sa.top>", 
            // 收件人：你的 ProtonMail
            "to": "My Proton <free514dom@proton.me>",
            "subject": "来自 Rust API 的测试邮件！",
            "text": "成功啦！这封邮件走的是 HTTPS 的 443 端口，DigitalOcean 根本拦不住我们。"
        }))
        .send()
        .await?;

    // 4. 检查结果
    if res.status().is_success() {
        println!("✅ 邮件发送成功！HTTP 状态码: {}", res.status());
    } else {
        // 如果失败，打印出 Resend 接口返回的详细错误信息
        let error_text = res.text().await?;
        eprintln!("❌ 发送失败，Resend 返回错误: {}", error_text);
    }

    Ok(())
}
