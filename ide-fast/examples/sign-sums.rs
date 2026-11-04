//! 发布签名小工具 (只给项目所有者在自己机器上用; 私钥永远不进仓库)。
//!
//! ```text
//! cargo run --release -p ide-fast --example sign-sums -- pubkey <私钥文件>
//!     打印要粘进 src/updkey.rs 的 `UPDATE_PUBKEY` 数组
//! cargo run --release -p ide-fast --example sign-sums -- sign <私钥文件> <SHA256SUMS.txt> <输出 .sig>
//!     对清单的原始字节做 ed25519 签名, 写出原始 64 字节的分离签名, 并用对应公钥自检一遍
//! ```
//!
//! 私钥文件 = 32 字节种子, 存成 64 个十六进制字符 (首尾空白忽略) 或恰好 32 个原始字节。生成见 `packaging/sign-release.ps1 -GenKey` / `docs/UPDATE.md`。

use ed25519_dalek::{Signer, SigningKey};
use std::process::ExitCode;

fn load_key(path: &str) -> Result<SigningKey, String> {
    let raw = std::fs::read(path).map_err(|e| format!("读不了私钥文件: {e}"))?;
    let text = String::from_utf8_lossy(&raw);
    let hex = text.trim();
    let seed: [u8; 32] = if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        let mut s = [0u8; 32];
        for (i, b) in s.iter_mut().enumerate() {
            *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|e| e.to_string())?;
        }
        s
    } else {
        raw.as_slice().try_into().map_err(|_| "私钥文件应是 64 个十六进制字符或 32 个原始字节".to_string())?
    };
    Ok(SigningKey::from_bytes(&seed))
}

fn run(args: &[String]) -> Result<(), String> {
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["pubkey", key] => {
            let pk = load_key(key)?.verifying_key().to_bytes();
            let items: Vec<String> = pk.iter().map(|b| format!("0x{b:02x}")).collect();
            println!("pub const UPDATE_PUBKEY: [u8; 32] = [{}];", items.join(", "));
            Ok(())
        }
        ["sign", key, sums, out] => {
            let sk = load_key(key)?;
            let data = std::fs::read(sums).map_err(|e| format!("读不了清单: {e}"))?;
            let sig = sk.sign(&data);
            sk.verifying_key().verify_strict(&data, &sig).map_err(|e| format!("自检失败: {e}"))?;
            std::fs::write(out, sig.to_bytes()).map_err(|e| format!("写不了签名: {e}"))?;
            println!("已签名 {} 字节 -> {out} (64 字节原始 ed25519 签名)", data.len());
            Ok(())
        }
        _ => Err("用法: sign-sums pubkey <私钥文件> | sign-sums sign <私钥文件> <SHA256SUMS.txt> <输出.sig>".into()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
