# 安全与性能设计目标

此文件是工程设计，不是已实现或认证的安全声明。已实现边界见 README 状态表。

## 信任边界

~~~mermaid
flowchart LR
    H[Mac / Linux / Windows 管理器] --> I[USB 输入边界：长度/分片/命令验证]
    N[iPhone NFC / USB FIDO] --> I
    I --> T[受保护授权域：设备身份、请求绑定、指纹或设备 PIN]
    F[可信指纹方案] --> T
    U[设备屏幕 / 确认 / 取消] <--> T
    T --> S[SE：FIDO 私钥、hmac-secret、OTP HMAC、数据密钥保护]
    T --> R[RTC / 时间有效性]
    T --> D[加密 notes / recovery codes / 元数据]
~~~

主机、USB/NFC 输入、外部 Flash、普通指纹模块都不能默认可信。UI 与按键、指纹/PIN 验证、实际请求及 SE 使用策略必须处于同一授权链。安全芯片不能独立判断“屏幕是否显示了正确请求”；受保护固件必须承担该责任。

## 功能与授权

| 操作 | 必须条件 |
|---|---|
| USB 枚举、CTAP GetInfo/公开能力协商 | 不含秘密、账户列表或 notes 元数据；保持标准客户端可发现性 |
| 私密列表 / 标题 / OTP 账户名 | 设备验证指纹，或设备端 PIN 备用；短时会话内可显示 |
| FIDO 签名、hmac-secret 派生 | 设备 UV、正确 RP/请求绑定、用户确认；NFC 路径同样执行 |
| 设备屏幕显示 TOTP | 已验证用户；可信 RTC；仅显示当前验证码，不显示种子 |
| Notes/recovery code 明文 | 新鲜授权与设备确认；默认仅设备显示；向主机释放属于单独明确授权 |
| 导入 / 删除 / 标记已使用 | 绑定完整内容、条目版本与目标设备的确认；批量上限；原子提交 |
| 指纹录入/删除、PIN 更改、时间校准、恢复/备份 | 已有可信身份验证及明确确认；没有“管理员默认密码”旁路 |

R0.2指纹目标为FPC2534AM，协议策略见[BIOMETRIC_PROTOCOL](BIOMETRIC_PROTOCOL.md)；真实硬件后端仍未实现。用户已允许 PIN 备用，但必须是设备 PIN。管理器不收集指纹图像，不用电脑指纹或一个布尔值替代设备验证。首次初始化尚无指纹时，需要设备物理初始化流程先设置 PIN、建立受保护策略，再录入指纹；恢复不能静默关闭授权。

当前 device-policy 授权模型将单个命令与其摘要/连接绑定，15 秒后失效且一次性消费。它没有实现 SE 计数器、指纹匹配、设备身份或断电持久性；SecureVerifier 只能由未来受保护硬件适配层实现。

## 密钥与数据

- FIDO 私钥在 SE 内生成并签名；hmac-secret 的 CredRandom/HMAC 根秘密在同一受保护边界内运算。不能把密钥解封到普通 MCU RAM 后继承 SE 的抗侧信道宣传。
- OTP 导入的种子本来就存在于用户的导出文件与主机内存。传输用经过认证的加密通道，导入后由 SE 持有/运算；导入临时缓冲应擦除，不能宣称导入链路中从未有明文。
- Notes/recovery codes 和标题等元数据加密且认证存储；数据密钥由 SE 保护。读取后只进入获授权显示路径。外部 Flash 的 AEAD 不自动防回滚，需要持久版本/根状态和掉电原子提交。
- AEAD nonce 唯一性、密钥轮换、导入幂等性及掉电恢复顺序须单独制定并验证；当前源码没有实现磁盘或设备持久存储加密。
- PIN 与指纹失败计数必须由受保护持久状态强制，断电不能重置。SE050 认证对象的计数规则、不可原地改写的 UserID、对象策略迁移和 SCP03 配对必须逐项验证，不能只在主机 Flash 存一个 retries 数字。
- 每台设备使用独立配对密钥。上游/开发示例里的默认 SCP03 密钥不能用于成品。
- 三把 FIDO 钥匙分别注册。OTP/notes 备份另定义受授权的加密恢复流程，不提供默认证明不存在的 FIDO 私钥复制。

