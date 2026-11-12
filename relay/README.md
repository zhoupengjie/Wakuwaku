# 崩溃报告中转

宠物崩溃后，会把去掉隐私的报告发到这里（`POST /report`）。中转检查报告的格式和大小，再脱敏一遍，然后在 GitHub 或 Gitee 上开一个 issue；同一个崩溃再来，只在原 issue 下加评论或计数。exe 里没有任何 token：token 只放在中转里。

有两个中转，代码是同一份 `relay.js`：

| | 部署在 | issue 开在 | 宠物什么时候先用它 |
|---|---|---|---|
| A | Cloudflare Workers（免费） | GitHub `zhoupengjie/Wakuwaku` | 中国以外 |
| B | 阿里云函数计算（国内） | Gitee `zhoupengjie/Wakuwaku` | 在中国（和更新先走 Gitee 的判断一样）|

一个发不出去就换另一个；两个都发不出去，报告留在用户电脑上，下次启动或 6 小时后再试。

两个都部署好以后，把两个地址告诉 Claude，填进 `src-tauri/src/crash.rs` 的 `GITHUB_RELAY` 和 `GITEE_RELAY`，发一个新版本，报告才会真的发出去。地址是空的时候，宠物只在本地记录崩溃，不会问，也不会发。

中转每次收到报告，会检查这些：

- 不超过 48 KB，只收认识的字段，每个字段都有长度或格式限制，多一个字段都不收；
- 同一个 IP 每分钟 5 次、每天 20 次；全部用户加起来每天最多新开 30 个 issue（都可以改）；
- 同一个崩溃（同一个签名）：前 10 次各加一条评论，之后只改正文里的计数；来了新版本再加一条评论；
- 已经关掉的 issue：旧版本发来的只计数；比 issue 里出现过的版本都新的，新开一个 issue（回归），并链接旧的；
- 报告里的文字都放在代码块里，`@` 不会提醒到任何人；
- IP 只用加盐哈希来计数，不保存原始 IP。

---

## A. GitHub + Cloudflare Workers

已部署（2026-10-11）：Worker `wakuwaku-crash`，地址 `https://wakuwaku-crash.wakuwaku-crash-relay.workers.dev`，issue 由 GitHub App `wakuwaku-crash-reports`（App ID 5274431）开。KV 的 id 写在 `wrangler.toml` 里；密钥 `GITHUB_APP_ID`、`GITHUB_APP_KEY`、`SALT` 只在 Cloudflare 里。改了 `relay.js` 以后在这个文件夹里 `npx wrangler deploy` 就行，密钥不受影响。

### 1. 让中转能在 Wakuwaku 开 issue

推荐用 **GitHub App**：issue 由 App 的机器人账号开，你会像收到别人开的 issue 一样收到通知。用你自己的 token 的话，issue 的作者是你自己，GitHub 不会因为你自己的操作通知你。

1. GitHub 右上角头像 → **Settings** → 左边最下面 **Developer settings** → **GitHub Apps** → **New GitHub App**。
2. 按下面填：
   - **GitHub App name**：`wakuwaku-crash-reports`（这个名字要全站唯一，被占了就换一个）
   - **Homepage URL**：`https://github.com/zhoupengjie/Wakuwaku`
   - **Webhook**：取消勾选 **Active**
   - **Repository permissions** → **Issues**：选 **Read and write**（Metadata 会自动变成 Read-only）
   - **Where can this GitHub App be installed?**：**Only on this account**
3. 点 **Create GitHub App**，记下页面上的 **App ID**。
4. 同一页往下拉，**Private keys** → **Generate a private key**，浏览器会下载一个 `.pem` 文件。这个文件就是钥匙，不要提交到任何地方。
5. 左边 **Install App** → 你的账号旁边点 **Install** → **Only select repositories** → 选 **Wakuwaku** → **Install**。
6. 在 Wakuwaku 的 Issues → **Labels** → **New label** 建一个叫 `crash` 的标签，颜色随意。中转开 issue 时会打上它，方便筛选。

不想建 App 的话，也可以用 fine-grained token：Settings → Developer settings → Personal access tokens → **Fine-grained tokens** → Generate new token，**Repository access** 选 Only select repositories → Wakuwaku，**Permissions** 里只把 **Issues** 设为 Read and write，有效期选一年并记个提醒。后面第 3 步把 `GITHUB_APP_ID` 和 `GITHUB_APP_KEY` 换成 `GITHUB_TOKEN`。

### 2. 注册 Cloudflare

打开 <https://dash.cloudflare.com/sign-up> 注册，免费计划就够。不用绑卡，也不用域名。

### 3. 部署

在这个文件夹里，用 PowerShell 运行（Node 已经装了，`npx` 会临时下载 wrangler）：

```powershell
cd relay
npx wrangler login
```

浏览器里点允许。然后建一个 KV（用来记计数和"哪个崩溃对应哪个 issue"）：

```powershell
npx wrangler kv namespace create CRASH
```

它会打印一个 `id = "…"`，把这个 id 填进 `wrangler.toml` 里 `<the id wrangler printed>` 的位置。

再放三个秘密，值不会写进任何文件：

```powershell
npx wrangler secret put GITHUB_APP_ID
```

粘贴第 1 步记下的 App ID，回车。

```powershell
Get-Content "$HOME\Downloads\wakuwaku-crash-reports.*.private-key.pem" -Raw | npx wrangler secret put GITHUB_APP_KEY
```

路径换成你下载的 `.pem` 文件。用管道传是因为钥匙有好几行，粘贴进提示符会断。

