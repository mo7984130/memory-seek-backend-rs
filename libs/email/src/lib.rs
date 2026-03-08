use common::error::AppError;
use common::utils::ResultExt;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Clone)]
pub struct EmailClient {
    transport: AsyncSmtpTransport<Tokio1Executor>
}

impl EmailClient {
    pub fn new(server: &str, port: u16, user: &str, pass: &str) -> Self {
        let creds = Credentials::new(user.to_string(), pass.to_string());

        // 根据端口判断加密方式：
        let transport = if port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(server)
                .expect("无法解析 SMTP 服务器地址")
                .port(port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(server)
                .expect("无法解析 star ttl 服务器地址")
                .port(port)
                .credentials(creds)
                .build()
        };

        Self {
            transport
        }
    }

    pub async fn send_html(
        &self,
        from: &str,
        nickname: &str,
        to: &str,
        subject: &str,
        html_body: String,
    ) -> Result<(), AppError> {
        let email = Message::builder()
            .from(format!("{} <{}>", nickname, from).parse().map_internal_err("发件人地址格式错误")?)
            .to(to.parse().map_bad_request_err("目标邮箱格式错误")?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body)
            .map_internal_err("构建邮件消息失败")?;

        self.transport
            .send(email)
            .await
            .map_internal_err("邮件服务商发送失败")?;

        Ok(())
    }
}