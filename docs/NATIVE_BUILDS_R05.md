# R05 GPUI 开发包与诊断

[源码与安全边界](../apps/manager-gpui/README.md) · [检查证据](ci-evidence-r05.json)

这些是工程开发产物，**没有真实认证设备后端**。GPUI 可以查看英文界面，设备操作会明确拒绝；不能导入真实 OTP / notes，不能解锁安全密钥。没有正式签名、安装包或移动 App。

源码提交 `665c0a3d807ad49942cd228ba99b3c77d773179c`，三平台构建与严格检查均通过；完整日志 warning 为 0。本页下载信息与 ZIP 摘要来自 GitHub API；本环境尝试下载时返回 HTTP 403，未验证下载包内容或运行这些二进制。另有此前本地 Linux 源码构建的界面启动 / 锁定视图观察，见管理器截图；它不等于发布二进制和三系统设备互通验证。

| 产物 | 下载 | GitHub 提供的 ZIP SHA-256 |
|---|---|---|
| openkeys-gpui-development-macos-15 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43336263695/artifacts/13997802104) | `f27ffe1996df900c3366ca283ddd63da7ac1a32dbd0b3ee95f871a55ecd91bc2` |
| openkeys-gpui-development-ubuntu-24.04 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43336263695/artifacts/13998640820) | `f3661fe63b4d11478e00f16b7452d2de0d3d0e7ef98ef1489900cf4d1ad22205` |
| openkeys-gpui-development-windows-2022 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43336263695/artifacts/13997643213) | `e2e998c0ae271b702b407f8f5352408ddbcafdbea318bc3ef8651a447fe879b1` |
| r05-diagnostic-arm-elf | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43336263695/artifacts/13997148211) | `52d4e85a3cab0c737b3923cdc6b1e290398ff65809921f0bc91964df7cdf1aed` |

macOS 目标为 Apple Silicon，Windows 为 x64 MSVC，Linux 为 Ubuntu 24.04 x86_64。平台运行库、GPU 与原生对话框仍需用户系统验证。需要 GitHub 访问权限；实际有效期见[构建元数据](native-builds-r05.json)。

R05 ARM 产物是明确标注 DFR0555 V1.0 / V1.1 的电气诊断 ELF，**不是生产认证固件**。当前受源摘要绑定的 ELF / BIN 及清单也位于 [firmware/dist/r05](../firmware/dist/r05)；只验证公开 LCD、十键与 Flash ID，SE / 充电 / USB/NFC 协议关闭。禁止输入真实 PIN 或凭据；板与模块版本必须匹配。
