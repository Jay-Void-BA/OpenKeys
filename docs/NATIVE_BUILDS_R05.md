# R05 GPUI 开发包与诊断

[源码与安全边界](../apps/manager-gpui/README.md) · [检查证据](ci-evidence-r05.json)

这些是工程开发产物，**没有真实认证设备后端**。GPUI 可以查看英文界面，设备操作会明确拒绝；不能导入真实 OTP / notes，不能解锁安全密钥。没有正式签名、安装包或移动 App。

源码提交 `9f697f84873ab653803c9e2a3e97aec8c3a9ac58`，三平台构建与严格检查均通过；完整日志 warning 为 0。本页摘要来自 GitHub API，未下载并运行这三个发布包。另有本地 Linux 源码构建的界面启动 / 锁定视图观察，见管理器截图；它不等于发布二进制和三系统设备互通验证。

| 产物 | 下载 | GitHub 提供的 ZIP SHA-256 |
|---|---|---|
| openkeys-gpui-development-macos-15 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43007113141/artifacts/13848264973) | `653e42923a794e69f8ab8b307750635ed256e50327ce011154a2f0fd86aa5e7d` |
| openkeys-gpui-development-ubuntu-24.04 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43007113141/artifacts/13849302204) | `95c5d4c5767c49b0797a2582a64312a32ae97585738730896b1e4a85e5dbdae9` |
| openkeys-gpui-development-windows-2022 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43007113141/artifacts/13848244773) | `618d4a7ec341eae9ef6314245bb994fbaab3c2b8902845afa7cb189ee7aebd8a` |
| r05-diagnostic-arm-elf | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43007113141/artifacts/13848588154) | `8dea0ed753ef1a8f84abf5837eba8d32ec34b718c39c9349fce9df6036cbf4f3` |

macOS 目标为 Apple Silicon，Windows 为 x64 MSVC，Linux 为 Ubuntu 24.04 x86_64。平台运行库、GPU 与原生对话框仍需用户系统验证。需要 GitHub 访问权限；实际有效期见[构建元数据](native-builds-r05.json)。

R05 ARM 产物是明确标注 DFR0555 V1.0 / V1.1 的电气诊断 ELF，**不是生产认证固件**。当前受源摘要绑定的 ELF / BIN 及清单也位于 [firmware/dist/r05](../firmware/dist/r05)；只验证公开 LCD、十键与 Flash ID，SE / 充电 / USB/NFC 协议关闭。禁止输入真实 PIN 或凭据；板与模块版本必须匹配。
