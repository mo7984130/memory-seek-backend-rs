/// 本地存储错误。
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// 底层 IO 失败(读写文件、创建目录等)。
    #[error("存储 IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 序列化 / 反序列化失败(如存储文件损坏)。
    #[error("存储序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    /// 无法定位存储位置(如各平台数据目录解析失败)。
    #[error("存储路径错误: {0}")]
    Path(String),
}
