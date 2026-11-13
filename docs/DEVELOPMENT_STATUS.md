# R0.5 当前状态

[首页](../README.md) · [硬件](../hardware/r05/README.md) · [安全复查](SECURITY_REVIEW_R05.md) · [制作手册](../manufacturing/BUILD_R05_ZH.md)

**工程候选；生产放行 BLOCKED。** 当前不是可用于三个真实账户密钥的生产认证器。下表区分候选源码、软件检查和未实现的实机功能。

**先推进经典版 `main`。** [后量子版 `pqc`](EDITIONS.md)保存独立基线，暂缓实现；
SM2 在两版均排除，ML-DSA-65 / ML-KEM-768 不属于本版交付。两版均外国芯片优先、
中国芯片备选，三颗与大陆交付须确认。已将 U5 候选换为 SE051C2 / OEF A8FA / Applet 7.2，并将防拆 / RTC 辅助 LDO
从 BAT 改接 SYS、补齐局部去耦；见[电路修订](CLASSICAL_SE_POWER_R05.md)。实际原生 PIN、
RSA 与 USB 认证仍待集成。[LCD 供电 / 硬件延时修订](CLASSICAL_LCD_POWER_R05.md)已补静态角落计算，
实际电压、关断、整机负载和抗故障仍未验证。

| 部分 | 已有 | 未完成 |
|---|---|---|
| 电路 / PCB / 结构 | R05 十键、H573、SE051C2、DFR0555、NFC、供电 / LCD 故障互锁；11 页 / 169 BOM / 四层，ERC / DRC 零问题，948 项核对，制造导出 | 首板 DFM、LCD 静态角落假设 / 电容 / 关断时序 / 线束、结构、公差、RF、电源、热和 EMC / ESD 实测 |
| 本地六位 PIN | 6+6 首次设置策略、独立十键、持久预扣 / 擦除锁存；[原生 PIN v4 后端](NATIVE_PIN_RUNTIME_R05.md)、原生会话内 credit；[PIN 安装 / SE 清点 / 全 NOR 擦除组件](CLASSICAL_LIFECYCLE_R05.md)；[生命周期 journal 与 PIN 适配](../crates/root-journal/README.md)、[H573 固定两扇区安全 Flash](../crates/root-flash/README.md)、硬件 DHUK MAC、不可改写的 SE 用户代际匹配；私钥端口逐 APDU 检查同一 Execution | 整机首次初始化、生产启动 / 链接 / OTA 预留、完整 ALL-DATA 协调与恢复、Root 接线、SE profile / 真实 ACL / 原生限次资格验证；组件和模型通过不等于整机 Ready / 全部删除 |
| PIN 缓存 | 配对会话、对端、USB/NFC 生命周期绑定；硬件及六个协议角色独立 30 秒；不接受保活续期；实体确认、一次执行、撤权与回放测试；[Noise XX 与 AEAD 通道内核](../crates/secure-channel/README.md)及 CCID / NFC 共用分片编码 | 可信时间、射频离场 IRQ、USB reset / suspend、原生 PIN 会话撤权及各协议实际验证状态的 Root 接线；真实 OS 连接 / 配对存储与通道集成 |
| LCD / 十键 / 诊断 | 独立 Rust 驱动、版本明确、断电故障锁存、按键去抖 / 组合键拒绝；真实 R05 引脚构建约束 | 生产 Secure 归属、板上时序与故障响应；ARM 诊断没有认证功能 |
| 电量 / 电源 UI | USB / 电池供电候选电路、充电禁止的诊断边界 | 实际电量采集 / 校准 / UI、USB 无电池与低电量工作验证；不得用充电状态伪报电量百分比 |
| SE / MCU 密码 | T=1、SCP03、HKDF / 签名 / 持久计数、OTP MACOneShot 候选；H573 SAES / RNG 与受限 MMIO | 实片身份、唯一配对、profile / ACL、容量与耗尽、掉电原子性、KAT 和时钟 / DMA / GTZC / 调试配置 |
| CanoKey 传统兼容（SM2 除外） | 固定上游参考；已选 C2 全功能 profile 候选，纠正 E2 不支持 RSA 的限制；本版不引入 PQC | 确切到货身份与逐算法矩阵、原生 PIN / ACL、[有界长 APDU 编码已补](CLASSICAL_APDU_R05.md)，实际驱动 / RSA / 传统协议集成仍待完成，不能用主控软件私钥运算绕过硬件保护 |
| FIDO | 已固定并修改的 OpenSK 子集、注册 / 登录 / U2F / hmac-secret 网关；[本地 PIN / 独立主机 PIN](CLASSICAL_FIDO_PIN_R05.md)、指纹入口关闭、token / ECDH 撤权、v1 / v2 互通与重放测试 | 真实 H573 Env / 原生 PIN / 硬件密钥、CTAPHID / NFC、完整 Root dispatcher、租约 / token 生命周期与端到端验收 |
| TOTP / HOTP | 6 位设备显示、RFC 测试、SE MAC 不读种子、HOTP 持久预增；新增严格 URI / 设备解析与[SE 导入 / 删除](OTP_PROVISIONING_R05.md)，精确账户 / 种子 / 槽位 / 代际绑定 | 密封账户目录、pending / tombstone 跨介质事务与恢复、受保护 RTC / 时间管理、Root / 管理器适配和真机互通 |
| OpenPGP / PIV / SSH | 已拉取上游研究；协议角色与会话权限边界；管理器入口 | 真正的 OpenPGP / PIV 应用、导入 / 生成 / 证书管理与 CCID；SSH FIDO / PIV / OpenPGP 分别验证，不能视为已有可用实现 |
| notes / recovery | 加密快照、记录隔离、解析 / 事务模块 | Root 真实密钥、管理器私密数据通道、完整销毁和端到端验收 |
| GPUI 管理器 | 英文桌面界面源码；所有设备操作在缺失真实安全后端时明确拒绝 | 认证加密连接、真实导入 / 设备操作、三桌面安装包；iOS / iPadOS / Android App、权限、签名与真机测试 |
| 生产安全 | 设计约束和负向测试计划 | 可信启动、签名升级、防回滚、调试关闭、完整受保护 Root、实物防拆 / SCA / FI；均不得以模型代替 |

