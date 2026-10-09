# Wakuwaku Mail：Thunderbird 扩展

用 Thunderbird 收信的话，不用另外登录邮箱：这个扩展读 Thunderbird 自己的收件箱（它已经帮你登录好了，Gmail、Outlook 的 OAuth 也是），把未读数发到灵动岛；来了新邮件，岛会打开一次，说是谁发的、什么主题。

发件人和主题是私密的：设置里的「显示具体内容」关掉时，岛上只显示「3 封未读」。

## 安装

1. 打包（在仓库根目录）：

   ```powershell
   pwsh integrations/thunderbird/build.ps1
   ```

   得到 `integrations/thunderbird/wakuwaku-mail.xpi`。

2. Thunderbird → 菜单 → 附加组件和主题 → 右上角齿轮 → 从文件安装附加组件，选这个 `.xpi`，确认安装。

要求 Thunderbird 115 或更新的版本。宠物的端口不是 47213 的话，改 `wakuwaku-mail/background.js` 开头的 `PORT` 再打包。

## 它做什么

- 每分钟、来新邮件时、邮件被标记已读或未读时，把所有账户收件箱的未读数加起来，发到 `http://127.0.0.1:47213/widget`（和[示例脚本](../../examples/widgets)一样）。
- 只要 `accountsRead`、`messagesRead` 两个权限，和访问本机 `127.0.0.1` 的权限；不读邮件正文，不发到别处。

---

**Wakuwaku Mail for Thunderbird.** Build with `pwsh integrations/thunderbird/build.ps1`, then in Thunderbird: Add-ons and Themes → gear → Install Add-on From File. It sums the unread mail in every account's inbox and tells the pet; a new letter opens the island once. No sign-in of its own: Thunderbird already has one.
