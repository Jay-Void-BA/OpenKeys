# R0.4 当前状态

当前入口：[主README](../README.md)、[R04硬件](../hardware/r04/README.md)、[安全复查](SECURITY_REVIEW_R04.md)、[安全测试步骤](SECURITY_TEST_PLAN.md)。本文件取代旧版OTP/双USB/PIN备用状态表。

| 已完成 | 证据 |
|---|---|
| 10页原理图、130位置完整电气PCB、1430段走线及216过孔 | ERC/DRC0违规、0未连接，624项独立板网核对 |
| Gerber/钻孔/CPL、外壳/按键/指纹托源文件和尺寸图 | R04 exports；89个CAD文件SHA256绑定 |
| 被动线圈和复位RC模拟、eFuse DC角落计算 | 模型与明确假设，未冒充EMC/物理攻击模拟 |
| 设备授权/认证指纹帧/四键PIN/可信LCD策略 | no_std Rust库，真实硬件适配未部署 |
| Notes/recovery导入解析、三平台英文GUI开发构建、遮蔽/重新授权及可取消后台导入 | 真实硬件后端保持拒绝；不是可用管理器 |
| 家用/工厂/实验室测试方法 | 逐台证据格式，NOT_RUN不写PASS |
| SE通信、加密会话与持久状态 | T=1/SCP03，实际SE对象APDU、持久生物重试及vault提交锚点；候选源码/模型通过，实片配对/ACL/断电资格未通过 |
| H573密码适配 | 专用SAES寄存器状态机、检查型RNG及Secure MMIO边界，取代旧HAL密码路径；ARM编译通过，芯片KAT未执行 |
| FIDO协议核心 | 固定版本OpenSK与资源/授权网关，注册、登录、U2F、hmac-secret v1/v2及多账户连续请求测试通过；实际硬件Env/USB/NFC尚未接通 |
| SE密钥操作 | 芯片内P-256生成/签名与根密钥HKDF候选APDU，独立验签及异常关闭测试；非导出/策略/容量实片资格未执行 |
| SE目标I2C | 按真实寄存器进度区分地址忙与数据NACK，有界状态机及ARM适配通过；实际逻辑分析仪/故障测试待执行 |
| 逻辑存储与设备记录格式 | 单owner流式编辑、FIDO/notes/recovery隔离、设备端严格值解析及OpenSK持久适配已实现；真实Root/批量staging/长事务/密钥生命周期尚未完成 |

当前必须继续：按R04引脚实现目标板运行时和实际驱动；认证SE/FPC配对与持久状态；完整CTAP/vault/认证管理通道；可信启动/调试/防回滚配置；三系统与手机的真实设备互通；对样板进行上电、结构/RF/电源、EMC、侧信道和故障注入资格。缺少实物不阻止完成独立可做的源码、图纸、检查与制造资料。

新的密码路径说明：[H573核查](H573_CRYPTO_REVIEW.md)。协议/策略/密码状态机继续禁止unsafe；[Secure MMIO](../firmware/secure-mmio/README.md)单独列出六个固定地址构造的Rust unsafe边界，不再把R03的“全部自研运行代码无unsafe”泛化到R04。仍无自写或运行构建所需的C/C++。

[SE持久实现约束](SE_PERSISTENCE_REQUIREMENTS.md)记录提交锚点、撕裂/回滚区分和实际I2C NACK验证要求。这些尚未实现/实测的环节与完整运行时仍阻止成品交付；不能声称“只剩烧录”或“只差认证”。

最新源码续更：[SE状态适配](../crates/se-state/README.md)及共享SAES/RNG服务已实现；目标增加实际FPC AES256-GCM认证标签验证适配，只有标签验证成功才释放12字节payload，之后还需指纹语义检查与SE credit成功。已增加R04指纹SPI候选驱动、受保护加密请求构造与SAES发送适配；主控引导、配对/模板/取消运行时、I2C实片资格和端到端协议仍未完成。八次失败锁定的可用性与恢复策略须在制造前评审。

固定范围：指纹必须先通过，PIN仅附加；白背光16×2字符LCD；四个实体上下文按钮；唯一USB-C母口兼充电，NFC；FIDO2/U2F/hmac-secret、凭据、加密notes及recovery codes；无OTP。自己的运行代码Rust，按模块允许Zig，不引入自写C/C++。

[FPC通信核查](FPC_RUNTIME_REVIEW.md)新增阻塞HAL看门狗约束，以及PAC非安全别名/SAU事务属性核查。单纯勾选HAL功能或通过引脚类型检查均不证明安全归属。

[认证核心核查](AUTHENTICATOR_CORE_REVIEW.md)记录实际实现与尚未接通的硬件边界；[硬件验收入口](../manufacturing/HARDWARE_ACCEPTANCE.md)列明你、工厂和实验室分别需要完成的工作。

[对抗性复核](ADVERSARIAL_REVIEW_R04.md)明确当前不能确认整机“理论上不易攻破”；[大陆制造与实验室候选](../manufacturing/MAINLAND_SUPPLIERS_AND_LABS.md)附官方来源、数量/范围边界和可复制的询价内容。
