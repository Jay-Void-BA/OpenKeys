# 经典版 FIDO：设备 PIN 与主机协议 PIN

[当前状态](DEVELOPMENT_STATUS.md) · [内核](../crates/authenticator-core/src/processor.rs) · [互通 / 撤权测试](../crates/authenticator-core/src/protocol_pin_tests.rs)

本轮修正候选内核中仍沿用的 R04 指纹入口，并恢复独立的 FIDO clientPIN 路径。
**这还不是已接通 USB / NFC 的生产认证固件。** H573 Env、可信启动、原生 SE PIN、
真实传输和全部生命周期中断仍须集成；模型中的成功不能当作实片验证。

## 两层验证分别成立

1. 六位设备 PIN 只由钥匙十键输入。经典版 OpenSK 使用独立的
   `openkeys-local-pin` / `Env::LocalPin` 接口，不编译指纹认证或录入功能。
   Root 必须检查当前精确请求、原生 PIN 会话、通道、代际、时间和硬件状态。
   内核在每次私密执行前后调用此接口，主机 token 不能跳过它。
2. 若另设了主机 FIDO PIN，注册 / 登录还必须带有效的主机 PIN token / MAC，
   并由 OpenSK 检查权限、RP 和协议版本。仅通过设备 PIN 不够。
   若尚未设置主机 PIN，可在设备 PIN 授权后取得本地 UV token。
3. PIN 设置状态是公开能力标志；它不公开 PIN 哈希、账户、凭据容量或标题。
   GetInfo 不先读取私密目录再过滤输出。真实 Root 的公开标志必须与提交的
   协议 PIN 状态一致，并在损坏 / 掉电后安全恢复。

| clientPIN 子命令 | 经典版策略 |
|---|---|
| 2：密钥协商 | 公开 ECDH 公钥；不是用户授权，不续 PIN 租约 |
| 1：重试信息 | 先设备 PIN 与本地确认 |
| 3 / 4：设置 / 修改主机 FIDO PIN | 先设备 PIN；协议自身加密 / 验证；不会改变设备 PIN 或清空所有用户数据 |
| 5 / 9：主机 PIN token | 先设备 PIN，再真正验证主机 PIN；权限限本版支持的注册、登录、凭据管理 |
| 6：本地 UV token | 先设备 PIN；只有未设置独立主机 PIN 时允许；已设置时返回 PUAT_REQUIRED |
| 7 / 8 及未知命令 / 权限 | 拒绝；不开放指纹、large blob 或配置旁路 |

指纹管理命令 `0x09` / `0x40` 拒绝。GetInfo 不广告指纹录入，UV modality
使用内部 PIN；主机 PIN 已设置时引导平台使用独立主机 PIN 路径。
标准 U2F 没有主机协议 PIN，仍须单独通过设备 PIN 与确认，不冒充 CTAP2 PIN。

## 撤权、重放和缓存

`Processor::lock()` 先要求 Root 同步撤销原生 PIN、全部租约、显示和输入，
再清断言游标、token / 权限，重新生成 ECDH 密钥。USB / NFC 物理场次或通道
变化会触发撤权；不能把公开 CID 当成经过认证的对端身份。

token 清理不重建整个 ClientPin 对象，因此不会返还本次通电中的连续错误预算。
设置 / 修改主机 PIN，以及验证失败，都会失效此前 token；设置主机 PIN 前取得的
本地 UV token 也不能在设置后绕过新 PIN。捕获的旧加密 PIN 在锁定后的新 ECDH
上下文中无效。硬件 PIN 的 11 次持久限次属于另一层，不能被这些操作重置。

[配对管理会话的 30 秒租约](PIN_SESSIONS_R05.md)与 CTAP token 规则分别检查。
确认请求、公开查询和保活本身均不续期；仅成功完成的受保护操作可以续期。
CTAP 会消费 token 权限，管理租约不能悄悄恢复这些标志；非零 CTAP 状态 / CCID
传输成功也不能被 Root 当作私密操作成功。标准 FIDO 没有管理器的 Noise 配对证明，
不能继承管理器缓存。真实定时器、RF 离场、USB suspend/reset 和 Root 回调尚未接通。

## 可复核证据与剩余边界

测试用独立客户端流程验证 PIN 协议 v1 / v2 的 ECDH、KDF、CBC、MAC 和 token；
覆盖设备 PIN 缺失、主机 PIN 缺失 / 错误、修改后的旧凭证 / token、锁定后的
密文重放、连续错误预算、通道切换，以及公开信息的私密读取边界。
软件密码和固定 PIN 仅存在于 `cfg(test)`，不作为 H573 私钥后端。
客户端消息编排独立于服务端流程，但使用同一组密码原语依赖；这不能代替不同
实现之间的互通测试、浏览器测试或 FIDO 一致性测试。

OpenSK 修改记录和可重放补丁见 [来源说明](../vendor/opensk/README.md)。
尚需完整 Root / SE ACL / 可信时钟 / 原生传输、FIDO 一致性及平台真机测试，
以及实物故障注入和侧信道评估。此修订不等于完整认证器、FIDO 认证或漏洞清零。
