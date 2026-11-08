# R0.3 原生开发版构建

这些是**未签名、缺少真实硬件认证后端、保持锁定的开发版可执行文件**。无法实际解锁或写入设备，不用于真实凭据。它们不是正式DMG/MSI或完整成品交付。

[GitHub Actions与构建日志](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42468485292)，输入提交 `63e1e1672fc5484140cfbaeeb3297f133b239892`。下载页需GitHub访问权限；当前Actions保留期至2027-02-06。所有ZIP已与GitHub报告的SHA256核对，未执行Mac/Windows二进制。

| 平台/构建目标 | 开发版ZIP | ZIP SHA256 |
|---|---|---|
| x86_64-unknown-linux-gnu | [下载](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42468485292/artifacts/13612945120) | `2035fc5885edc92b9acc7fca085af41ff61facb5679f9268e35599801091e07c` |
| x86_64-pc-windows-msvc | [下载](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42468485292/artifacts/13612553052) | `cf2b7ce7c1eaa905a18f26fd32cd9c5af36f628ce192d7402e803eaf82d15859` |
| aarch64-apple-darwin | [下载](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42468485292/artifacts/13612254271) | `fba7049d244f2f92785af2bee7e0f9e537ef10497df240e7bf43fe89593c1fe7` |

macOS包针对Apple Silicon；未交Intel macOS版本。Linux包在Ubuntu24.04构建，不保证所有发行版的系统库兼容。Windows包针对x64 MSVC目标。设备交互、安装包签名/公证和手机NFC均需另外验收。

编译器、原始二进制摘要和限制见[build-report.json](build-report.json)；整体剩余风险见[SECURITY_REVIEW_R03](SECURITY_REVIEW_R03.md)。
