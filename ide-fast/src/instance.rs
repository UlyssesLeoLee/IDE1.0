//! 单实例: 第二次启动不再开新窗口, 把命令行里的文件交给已在运行的那一个, 然后退出。
//! 做法: 第一个实例在 127.0.0.1 随机端口监听, 端口写进设置目录的 `instance.port`; 后来的实例读到端口、连上、发路径 (一行一个) 就走。
//! 连不上 (上一个实例已经退出留下的旧端口文件) = 自己当第一个。只绑回环地址, 且只接收「路径」文本, 不执行任何东西。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

fn port_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("IDE1.0 Fast").join("instance.port"))
}

/// 如果已有实例在运行: 把 `args` 交给它, 返回 true (调用方直接退出)
pub fn forward_to_running(args: &[String]) -> bool {
    if std::env::var_os("IDE_FAST_MULTI").is_some() {
        return false; // 调试 / 测试: 允许多开
    }
    let Some(pf) = port_file() else { return false };
    let Ok(port) = std::fs::read_to_string(&pf).map(|s| s.trim().parse::<u16>().unwrap_or(0))
    else {
        return false;
    };
    if port == 0 {
        return false;
    }
    let Ok(mut s) = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        std::time::Duration::from_millis(100),
    ) else {
        // 连不上 = 上一次异常退出留下的死端口文件，立即清理，避免后续启动产生 TCP 握手超时
        let _ = std::fs::remove_file(&pf);
        return false;
    };
    let mut msg = String::from("IDE-FAST-OPEN\n");
    for a in args {
        // 相对路径按「调用者」的当前目录解析成绝对路径再发
        let p = std::path::absolute(a)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| a.clone());
        msg.push_str(&p.replace('\n', " "));
        msg.push('\n');
    }
    msg.push_str("END\n");
    if s.write_all(msg.as_bytes()).is_err() {
        let _ = std::fs::remove_file(&pf);
        return false;
    }
    let mut resp = String::new();
    let mut reader = BufReader::new(s);
    if reader.read_line(&mut resp).is_err() || resp.trim() != "OK" {
        // 对方没有应答 OK (可能是占用该端口的其他服务): 判定转发失败，删除死端口
        let _ = std::fs::remove_file(&pf);
        return false;
    }
    true
}

/// 进程正常退出或关闭时清理残留端口文件
pub fn cleanup() {
    if let Some(pf) = port_file() {
        let _ = std::fs::remove_file(pf);
    }
}

/// 成为第一个实例: 开始监听, 返回收「别的实例转来的路径」的通道
pub fn listen() -> Option<Receiver<Vec<String>>> {
    if std::env::var_os("IDE_FAST_MULTI").is_some() {
        return None;
    }
    let l = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = l.local_addr().ok()?.port();
    let pf = port_file()?;
    if let Some(d) = pf.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(&pf, port.to_string()).ok()?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for conn in l.incoming().flatten() {
            let mut r = BufReader::new(conn);
            let mut first = String::new();
            if r.read_line(&mut first).is_err() || first.trim() != "IDE-FAST-OPEN" {
                continue; // 不是我们的协议: 忽略
            }
            let mut paths = vec![];
            let mut line = String::new();
            while r.read_line(&mut line).map(|n| n > 0).unwrap_or(false) {
                let t = line.trim_end_matches(['\r', '\n']).to_string();
                line.clear();
                if t == "END" {
                    break;
                }
                if !t.is_empty() && paths.len() < 64 {
                    paths.push(t);
                }
            }
            let _ = r.get_mut().write_all(b"OK\n");
            let _ = tx.send(paths);
        }
    });
    Some(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_instance_hands_paths_to_the_first() {
        let dir = std::env::temp_dir().join(format!("ide-fast-inst-{}", std::process::id()));
        std::env::set_var("APPDATA", &dir);
        std::env::remove_var("IDE_FAST_MULTI");
        let rx = listen().expect("first instance listens");
        assert!(
            forward_to_running(&["a.txt:3".to_string(), "C:\\x\\b.md".to_string()]),
            "第二个实例连得上"
        );
        let got = rx
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("first gets the paths");
        assert_eq!(got.len(), 2);
        assert!(
            got[0].ends_with("a.txt:3") && got[1].ends_with("b.md"),
            "{got:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
