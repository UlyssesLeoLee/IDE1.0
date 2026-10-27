//! 守卫: 网页版内嵌的前端 (src/editor.html) 和桌面版打包的前端 (ide-shell-desktop/dist/index.html) 必须逐字节一致。
//! 桌面版的 build.rs 会在构建时把前者复制到后者; 这里保证提交进仓库的两份没有漂移 (只改了一份就提交)。

#[test]
fn desktop_dist_matches_the_embedded_editor() {
    let web = include_str!("../src/editor.html");
    let desktop = include_str!("../../ide-shell-desktop/dist/index.html");
    assert!(
        web == desktop,
        "crates/ide-shell-web/src/editor.html 与 crates/ide-shell-desktop/dist/index.html 不一致: 运行 `cargo build -p ide-shell-desktop` (build.rs 会同步) 后一起提交"
    );
}
