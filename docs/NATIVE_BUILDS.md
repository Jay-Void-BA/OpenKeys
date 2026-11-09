# R0.4 原生开发版构建

以下是**缺少真实硬件认证后端、保持锁定的开发版可执行文件**，不是正式管理器或DMG/MSI安装包。没有开发者发布签名/Apple公证；无法实际解锁或写入设备，不用于真实凭据。

[GitHub Actions 42606524747](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42606524747)，输入提交`3bf60ce861eb38ce3174de26fa93e165ff7ae063`。三个原生目标各125项测试、严格依赖审计、编译/Clippy/格式及Linux ARM检查均成功；完整日志warning行0、warning注释0。macOS有1条平台排队容量notice。

| 平台 | 当前开发版ZIP | ZIP SHA256 |
|---|---|---|
| macOS / Apple Silicon | [下载开发包](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42606524747/artifacts/13668128488) | `f43d7bd443cf5a203c3691682a3e70ccce8401edb2a3e505a61b50e5ba0a89b4` |
| Windows / x64 | [下载开发包](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42606524747/artifacts/13669100344) | `12a1d91ae96c868288f8d1ac1b49a7fb559238ca2b136e7884c45bfcbf77a626` |
| Linux / x86_64 | [下载开发包](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42606524747/artifacts/13668456626) | `220861830639d4bad0d3e98657688bf2197dc96911cf6e6009b52f18372c69f3` |

三个ZIP均已实际下载，并与GitHub公布的SHA256一致；同时核对ELF/PE/Mach-O架构与包内rustc构建信息。未执行这些发布二进制、未做三系统原生对话框/合成器实机验证，也未连接真实设备。机器记录含二进制SHA256：[native-builds-r04.json](native-builds-r04.json)。

下载需要GitHub访问权限，当前保留至2027-02-07 UTC。macOS是Apple Silicon版本，未提供Intel版本。Linux在Ubuntu24.04构建，需要对应图形系统/系统库，不保证全部发行版。Windows针对x64 MSVC；其他架构未验证。旧R03构建只供历史追溯，见[旧构建记录](build-report.json)，不再是README下载入口。

导入已增加先遮蔽/再选文件、重新设备指纹验证、单个后台读取线程、整批大小上限、过期/旧结果丢弃及未知窗口焦点拒绝。硬件后端仍拒绝，因此这些开发包不能完成设备认证或真实导入。参考[管理器源码与限制](../apps/manager/README.md)、[R04复查](SECURITY_REVIEW_R04.md)、[安全测试M03](SECURITY_TEST_PLAN.md)。

下载后可以独立核对ZIP：macOS用`shasum -a 256 文件.zip`，Linux用`sha256sum 文件.zip`，Windows PowerShell用`Get-FileHash 文件.zip -Algorithm SHA256`，与上表比较。摘要一致只证明下载字节一致，不证明整机安全或设备认证后端已完成。
