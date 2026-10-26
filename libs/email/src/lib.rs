use common_core::error::{
    AppError,
    contextual::{Result, ext::ResultExt},
};
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Clone)]
pub struct EmailClient {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from_email: String,
    from_name: String,
}

impl EmailClient {
    /// 创建邮件客户端；`plain` 为真时明文传输,否则 465 端口走 SSL、其余端口走 STARTTLS；服务器地址无法解析时 panic
    pub fn new(
        server: &str,
        port: u16,
        user: &str,
        pass: &str,
        from_email: &str,
        from_name: &str,
        plain: bool,
    ) -> Self {
        let creds = Credentials::new(user.to_string(), pass.to_string());

        // 根据端口与传输模式决定加密方式：
        let transport = if plain {
            // 明文：MailHog 等测试 SMTP
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(server)
                .port(port)
                .credentials(creds)
                .build()
        } else if port == 465 {
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
            transport,
            from_email: from_email.to_string(),
            from_name: from_name.to_string(),
        }
    }

    /// 发送 HTML 格式邮件
    pub async fn send_message(&self, to: &str, subject: &str, body: String) -> Result<()> {
        // 操作级指标：email:send:attempts / duration_seconds / success
        common_metrics::metrics_group!("send");

        let email = Message::builder()
            .from(
                format!("{} <{}>", self.from_name, self.from_email)
                    .parse::<Mailbox>()
                    .context_error(
                        "email_from_email_err",
                        "发件人地址格式错误",
                        AppError::InternalServerError,
                    )?,
            )
            .to(to.parse::<Mailbox>().context_warn(
                "email_to_email_err",
                "目标邮箱格式错误",
                AppError::bad_request("邮箱格式错误"),
            )?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body)
            .context_error(
                "email_body_err",
                "构建邮件消息失败",
                AppError::InternalServerError,
            )?;

        let send_result = self.transport.send(email).await.context_error(
            "email_send_err",
            "邮件服务商发送失败",
            AppError::InternalServerError,
        );

        send_result?;

        common_metrics::metrics_success!("send");

        Ok(())
    }
}