参考：[NXP SE050](https://www.nxp.com/docs/en/data-sheet/SE050-DATASHEET.pdf)、[APDU 规则](https://www.nxp.com/docs/en/application-note/AN12413.pdf)。NXP 安全元件内部 applet/ROM 不是本项目可审查的 Rust 源码。

## 可信屏幕、NFC、OTP 时间

- FIDO2 显示与实际认证绑定的 RP ID，避免仅显示主机传来的友好名称；长域名可完整滚动查看，拒绝文本控制符误导。
- U2F 标准请求只有应用 ID 哈希，未知映射不能编造域名；显示“旧版 U2F / 未知站点标识”与可核对标识。[U2F 规范](https://fidoalliance.org/specs/fido-u2f-v1.2-ps-20170411/fido-u2f-raw-message-formats-v1.2-ps-20170411.html)
- 带按键/指纹的 NFC 认证不能直接把进场当成用户确认；屏幕/指纹功耗由电池/供电设计保障，不能假定 RF 采能足够。验证目标 iPhone 的等待与 keepalive 行为。
- 离线 TOTP 需要低功耗 RTC 和电池。时钟无效、回拨或电池掉电时拒绝生成“看似正常”的码，要求设备确认后校时。主机不能偷偷修改时间。
- Notes 和 recovery codes 在小屏幕上分页显示，可信确认按钮区域固定；导入内容不能覆盖或模拟系统授权界面。设备字体属于签名固件/受认证资源，不接收电脑随意提供的确认画面。
- SHA1 仅用于兼容 RFC TOTP/HOTP，不能扩大为固件签名/新通道的密码选择。[RFC 6238](https://www.rfc-editor.org/rfc/rfc6238)

## 管理器与协议

选择原生 Rust GUI（egui/eframe），不引入浏览器页面来处理秘密。协议核心与 GUI 分离。Linux/macOS/Windows 同一业务逻辑；系统 GUI/USB 库属于操作系统信任边界。

拟采用标准化、经审查的认证加密通道。设备身份必须可核对且可绑定，消息具有方向、序号、会话和操作权限。当前只实现 USB 枚举，没有实现通道握手，因此不接受任何真实秘密。不可把 VID/PID、序列号或无密钥 SHA256 摘要当作设备身份验证。

拟定操作：公开能力；请求本地 UV；列私密元数据；导入预检/分块/提交；撤销导入；生成 OTP；设备内显示 note/code；单独批准向主机释放；删除；PIN/指纹管理；RTC 校准。导入内容摘要由设备对解密后数据重新计算，不能盲信管理器预览。

管理器无账户系统、网络同步、遥测、默认剪贴板复制或明文数据库。依赖中的系统文件对话框可能与本机桌面 portal 通信；这不构成云同步。若明文已被允许释放给电脑，被控制的操作系统仍可窃取，指纹不能让受感染主机变安全。

前端文件选择和预览按权限门控；失去焦点/恢复后锁定并清除暂存缓冲。当前 no-unsafe 是本项目源码约束，不涵盖上游 GUI、USB、系统库或硬件。zeroize 只能尽量清理拥有的缓冲，不能保证交换区、解析中间对象、GPU、OS 崩溃转储中的所有历史副本已抹除。

## 性能目标（待硬件实测）

| 项目 | 目标/约束 |
|---|---|
| 通信解析 | 有界帧与分片长度、固定最大并发和超时，不阻塞取消/看门狗 |
| FIDO/UI | 分开记录解析、SE 运算、渲染和用户验证耗时；不把指纹等待计成密码运算性能 |
| 导入 | 主机单文件最多 1 MiB、批量最多 256 条；设备分块流式写入，不一次装入整个备份 |
| Notes | 单条最多 16 KiB 的主机格式上限；设备实际容量另测，不承诺无限存储 |
| 桌面 | 无工作时事件驱动刷新；USB 枚举后台线程；正式数据传输也须后台、可取消 |
| 密码 | RustCrypto 测试参照，成品使用经验证硬件实现；不从零手写 ECC/AES/HMAC |

## PCB / EMC / 侧信道

四层板起步，安全数字区域连续参考地、近端去耦、短回流；USB 低电容 ESD 保护，A/C 真双口数据互斥与电源防反灌。屏幕/电池/壳体参与最终 NFC 调谐，线圈区按 RF keepout，不允许整板铺铜或全金属壳覆盖线圈。

EMC 抗干扰与主动故障注入分别验证。干扰时安全底线是保持锁定或中止操作，绝不误授权、误签名、复活旧权限或回退重试次数。滤波/屏蔽/随机延时不是抗侧信道证明。

参考：[ST EMC](https://www.st.com/resource/en/application_note/an1709-emc-design-guide-for-st-microcontrollers-stmicroelectronics.pdf)、[NFC 天线](https://www.st.com/resource/en/application_note/an2972-how-to-design-an-antenna-for-dynamic-nfc-tags-stmicroelectronics.pdf)、[U585 安全指南](https://www.st.com/resource/en/user_manual/um2852-stm32u585xx-security-guidance-for-psa-certified-level-3-with-sesip-profile-stmicroelectronics.pdf)。

认证/安全公告按精确版本处理，例如 [TROPIC01 2026-06 LFI 公告](https://www.tropicsquare.com/news-and-events/tropic01-security-advisory-lfi-vulnerability-disclosure-and-mitigation) 表明开放安全芯片也须按芯片修订与缓解措施评估，不能无条件用作强物理防护根。
