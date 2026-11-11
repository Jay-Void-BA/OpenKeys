# R05 OTP 导入、删除与显示边界

[当前状态](DEVELOPMENT_STATUS.md) · [设备格式](../crates/otp-format/src/lib.rs) · [URI 导入](../crates/vault-import/src/otp.rs) · [SE 操作](../crates/se-state/src/otp.rs)

已实现候选软件组件，**GPUI、Root 目录与真实设备尚未接通**。不能据此宣称现在可用
管理器导入、删除或使用三把实物钥匙上的 OTP。

当前受支持输入：`otpauth://totp/...` / `otpauth://hotp/...`；6 位、30 秒 TOTP；
HMAC-SHA1 / SHA256 / SHA512；16–64 字节种子；最长 80 字节的 ASCII issuer / account。
非 6 位、非 30 秒、短种子、冲突 issuer、重复参数、未知参数、非法百分号编码、
异常 Base32 填充位、非法 HOTP counter 都明确拒绝，不自动改写。
输入总长上限 2 KiB；标准带 / 不带 padding 的 Base32 可导入。

设备独立重解析 v1 二进制格式，校验全部长度和尾部，重新计算绑定账户、算法、
模式、完整种子与初始 counter 的导入摘要。SE 请求再绑定槽位、代际与认证通道。
复制 / 修改管理器的预览标签不能替代设备的独立检查与 LCD 确认。

`OtpKeys::install` 只接受原生 PIN 接口和已激活执行作用域：先确认种子 / counter
对象均明确不存在，再创建持久 counter 并读回，最后创建不可导出 HMAC 对象。
对象策略只授权硬件 PIN 原生会话；HMAC 对象不授予 read / export / update。
任何不确定应答停止，不自动覆盖、重试创建或生成成功结果。验证只读类型 / 长度 /
counter，没有读取种子。

`delete` 必须由 Root 先持久写下账户 tombstone、隐藏目录并撤销该代际的所有待执行
操作，再先删种子、后删 counter。只在认证通道明确证明对象已不存在时跳过删除；
通信错误不能算不存在。断电后保留 tombstone，需重新进行本地授权并恢复清理。

尚须接通的软件：密封目录 / 代际分配、pending / tombstone 跨介质事务、重启恢复、
受保护 RTC / 设置时间、最终作用域检查、管理器选择导入与相应应答、生产显示适配。
目录不能在 SE 导入确认和最终安全检查之前发布；未完成条目不能列为可用。
未完成的导入可能留下孤立对象，只能在有权的恢复流程中清理。

已安装种子没有读回接口。管理协议不提供验证码、原始 HMAC、calculate-all 或键盘
输出；六位验证码只能由受保护显示流程放到实体 LCD。主机端解析期间仍会短暂
持有导入材料，清零是尽力措施，不能抵挡已经控制操作系统的攻击者。
