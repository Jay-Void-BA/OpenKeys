# R05：PicoKeys、OpenSK、CanoKey 与 SoloKeys

2026-11-10 已实际拉取以下仓库的浅克隆并固定提交进行源码研究。
[机器清单](../references-r05.lock.json)与[现有参考表](../references.lock.tsv)记录精确版本。
未递归构建全部子模块，未运行上游硬件测试；本次研究不能当作这些项目的安全审计。

可用[拉取脚本](../scripts/fetch-r05-references.py)复现固定提交：
`python3 scripts/fetch-r05-references.py /tmp/openkeys-reference`。
脚本使用仓库外目录，不重置已有或有修改的检出。

| 项目 / 固定提交前缀 | 借鉴内容 | 本项目需要额外约束 |
|---|---|---|
| [Pico-FIDO](https://github.com/polhenarejos/pico-fido/tree/99337e1204348921452a3ce6613e8e17617d4c7d) `99337e12` | FIDO、OATH、计数和多应用组织 | `src/fido/oath.c` 的 calculate / calculate-all 输出路径不符合“OTP 只在钥匙显示”；不能照搬可返回 MAC / 验证码、键盘输出或种子导出能力 |
| [Pico-HSM](https://github.com/polhenarejos/pico-hsm/tree/5d6a32c81b2367fb83e5320400f61217d2bd0d2f) `5d6a32c8`；[Pico SDK](https://github.com/polhenarejos/pico-keys-sdk/tree/abf333946af04b13f2ab8f4ed05bbb5f1de5765b) `abf33394` | 智能卡命令、密钥用途和传输边界 | 不开放任意 SE APDU / 任意密钥 ID；PIN 状态必须额外受本地硬件会话和槽策略限制 |
| [Google OpenSK](https://github.com/google/OpenSK/tree/f53621c33a02eee001cae82a87aa719fca61565b) `f53621c3` | Rust FIDO 核心、PIN / token / RP / 权限和 UV / UP | 新版 `api/key_store.rs::verify_pin_hash` 可用于硬件持有验证数据；设备六位 PIN 不等同 FIDO clientPIN。仓库已集成的旧版 OpenSK 子集仍单独锁定，尚未切换此研究版本 |
| [CanoKey Core](https://github.com/canokeys/canokey-core/tree/a349b268e12953dd85169e9649091c4f91ce7c68) `a349b268`；[STM32](https://github.com/canokeys/canokey-stm32/tree/350deb9bc4c9618882b53e9337dc1d642df1a035) `350deb9b` | APDU / CCID / NFC 分段、边界、reset 和 OATH 计数测试思路 | 分段错误、断连和重新选择必须撤权；传输层不能给协议状态制造隐式共享会话 |
| [Solo2](https://github.com/solokeys/solo2/tree/45311f0b6761187d409ea3e7ea34f896810eb156) `45311f0b`；[硬件](https://github.com/solokeys/solo2-hw/tree/e7d964cb1ce775b1faa1c0bd1e694b6a124aa630) `e7d964cb` | Rust / Trussed、多应用与接口分层，OpenPGP / PIV 集成参考 | `components/oath-export` 明确提供迁移用明文 OTP 种子导出；此功能必须排除。Solo2 的 LPC55 目标和硬件不能直接用于 H573 板 |

## Rust 应用选择

Solo2 的应用组合中使用 opcard 1.8.0 与 piv-authenticator 0.7.0。
检索时 crates.io 的 opcard 为 1.11.0、piv-authenticator 为 0.7.0；它们是 Rust
候选依赖，**尚未集成或完成本板审计**。需要核查 Trussed 服务、SE 不可导出
密钥适配、算法 / 槽策略、独立 PIN、重置与生成 / 导入生命周期，不能只把菜单写上就称为支持。

SSH 不需要另造密码协议：优先分别验证 OpenSSH FIDO `*-sk`、PIV / PKCS#11
或 OpenPGP agent 路径。各平台客户端和 PIN 缓存能力不同；`git` 的 SSH 提交签名
也必须单独验证。复用已有标准不免除本地硬件 PIN 和每请求确认。

## OTP APDU 的独立来源

SE 内 MACOneShot 编码另与 [NXP plug-and-trust](https://github.com/NXP/plug-and-trust/tree/7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2)
提交 `7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2` 的 APDU / enum 定义核对：
P1 MAC `0x0D`、P2 generate-one-shot `0x45`，密钥 ID / 算法 / 八字节计数 TLV。
自研 Rust 只编码有界命令、核对精确响应长度，不读取 OTP 种子；实际 SE051C2 profile / ACL
和此指令是否满足安全策略，仍需实片资格验证。

## 许可和安全取舍

Pico 系列及 CanoKey 的 C/C++ 实现仅作阅读参考，没有编译或复制为自研运行代码。
涉及 AGPL / GPL / Apache / BSD 等材料时保留原许可，不因“参考开源”就重贴 MIT。
真正集成的第三方来源与本地补丁在 vendor 中单独记录。

这些仓库各自针对不同硬件、威胁模型和用途。默认 PIN、可导出 OTP、calculate-all、
通用 APDU、单纯触摸授权、跨应用 PIN 缓存，均不能自动沿用到本项目。
