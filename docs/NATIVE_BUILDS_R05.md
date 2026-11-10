# R05 GPUI 开发包与诊断

[源码与安全边界](../apps/manager-gpui/README.md) · [检查证据](ci-evidence-r05.json)

这些是工程开发产物，**没有真实认证设备后端**。GPUI 可以查看英文界面，设备操作会明确拒绝；不能导入真实 OTP / notes，不能解锁安全密钥。没有正式签名、安装包或移动 App。

源码提交 `c2b89add698a20636775f89cfc827d439bd58474`，三平台构建与严格检查均通过；完整日志 warning 为 0。本页摘要来自 GitHub API，未下载并运行这三个发布包。另有本地 Linux 源码构建的界面启动 / 锁定视图观察，见管理器截图；它不等于发布二进制和三系统设备互通验证。

| 产物 | 下载 | GitHub 提供的 ZIP SHA-256 |
|---|---|---|
| openkeys-gpui-development-macos-15 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42923755116/artifacts/13814170400) | `c3542af39a759b3c2a22fcb64b6b81ab98799c54c03d285aab243c66ef35a955` |
| openkeys-gpui-development-ubuntu-24.04 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42923755116/artifacts/13812274188) | `3cdc608b64272a19d3d3fe2c1c777164e0170fdfb89377f07395977ed84c081e` |
| openkeys-gpui-development-windows-2022 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42923755116/artifacts/13812139485) | `555da997ffa1f11ca26e9d73c7d3ba679788e363aded4492056f01066e0fda62` |
| r05-diagnostic-arm-elf | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42923755116/artifacts/13811849598) | `da7effedc3605bd12a07cca780d5dbb174ef5b340bb59e4b41f998b118bef751` |

macOS 目标为 Apple Silicon，Windows 为 x64 MSVC，Linux 为 Ubuntu 24.04 x86_64。平台运行库、GPU 与原生对话框仍需用户系统验证。需要 GitHub 访问权限；实际有效期见[构建元数据](native-builds-r05.json)。

R05 ARM 产物是明确标注 DFR0555 V1.0 / V1.1 的电气诊断 ELF，**不是生产认证固件**。当前受源摘要绑定的 ELF / BIN 及清单也位于 [firmware/dist/r05](../firmware/dist/r05)；只验证公开 LCD、十键与 Flash ID，SE / 充电 / USB/NFC 协议关闭。禁止输入真实 PIN 或凭据；板与模块版本必须匹配。
