use common::{
    error::{AppError, deferred::Result},
    ext::DeferResultExt,
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
    /// - `DeferredError`: 由调用 service 记录并转换为 `AppError`
    pub async fn send_message(&self, to: &str, subject: &str, body: String) -> Result<()> {
        #[cfg(feature = "metrics")]
        let start = std::time::Instant::now();
        #[cfg(feature = "metrics")]
        metrics::counter!("email:send:attempts").increment(1);

        let result = (async {
            let email = Message::builder()
                .from(
                    format!("{} <{}>", self.from_name, self.from_email)
                        .parse::<Mailbox>()
                        .defer_error(
                            "email_from_email_err",
                            "发件人地址格式错误",
                            AppError::InternalServerError,
                        )?,
                )
                .to(to.parse::<Mailbox>().defer_warn(
                    "email_to_email_err",
                    "目标邮箱格式错误",
                    AppError::bad_request("邮箱格式错误"),
                )?)
                .subject(subject)
                .header(ContentType::TEXT_HTML)
                .body(body)
                .defer_error(
                    "email_body_err",
                    "构建邮件消息失败",
                    AppError::InternalServerError,
                )?;

            self.transport.send(email).await.defer_error(
                "email_send_err",
                "邮件服务商发送失败",
                AppError::InternalServerError,
            )?;

            Ok(())
        })
        .await
        .inspect_err(|_| {
            #[cfg(feature = "metrics")]
            metrics::counter!("email:send:errors:smtp").increment(1);
        });

        #[cfg(feature = "metrics")]
        {
            if result.is_ok() {
                metrics::counter!("email:send:success").increment(1);
            }
            metrics::histogram!("email:send:duration_seconds")
                .record(start.elapsed().as_secs_f64());
        }

        result
    }
}
