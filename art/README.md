# art/

美术素材和调查资料。**除了这份说明，目录里的东西都不进仓库**（见 `.gitignore`）：它们要么是别人角色的二创，要么是别人的原作参考，和代码的 MIT 许可无关，也不随安装包分发。

```
art/
├─ claude-chan/        Claude 小姐像素宠物（Codex pet v2 格式），按版本放
│  ├─ v1 … v1.3        每版的 final/（pet.json + spritesheet）、帧、预览、QA、脚本
│  └─ v1.2-review      v1.2 待机动画的检查记录
├─ ocean-maid/         蓝发海洋女仆：图标、托盘图标、表情、插画
│  ├─ v1/              素材包（打开 preview.html 看整套效果）
│  └─ export-assets.js 从三张生成图导出整套素材：node art/ocean-maid/export-assets.js <icon> <peek> <body>
├─ references/         参考图，别人的作品，只作对照
│  ├─ claude/  deepseek/  nanally/
└─ research/           调查资料
   └─ zipzippipe-llm-avatars/   ZipZipPipe 的 LLM 拟人角色记录（含来源链接）
```

## 署名

- Claude 小姐、DeepSeek 娘等 LLM 拟人形象的原作者是 B 站 UP 主 **ZipZipPipe**（[主页](https://space.bilibili.com/4168597)）。`claude-chan/` 是基于其 Claude 小姐形象的非官方像素二创。
- 默认宠物 [大肥鱼/Deepseek Chan](https://codex-pets.net/#/pets/deepseek-chan) 的作者是 Dullsaw；`ocean-maid/` 参考了它的造型。
- `references/` 里的图版权归各自作者。

要把某套素材放进应用（比如做成默认宠物或换掉图标），需要先确认原作者的授权。
