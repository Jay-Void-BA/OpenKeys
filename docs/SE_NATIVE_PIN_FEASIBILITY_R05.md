# SE 原生 PIN 强制：候选机制与验收条件

[当前复查](SECURITY_REVIEW_R05.md) · [会话规则](PIN_SESSIONS_R05.md)

**尚未实现或通过实片验证。** 当前 `pin_only` 后端用 SE 内 HKDF、持久计数和 MCU
常数时间比较，仍依赖可信 Root。SE_EN 不是 PIN 验证器。本页记录可进一步核实的
硬件授权机制，不能作为“已经硬件强制”的证明。

## 已核对的原厂接口事实

来源：[NXP plug-and-trust](https://github.com/NXP/plug-and-trust/tree/7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2)，
固定提交 `7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2`，
`hostlib/hostLib/se05x_03_xx_xx/se05x_APDU_apis.h` 中的接口说明：

- `WriteUserID` 创建 4–16 字节的 UserID 对象，因此六个数字的编码长度在范围内。
- 创建认证对象时可指定 `TAG_MAX_ATTEMPTS`；PIN 的最大尝试值必须小于 256。
  **省略或填 0 表示无限次数**，不能用于本项目。该字段只在首次创建且设置
  `INS_AUTH_OBJECT` 时适用，不能假设后续普通写入会改变限次策略。
- `CreateSession` 引用四字节认证对象 ID，返回八字节临时会话标识；
  `VerifySessionUserID` 验证成功才完成该类会话建立，错误则不建立。
- `CloseSession` 关闭后不能重开该会话，参数为临时状态。

这是通用 SE05x API 的描述。仍须确认 **实际采购的 SE050E2 料号、Applet / profile、
对象策略、SCP 组合、尝试计数及掉电行为**。有函数声明不代表选定器件和配置已经满足要求。
没有复制或编译 NXP C 实现。

## 不能只增加一次 Verify 调用

1. FIDO / OpenPGP / PIV 私钥、OTP HMAC 与 vault 解密 / 包装密钥，必须由 SE 对象 ACL
   限制为正确的本地 PIN 认证会话。仅在 MCU 设置 `unlocked=true`，或用 SCP03
   验证 MCU 身份后直接允许全部密码操作，仍然能够绕过本地 PIN。
2. 必须排除运行期管理员凭证的旁路授权。尤其不能允许删除 / 重写 PIN 对象后复用
   同一 AuthID 访问旧私钥；这会违反“改硬件 PIN 清空全部数据”。对象 ID 的复用、
   PIN 对象生命周期与所有私钥生命周期需要共同审查。
3. UserID / PIN 不可在未保护的 I2C 上传输。Platform SCP03、Applet SCP03 和 UserID
   会话是不同概念；必须验证实际组合和全程加密 / 完整性，不能根据接口名称推断。
4. SE 临时会话标识只留在受保护 Root 内部，不导出到管理器、日志或任意 APDU
   转发接口。设备配对通道的对端 / transcript / RF 场次绑定仍需独立完成。
5. 原生限次通常只说明认证对象会被限制，**不等于第 11 次失败自动擦除其他对象**。
   完整清空、掉电恢复、Flash 密钥 / 目录 / staging、协议 PIN 和配对必须另行实现。
   锁住 PIN 后如何执行不可读取旧秘密的全清空，也必须验证。
6. 原生会话不能被假设自带 30 秒空闲期限。可信计时、断连 / 离场撤权、SE 关闭 /
   断电和故障响应必须实测；不能把周期保活或 `RefreshSession` 当成用户有效活动。
   如果要求在 Root 已被攻陷时仍保证期限，需要额外硬件机制或更换能强制该策略的器件。

## 最小实片负向测试

使用合成密钥逐项测试：未验证 / 错 PIN / 错 AuthID / 错协议角色时签名、解密、
HKDF、OTP 均失败；跨每个 APDU 边界断电不能返还次数；十次错误后第十一次正确
与第十一次错误分别核对；达到限制后复位 / 重建会话 / 换接口不能恢复旧权限。

尝试普通 MCU SCP 会话、工厂命令、更新模式、对象删除重建、旧会话 ID 重放和
管理员权限替换，均不能保留旧秘密并跳过本地 PIN。随后验证改 PIN 全清空、
半擦除重启、NFC 快速重入、USB 重置 / 挂起、30 秒保活、短电压 / 时钟 / EM 故障。

必须记录器件批次、Applet/profile、对象策略字节、固件提交及测量证据。
在这些条件关闭前，当前 R05 继续维持生产放行 BLOCKED。
