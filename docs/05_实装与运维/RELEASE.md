# 发布约定

- **源码库里不放安装包** (`.msi` / `.dmg` / `.AppImage` / `.deb` …, `dist/` 已被忽略; `repo_hygiene` 测试会拦住)。
- 安装包只在发版时由 CI (`ci.yml` release 任务) 或本机 `cargo tauri build` 按**当前版本号**构建, 文件名带版本, 作为 **GitHub Release 资产**单独上传。
- 发版前核对: `Cargo.toml` / `tauri.conf.json` / 安装包文件名的版本一致; 上传后用 `gh release view <tag> --json assets` 检查资产名里的版本。
