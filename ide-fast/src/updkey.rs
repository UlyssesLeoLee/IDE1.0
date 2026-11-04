//! 更新包验签用的公钥 (ed25519, 32 字节)。**只有公钥**; 私钥由项目所有者自己保管, 不进仓库、不进构建 (见 `docs/UPDATE.md`)。
//!
//! 发行版里这是唯一的信任根: `app/updateui.rs` 只会把 `UPDATE_PUBKEY` 传给 `updnet::stage`, 没有环境变量 / 配置文件 / 命令行可以改它或绕过它。
//! 单元测试直接把运行时生成的测试公钥作为函数参数传进 `stage`, 不碰这个常量。

/// 「尚未配置」占位 = 全 0。此时更新窗口提示「更新签名密钥未配置」, 禁用「安装并重启」, 只给发布页链接。
/// 所有者生成密钥对之后, 用 `cargo run --example sign-sums -- pubkey <私钥文件>` 打印的数组替换这里 (见 UPDATE.md)。
pub const UPDATE_PUBKEY: [u8; 32] = [0; 32];

/// 公钥是否已配置 (不是全 0 占位)
pub fn is_configured(key: &[u8; 32]) -> bool {
    key.iter().any(|&b| b != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_means_not_configured() {
        assert!(!is_configured(&[0; 32]));
        let mut k = [0u8; 32];
        k[31] = 1;
        assert!(is_configured(&k));
    }
}
