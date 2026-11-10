# R0.4 当前状态

[文档导航](README.md) · [开发起步](GETTING_STARTED.md) · [对抗性复核](ADVERSARIAL_REVIEW_R04.md) · [制造入口](../manufacturing/README.md)

**阶段：工程候选。生产放行：BLOCKED。** 完整认证运行时和真实管理通道尚未完成；当前无法作为三把正式安全密钥交付。

## 状态怎么读

| 标记 | 含义 |
|---|---|
| 已有设计 / 源码 | 文件或候选实现存在；不表示完成实物集成 |
| 软件检查通过 | 对明确提交的源码、模型或构建验证通过 |
| 待集成 | 尚未接入完整受保护目标运行时 |
| NOT_RUN | 尚未进行所需实物测试，不能视为通过 |
| BLOCKED | 缺少交付条件；不放行生产认证器 |

## 各部分进度

| 部分 | 已完成 | 还需要完成 |
|---|---|---|
| 电路 / PCB / 结构 | 10 页原理图、130 个 BOM 位置、完整四层布线、制造导出和结构源；ERC / DRC 与 624 项独立核对通过 | 首板装配、DFM、结构公差、电源 / RF / 热及实物安全资格 |
| 授权 / 指纹 / PIN / LCD | 认证帧校验、持久尝试接口、四键 PIN、完整身份分页、一次请求执行范围及撤权检查 | 真实配对 / 模板 / PIN / 可信显示运行时和安全 GPIO 路径 |
| SE 通信 / 密钥 | 有界 T=1、SCP03、I2C 进度状态机；SE 对象 / 计数 / 签名 / HKDF 候选 APDU | 实片身份、profile / ACL、计数与掉电语义、容量、配对和密钥生命周期 |
| MCU 密码 | H573 SAES / RNG、共享所有权与故障关闭、明确 Secure MMIO 边界；ARM 编译通过 | 芯片 KAT、实际 Secure 地址 / SAU / GTZC / DMA / watchdog 核查及侧信道测试 |
| FIDO 核心 | 固定 OpenSK、UV / UP 网关、注册 / 登录 / U2F / hmac-secret、连续请求和持久适配测试 | 生产硬件 Env、硬件密钥桥接、USB / NFC、完整 Root dispatcher |
| 加密存储 | 两区快照、流式编辑、记录隔离、notes / recovery 校验、SE 提交锚点模型 | Root 检查点、批量 staging、可取消长事务、安全删除 / 重置生命周期 |
| 桌面管理器 | 英文 GUI、三平台开发包、受限导入、失焦 / 休眠 / 过期遮蔽 | 认证加密管理通道、真实设备导入和三系统交互验收、正式安装包 |
| 手机管理器 | Android / iOS NFC 实现前提 | 设备协议、可用 APK / IPA、签名与真机 NFC 管理验证 |
| 启动 / 更新 / 防拆 | 架构约束、复位 / 供电门控和测试计划 | 生产可信启动、签名更新、防降级、调试闭合及防拆策略部署 |
| 实物抗攻击 | 已列验收步骤和失败判据 | EMC / ESD、故障注入、SCA、PAD 等均 NOT_RUN |

## 当前证据

- **软件提交**：`029ca7f4ded00bc4fc7c4ddc8fabeeff9405c559`，三平台各 **191** 项测试；[CI 42723819737](https://github.com/Jay-Void-BA/OpenKeys/actions/runs/42723819737) 全部成功，完整日志和注释中 warning 均为 0。
- **CAD**：89 个文件摘要保持绑定；[板网核对](../hardware/r04/review/board-check.json)和[CAD 清单](../hardware/r04/review/cad-manifest.json)。
- **产物**：三平台锁定开发包和电气诊断；已核对下载摘要 / 架构，未执行这些发布二进制或连接真实设备。见[开发包记录](NATIVE_BUILDS.md)。
- **交付**：[生产预检](../manufacturing/readiness-report.json)仍有 13 项未完成条件。它们不等于下述安全风险的数量。

机器证据：[CI 摘要](ci-evidence-r04.json)、[构建记录](r04-build-report.json)、[依赖审计来源](dependency-audit-provenance.json)。历次结果保留在[验证时间线](VALIDATION.md)，旧结果不覆盖后续源码。

## 交付前的关键工作

1. 完成生产启动、隔离、真实 Root 与硬件密钥 / 授权路径。
2. 完成 SE / 传感器配对、持久 PIN、模板 / 重置及可用性策略。
3. 接通 USB / NFC 认证、认证加密管理和真实桌面操作。
4. 完成首板、三平台 / 手机互通和独立安全测试，修复后复测。
5. 冻结生产资料，再对 K01 / K02 / K03 逐台验收。

电路中的 SE 使能门不是独立指纹证明。八次指纹失败锁定、标签 / 密钥槽耗尽、对称配对密钥、LCD 路径和触点桥接等仍需评审；完整事项见[对抗性复核](ADVERSARIAL_REVIEW_R04.md)。

## 固定需求

设备指纹必须先通过，PIN 仅附加；白背光 16 × 2 英文字符 LCD、四键、唯一 USB-C 母口兼充电、NFC；FIDO2 / U2F / hmac-secret、凭据、notes / recovery codes；无 OTP。自研运行代码采用 Rust，允许按模块使用 Zig，不引入自写 C / C++。
