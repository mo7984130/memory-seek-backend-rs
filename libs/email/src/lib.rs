use common::error::AppError;
use common::ext::ResultErrExt;
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
    /// 创建邮件客户端
    ///
    /// # 参数
    /// - `server`: SMTP 服务器地址
    /// - `port`: SMTP 端口号（465 使用 SSL，其他使用 STARTTLS）
    /// - `user`: SMTP 用户名
    /// - `pass`: SMTP 密码
    /// - `from_email`: 发件人邮箱地址
    /// - `from_name`: 发件人显示名称
    ///
    /// # 返回
    /// 初始化完成的 `EmailClient` 实例
    pub fn new(
        server: &str,
        port: u16,
        user: &str,
        pass: &str,
        from_email: &str,
        from_name: &str,
    ) -> Self {
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
            transport,
            from_email: from_email.to_string(),
            from_name: from_name.to_string(),
        }
    }

    /// 发送 HTML 格式邮件
    ///
    /// # 参数
    /// - `to`: 收件人邮箱地址
    /// - `subject`: 邮件主题
    /// - `body`: HTML 格式的邮件正文
    ///
    /// # 返回
    /// 发送成功返回 `()`
    ///
    /// # 错误
    /// - `AppError::BadRequest`: 收件人邮箱格式无效
    /// - `AppError::InternalServerError`: 发件人地址格式错误、邮件构建失败或 SMTP 发送失败
    pub async fn send_message(
        &self,
        to: &str,
        subject: &str,
        body: String,
    ) -> Result<(), AppError> {
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.from_name, self.from_email)
                    .parse::<Mailbox>()
                    .trace_to_internal_err("email_from_email_err", "发件人地址格式错误")?,
            )
            .to(to
                .parse::<Mailbox>()
                .trace_to_bad_request_warn("email_to_email_err", "目标邮箱格式错误")?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body)
            .trace_to_internal_err("email_body_err", "构建邮件消息失败")?;

        self.transport
            .send(email)
            .await
            .trace_to_internal_err("email_send_err", "邮件服务商发送失败")?;

        Ok(())
    }
}