六位 PIN 只有一百万种组合，不能靠 PIN 长度抵挡离线提取。在线限次、不可导出 SE 对象、真实硬件隔离及物理测试缺一不可。SE_EN 逻辑只做故障互锁，不独立判断 PIN 正确。

## 证据范围

- 当前 CAD：[板网报告](../hardware/r05/review/board-check.json)、[CAD 摘要](../hardware/r05/review/cad-manifest.json)。
- 已发布的软件证据：[R05 本地验证记录](r05-validation.json)、[GitHub CI 记录](ci-evidence-r05.json)均注明对应提交；后续修改不得借用旧提交的通过结果。新内核须重新完成全仓库 / ARM / 三平台 CI 和日志核对。
- 上一次归档 CI 的软件提交 `665c0a3`：299 项 workspace 测试（含 30 项 FIDO 内核测试）及 6 项默认关闭的原生 UserID 探针测试，在 Linux / macOS / Windows 各通过 305 项；不是 915 个不同测试。完整 CI 日志与注释已核对，零 warning。新增生命周期组件以其后续提交的 CI 为准；旧结果不覆盖新增代码。CAD 对应 `0beebf7`，之后未改硬件文件时才可复用同文件检查，不能复用实物资格。
- 当前 PIN 规则：[会话与测试映射](PIN_SESSIONS_R05.md)、[原生 PIN v4 SE 布局](../manufacturing/se-native-pin-layout-r05.json)。旧 HKDF v2 布局仅供历史回归，不用于生产授权。
- 诊断用途：[R05 目标说明](../firmware/r05/README.md)；公开 LCD / 键 / Flash ID，不验证真实 PIN，不启用充电或 USB/NFC 协议。
- 旧 R04 的 [CI](ci-evidence-r04.json)、[手册](../manufacturing/BUILD_MANUAL_ZH.md)及 [预检](../manufacturing/readiness-report.json)仅供历史追溯，不能充当 R05 放行证据。

生产前须完成上述缺口、合成数据负向测试、独立实物审查、修复复测，再分别验收 K01 / K02 / K03。当前不应把诊断镜像刷进三把最终密钥并存入真实秘密。
