# R05 GPUI 开发包与诊断

[源码与安全边界](../apps/manager-gpui/README.md) · [检查证据](ci-evidence-r05.json)

这些是工程开发产物，**没有真实认证设备后端**。GPUI 可以查看英文界面，设备操作明确拒绝；不能导入真实 OTP / notes 或解锁安全密钥。没有正式签名、安装包或移动 App。

源码提交 `3a942d3ed2741ff67866985b6e0d73f1efadd3a4` 的[三平台 CI](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147314)、[209 位 CAD CI](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147340)和[文档 CI](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147338)均通过。各平台372项测试（366项 workspace / 文档及6项默认关闭探针），完整日志和注释零 warning；另有2条 notice。文件 / 编译 / 模型结果不能证明实物安全。

下表 ZIP 摘要来自 GitHub API。逐包下载核验结果见[构建元数据](native-builds-r05.json)；这些二进制未在本次执行，不能视为三系统设备互通验收。

| 产物 | 下载 | GitHub 提供的 ZIP SHA-256 |
|---|---|---|
| r05-diagnostic-arm-elf | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147314/artifacts/14145425014) | `sha256:eacd80cf8ad3c3c4d353aa587184c698e259e2479f9b4b1507dffa0653fb8960` |
| openkeys-gpui-development-windows-2022 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147314/artifacts/14145012674) | `sha256:8009e976929dbaa7f0a4875c11c9d8d7b476e487742260fc936be41346a3f018` |
| openkeys-gpui-development-macos-15 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147314/artifacts/14144957367) | `sha256:cb95b51c243a38837679974751cbb937ea8fc4b5a5332e8eaeaf90caa8cc6be5` |
| openkeys-gpui-development-ubuntu-24.04 | [Download](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/43672147314/artifacts/14144537956) | `sha256:5d8da3f29279a16e9b0a251bf0daf92ac21eed911b2fb56af5c8ab784f37812b` |

macOS 目标为 Apple Silicon，Windows 为 x64 MSVC，Linux 为 Ubuntu24.04 x86_64。平台运行库、GPU、原生对话框、下载权限及有效期分别核实。

本轮 ARM 诊断已绑定209位PCB及其引脚表；旧169/195位版本的诊断证据不能代替它。受源摘要绑定的 V1.0 / V1.1 ELF / BIN 位于 [firmware/dist/r05](../firmware/dist/r05)，**不是生产认证固件**。它们只验证公开 LCD、十键与 Flash ID，SE / 充电 / USB/NFC 协议关闭；不要输入真实 PIN / 凭据。
