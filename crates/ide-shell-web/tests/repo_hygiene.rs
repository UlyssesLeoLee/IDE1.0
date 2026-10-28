//! 仓库卫生: 安装包 / 二进制产物不能进源码库 (曾经 0.1.2 的源码里带着 0.1.0 的 .msi)。
//! 安装包按版本构建, 作为 Release 资产单独上传。

#[test]
fn no_installers_tracked_in_git() {
    let out = match std::process::Command::new("git").args(["ls-files"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return, // 不在 git 工作区 (例如从源码包构建) 就跳过
    };
    let list = String::from_utf8_lossy(&out.stdout);
    let bad: Vec<&str> = list
        .lines()
        .filter(|l| {
            let l = l.to_ascii_lowercase();
            [".msi", ".dmg", ".appimage", ".deb", ".rpm"].iter().any(|e| l.ends_with(e)) || l.ends_with("-setup.exe")
        })
        .collect();
    assert!(bad.is_empty(), "安装包不应入库, 请改为 Release 资产: {bad:?}");
}
