# Wakuwaku

开发说明在 [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)：目录、构建和运行、各部分怎么工作、踩过的坑。动手前先看相关的那一节；加了、拆了、搬了文件，同步改它的「目录」。

- **语言**：代码注释和提交信息用英文，用「她」的口吻（她就是这只桌宠，照现有代码的写法）；`docs/` 下的文档用中文。
- **提交信息**：标题一行英文，不超过 72 个字符左右，说清楚改了什么；空一行写正文，讲为什么改、怎么验证的，72 列左右换行；末尾带 `Co-Authored-By`。详见 DEVELOPMENT.md 的「提交信息」。
- **PR** 开到 `dev`，不开到 `main`。
- **测试**：Rust 在 `src-tauri` 里 `cargo test`；页面在仓库根目录 `node --test "test/*.test.js"`。改了页面，用 `tools/preview` 拍改动前后的对比图（见它的 README）。
- **发布用的 exe** 用 `pwsh scripts/build-release.ps1` 编，不要直接 `cargo build --release`（为什么见 DEVELOPMENT.md「构建和运行」）。
- 仓库是公开的：代码、文档、提交信息里不写个人信息（账号、机器名、IP、本机路径）。
