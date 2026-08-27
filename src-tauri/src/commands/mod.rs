pub mod download;
pub mod java;
pub mod version;

use reqwest::Client;

/// 统一超时与 UA 的 HTTP 客户端,所有联网命令共用(DRY)
pub(crate) fn http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("BAMCLaunch/0.1.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))
}