use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, AsyncSmtpTransport, Tokio1Executor, AsyncTransport};

#[tokio::main]
async fn main() {
    // 1. 构建你要发送的邮件内容
    let email = Message::builder()
        // 发件人：填写你的【发送方邮箱地址】，格式严格保持 "名字 <邮箱>"
        .from("Rust Tester <kirisamefreeman@gmail.com>".parse().unwrap()) 
        // 收件人：填写你的【ProtonMail 邮箱地址】
        .to("My Proton <free514dom@proton.me>".parse().unwrap())     
        .subject("来自 Rust 的测试邮件")
        .header(ContentType::TEXT_PLAIN)
        .body(String::from("你好！当你看到这封信时，说明你的 Rust 程序成功连接了 SMTP 并发送了邮件。"))
        .unwrap();

    // 2. 配置 SMTP 凭证 (发送方邮箱的账号和密码)
    let creds = Credentials::new(
        "your_sender@gmail.com".to_owned(), // 【需要替换：你的发送方邮箱账号】
        "your_app_password".to_owned(),     // 【需要替换：该邮箱的“应用专用密码” / 授权码，绝大多数不是网页登录密码！】
    );

    // 3. 设置 SMTP 服务器地址 (以 Gmail 为例)
    // 如果你用的是 QQ 邮箱，这里是 "smtp.qq.com"
    // 如果是 163 邮箱，这里是 "smtp.163.com"
    let mailer: AsyncSmtpTransport<Tokio1Executor> =
        AsyncSmtpTransport::<Tokio1Executor>::relay("smtp.gmail.com") // 【需要替换：你发送方邮箱对应的 SMTP 地址】
            .unwrap()
            .credentials(creds)
            .build();

    println!("正在尝试发送邮件，请稍候...");

    // 4. 发送邮件并处理结果
    match mailer.send(email).await {
        Ok(_) => println!("✅ 邮件发送成功！快去 ProtonMail 查收吧。"),
        Err(e) => eprintln!("❌ 邮件发送失败: {:?}", e),
    }
}
