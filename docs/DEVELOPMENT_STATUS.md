> 历史快照：R0.1。最新进度与缺口请以[主README](../README.md)和[制造预检](../manufacturing/README.md)为准。

# 开源认证器与 AuthDesk 管理器

这是研发工程首版，不是经过安全评估的成品。当前没有可投产 PCB，也没有能在真实指纹模组上运行的设备固件。不要用于真实账户、OTP 种子或 recovery codes。

## 本次实际完成

- 拉取并锁定 8 个上游仓库：Solo 2、Solo 2 HW、Google OpenSK、Google Wasefire、Trussed FIDO、Nitrokey 3、Zig keylib、PassKeeZ。提交见 references.lock.tsv，上游源码保留各自许可证。
- Rust 原生桌面工程 AuthDesk：跨 macOS/Linux/Windows 的构建目标；锁屏、USB 公开信息扫描、指纹/PIN 授权请求、受授权保护的文件选择/批量筛选界面。
- Rust 导入库：多条 otpauth://totp URI、版本化 JSON notes/recovery codes、输入大小限制、重复参数拒绝、请求内容摘要、RFC OTP 截断与向量测试。
- no_std Rust 授权模型：请求/连接绑定、一次性授权、超时、拒绝与断开时撤销、单调时间回退拒绝。
- 设计文档：开源项目比较、三档硬件路线、管理软件/设备授权边界、侧信道和 EMC 验证要求。

## 尚未完成的关键路径

| 交付 | 当前状态 |
|---|---|
| 管理器图形界面 | 开发版；不含模拟解锁按钮 |
| 真实 USB 加密管理协议、设备身份验证 | 未实现；USB 扫描不能代表设备可信 |
| 指纹 / 设备 PIN 验证 | 真实硬件适配未实现，默认拒绝全部授权 |
| 真实设备批量写入、OTP/notes 读取 | 未实现；不会伪报导入成功 |
| 设备 FIDO/SE/RTC/屏幕完整固件 | 未移植、未烧录验证 |
| 自制 PCB 原理图、布局、Gerber | 已增加R0原理图/结构与未布线分区草案；无Gerber，见hardware目录 |
| 三平台安装包、签名与公证 | 尚无发布版；本地/CI 检查不等于各系统实机验收 |
| 抗侧信道 / 故障注入 / EMC | 设计目标与测试计划；未完成实验室验证 |

管理器当前保持锁定：硬件后端明确拒绝解锁和写入。这是尚未接入设备的状态，不能称为已经交付可用的安全管理软件。

## 最新需求

完整目标：FIDO2/WebAuthn/Passkey/U2F、hmac-secret、PIN/重试限制、指纹、可信屏幕/确认取消、凭据管理、USB-A/USB-C/NFC、macOS/iOS 认证；恢复离线屏幕 TOTP；设备保存 OTP、notes 和 recovery codes；macOS/Linux/Windows 原生管理器。

秘密和私密元数据均须设备指纹解锁后可见；用户已明确允许设备 PIN 作为备用。桌面点击、电脑登录、OS 指纹与普通 UART 匹配信号均不能替代设备端可信验证。

自己编写的运行代码只使用 Rust，目前没有自写 C/C++，也不强行加入 Zig。操作系统图形/USB API、芯片内部 ROM/安全元件固件和参考项目的非 Rust 依赖须分别审查，不能据此宣称整个系统全栈不存在 C。

## 构建与验证

安装 Rust 后，在本目录运行：

~~~sh
cargo test --locked
cargo test -p authdesk --locked
cargo check -p authdesk --locked
cargo run -p authdesk --locked
~~~

Linux 图形版需要 X11/XWayland 和桌面图形运行库；文件对话框采用 xdg-desktop-portal。macOS/Windows 使用各系统 GUI/USB API。中文标题使用系统 CJK 字体（Linux Noto Sans CJK、macOS PingFang、Windows 微软雅黑），不重新分发专有系统字体。默认构建不包含任何参考仓库的代码。

当前已构建 Linux x86_64 优化开发版，Windows x86_64 和 Apple Silicon macOS 已通过交叉编译检查。17 项测试、格式和 clippy 检查通过；依赖扫描 0 条已知漏洞、0 条维护警告。第三方声明见 docs/THIRD_PARTY_NOTICES.txt。

详细记录见 [验证记录](VALIDATION.md)，流程见 [.github/workflows/build.yml](../.github/workflows/build.yml)。CI 文件尚未提交到远端触发，不能把工作流配置当成已构建三平台发布物。

## 导航

- [开源方案比较与语言限制](OPEN_SOURCE_REVIEW.md)
- [设备与管理器安全设计](ARCHITECTURE.md)
- [低 / 中 / 高三档方案](TIERS.md)
- [导入格式和限制](IMPORT_FORMAT.md)
- [必须通过的验证](VALIDATION.md)
