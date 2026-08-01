// ─── 腾讯国服 SGP 公共工具 ───
// 战绩查询、观战、TFT 战绩等模块共用的大区白名单与 SGP base 构造。

/// 腾讯大区白名单（SGP 仅在这些大区可用）
const TENCENT_SERVERS: &[&str] = &[
    "hn1", "hn10", "bgp2", "tj100", "cq100", "gz100", "nj100", "tj101",
];

/// 需要 k8s-sgp 子域名的特殊大区
const K8S_SGP_SERVERS: &[&str] = &["hn1", "hn10", "bgp2"];

/// 判断大区是否为腾讯国服（SGP 可用），入参为小写的大区标识
pub fn is_tencent_server(server_lower: &str) -> bool {
    TENCENT_SERVERS.contains(&server_lower)
}

/// 构建 SGP base URL，入参为小写的大区标识
pub fn sgp_base_url(server_lower: &str) -> String {
    if K8S_SGP_SERVERS.contains(&server_lower) {
        format!("https://{}-k8s-sgp.lol.qq.com:21019", server_lower)
    } else {
        format!("https://{}-sgp.lol.qq.com:21019", server_lower)
    }
}

/// 构建访问 SGP 的 HTTP 客户端（自签证书 + 不走代理）
pub fn build_sgp_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .map_err(|e| format!("创建 SGP HTTP 客户端失败: {}", e))
}