```powershell
[guid]::NewGuid().ToString() | npx wrangler secret put SALT
```

最后部署：

```powershell
npx wrangler deploy
```

它会打印地址，形如 `https://wakuwaku-crash.<你的子域名>.workers.dev`。

如果部署时报 `ratelimits` 相关的错误（你的计划不支持这个绑定），把 `wrangler.toml` 里 `[[ratelimits]]` 那一段删掉再部署：每天 20 次的限制靠 KV，照样有效。

### 4. 检查

```powershell
curl.exe https://wakuwaku-crash.<你的子域名>.workers.dev/
```

应该返回 `{"ok":true,"relay":"wakuwaku crash reports"}`。不要拿它测试发报告：发一份合法的报告就会在 Wakuwaku 开一个真的公开 issue。测试请用 `node test/fake-relay.cjs`（见 docs/DEVELOPMENT.md）。

---

## B. Gitee + 阿里云函数计算（国内）

已部署（2026-10-11）：华东 1（杭州）的 Web 函数 `wakuwaku-crash`，Node.js 22，0.05 vCPU / 128 MB，最小实例数 0（没有报告时不运行、不计费，第一份报告多等一两秒冷启动），HTTP 触发器"无需认证"，地址 `https://wakuwaku-crash-cpprabvetg.cn-hangzhou.fcapp.run`。环境变量 `FORGE=gitee`、`REPO`、`SALT`、`GITEE_TOKEN`（仓主自己账号的仓库级令牌，只限 Wakuwaku、只有 issues 权限）。改了 `relay.js` 以后要重新打包上传：函数详情 → 代码 → 上传代码。

注意：阿里云说默认域名（`*.fcapp.run`）"禁止在生产环境下使用"，建议绑定自定义域名；在杭州等内地地域绑定的域名要先备案。量很小时默认域名照样能用，万一被限制，宠物会改走 GitHub 那条，报告不会丢。

### 1. Gitee 的 token

Gitee 现在有**仓库级私人令牌**：只对指定仓库生效。用它的话，用自己的账号也可以，令牌泄露了别人也只能在 Wakuwaku 开、改 issue。缺点是 issue 的作者是你自己，Gitee 不会因为你自己的操作通知你；想收到通知就另建一个小号，用小号的令牌（小号不需要是 Wakuwaku 的成员，公开仓库谁都能开 issue）。

1. 确认 <https://gitee.com/zhoupengjie/Wakuwaku> 开着 Issue：仓库 → 管理 → 功能设置 → Issue。
2. 右上角头像 → **设置** → **私人令牌** → **生成新令牌**。
3. 描述写 `wakuwaku crash relay`，令牌类型选**仓库级私人令牌**、仓库选 Wakuwaku，权限**只勾 `issues`**，提交后复制令牌。令牌只显示这一次，直接粘进阿里云的环境变量里，不要发到聊天、截图或者任何文件里。

### 2. 阿里云函数计算

需要一个实名认证过的阿里云账号。函数计算按调用量计费：新账号有试用额度；一天几份报告这种量，试用期过后每月也基本是几分钱，但不保证完全免费。

1. 先打包代码（在仓库根目录）：

   ```powershell
   Compress-Archive -Force relay\relay.js, relay\server.js, relay\package.json relay.zip
   ```

2. 打开 <https://fcnext.console.aliyun.com/>，选一个离用户近的地域（比如华东 1 杭州），**函数** → **创建函数** → **Web 函数**：
   - **函数名称**：`wakuwaku-crash`
   - **运行环境**：自定义运行时 → Node.js → Node.js 22（Debian 11）
   - **代码上传方式**：通过 ZIP 包上传，选刚才的 `relay.zip`
   - **启动命令**：`node server.js`
   - **监听端口**：`9000`
   - **规格**：最小的就行（0.05 vCPU、128 MB）
   - **最小实例数**：改成 **0**（默认是 1，那样空闲时也一直计费）
   - **标签**：控制台要求填的话，随便填一个，比如键 `app`、值 `wakuwaku`
   - **环境变量**（高级配置里）：

     | 变量 | 值 |
     |---|---|
     | `FORGE` | `gitee` |
     | `REPO` | `zhoupengjie/Wakuwaku` |
     | `GITEE_TOKEN` | 第 1 步的令牌 |
     | `SALT` | 随便一串字，比如 PowerShell 里 `[guid]::NewGuid()` 的结果 |

3. 创建后，在 **触发器** 里应该已经有一个 HTTP 触发器；**认证方式**设成**无需认证**，请求方法勾上 GET 和 POST。复制它的**公网访问地址**，形如 `https://wakuwaku-crash-xxxx.cn-hangzhou.fcapp.run`。

### 3. 检查

```powershell
curl.exe https://wakuwaku-crash-xxxx.cn-hangzhou.fcapp.run/
```

应该返回 `{"ok":true,"relay":"wakuwaku crash reports"}`。

---

## 在哪里看

- GitHub：Wakuwaku → Issues，筛选 `label:crash`。仓库右上角 **Watch** 设成 All Activity，或者 Custom 里勾 Issues，手机上的 GitHub App 会推送。
- Gitee：Wakuwaku → Issues。用小号开的 issue，你的账号会收到站内通知。

## 停掉

删掉 Worker 或函数，或者吊销 token / 卸载 App 都可以。之后用户那边的报告发不出去，会留在他们电脑上（最多 20 份），每 6 小时重试一次，不会造成别的影响。
