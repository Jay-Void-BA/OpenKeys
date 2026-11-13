# R05 原生 PIN 后端与私钥接口

[状态](DEVELOPMENT_STATUS.md) · [源码](../crates/se-state/src/native_pin.rs) · [测试](../crates/se-state/src/native_pin_tests.rs) · [原厂约束](SE_NATIVE_PIN_FEASIBILITY_R05.md)

已增加真正的 `ProtectedPin<6>` 后端 `NativePin`，以及密封的 `PinBoundExchange`。
这比[样板探针](NATIVE_UID_QUALIFICATION_R05.md)多了持久尝试账本、原生验证后的
credit、私钥 APDU 包装与故障撤权。**仍不是已接通 / 已验证的生产固件。**
生产启动、Root、首次初始化、完整销毁与物理 SE profile 尚未完成。

## 授权链

1. 原生后端只打开明确初始化的 v4 标记与已有对象。缺标记、旧 HKDF v2 标记、
   错类型、非持久对象或擦除待处理均拒绝，不能自动创建 PIN / 回退旧后端。
2. `PinEntry` 从独占 GPIO 收集六个数字；未完成 / 取消不调用预扣。
   完整提交先持久增加 spent 并读回，再为同一请求创建 UserID 会话。
3. 原生 `VerifySessionUserID` 成功后，才在该原生会话内写 credit 并读回。
   credit 的写权限必须只授予 PIN AuthID；普通 Platform SCP 只能读 credit。
4. 签名密钥与 OTP 后端的泛型要求 `PinBoundExchange`。仅有已验证的 `NativePin`
   不再满足该类型：必须用已经消耗的 `Execution`、Root 状态与可信时钟借出
   `private_port`，每个 APDU 前后重新检查同一个请求、期限、代际和硬件。
   过期、撤权、重入或应答后故障会清输出、取消执行并关闭 / 断电原生会话。
   普通 `ScpSession`、旧 HKDF `PinState` 和启动阶段的 SCP guard 不能代替此端口。
5. 私钥命令包在原生 SID 内，再由完整 Platform SCP03 保护。新签名密钥策略
   只授权 `0x4F4B0801`，其他用户明确 `FORBID_ALL`；不再给 AuthID 0 签名权。
6. 正常释放借用的操作后端允许保留原生会话。Root 必须继续按真实配对 / 请求 / 时间 /
   协议角色检查每次 I/O；30 秒过期、离场、重选、重置、手动锁定或故障时调用
   `revoke`，清私密显示、输入和所有协议状态。普通连接 / 保活不续期。

后端不发送 `RefreshSession`；错误 / 不确定应答不重试原命令，关闭 SCP 并请求物理断电。
`PowerCut` 返回值不能代替 EN / VDD 波形验证。原生 SID 和 PIN APDU 使用清零缓冲，
没有 Debug / Clone / 序列化或主机 APDU 转发接口。

## 初始化必须同时满足的事实

| 对象 | 约束 |
|---|---|
| PIN `0x4F4B0801` | 六字节 ASCII；明确认证对象、原生最大尝试 11；禁止更新、删除重建与管理旁路 |
| spent `0x4F4B0501` | 8 字节单调持久 counter；预扣后读回，不能减少 |
| credit `0x4F4B0502` | 8 字节单调 counter；普通 SCP 可读，只有已验证 PIN 会话可写 |
| erase `0x4F4B0507` | 8 字节单调锁存；非零永久拒绝正常访问 |
| owner `0x4F4B0802` | 32 字节随机用户代际；不可修改 / 删除；必须与受认证 Root journal 一致，不能读取所见值后自认正确 |
| marker `0x4F4B0506` | v4 固定布局摘要；最后安装，只读且不能运行时删除 / 重建 |
| 私钥、HMAC / vault 包装根 | 全部明确绑定正确 PIN 原生会话，不得保留其他用户可用的旧对象 |

源码的 `marker()` 只是已审查配置的摘要，**不是 SE 真实 ACL / maxAttempts 的远程证明**。
已增加[PIN 命名空间安装器](../crates/se-state/src/commissioning.rs)：实际写有限次、不可替换
UserID，创建并核对计数器，最后写 marker，再做原生验证并请求断电。它依赖尚未接通的
生产 Root / 受保护 journal / 明确空白生命周期，不是完整设备的首次初始化器。
不能通过手工写一段 marker 就宣称 ACL 已合格。见[安装 / 销毁组件边界](CLASSICAL_LIFECYCLE_R05.md)。
旧 HKDF v2 与原生 v3 布局没有自动升级；转换必须先完成全部旧秘密销毁并验证，再初始化新代际。

v4 的 `NativePin::open` 必须接收受认证 Root journal 的预期 owner nonce，在任何 PIN
猜测前比较当前 SE 的不可改写 owner 对象。每次私钥端口还将同一 nonce 交给
`PrivateState` 复查。整份旧 MCU 状态重放即使 MAC 正确，也不得与新 SE 用户代际
混用。新[Root journal 与安装适配](../crates/root-journal/README.md)已加入；真实安全
Flash / HUK / 启动适配及全数据销毁协调器尚未接通。不能将静态 marker 当作代际证明。

AN12413 Rev2.12 §3.2.4.4 明确：UserID 失败次数不通过通用对象 counter 属性报告，
该值保持 0；§3.2.3 表2 / §4.7.1.5 表98描述有限尝试配置。不能读到 0 就判断
从未猜过或可重置。`ReadSize(UserID)` 也不返回 PIN 长度，因此后端不调用它。
当前 C2 / Applet 7.2 的原生限次、正确后的重置和跨断电语义须直接用合成对象做负向测试。

## 失败与销毁边界

十次错误后第十一次正确可以恢复 credit；第十一次错误 / 结果不确定且未获 credit，
下次受控打开会锁存 erase。原生后端不退还已执行的预扣。
这证明的只是正常访问闭锁；**没有执行全数据销毁**。
完整 Root 必须立即进入独立擦除恢复流程，处理 SE 所有用户秘密、Flash 包装密钥、
目录、staging、协议 PIN 与配对；不能把 `EraseRequired` 转成“已安全空白”。

独立模型测试覆盖每个初始化 / 预扣 / 验证 / credit APDU 的前后断点，以及原生私钥
拒绝、错请求 / marker / SID、错误次数、借用释放与物理撤权请求失败。
这些测试不证明真实 Applet、MCU 隔离、编译器或物理抗攻击性质。

## 长命令边界

原生包装已支持 BER 81 / 82 和扩展 Lc，内层数据最多 1994 字节；外层仍完整 SCP03 认证。
[边界、独立转录与测试](CLASSICAL_APDU_R05.md)不代表 RSA / OpenPGP / PIV 或生产 Root 已完成，
也不代表实际 C2 的命令容量已验证。溢出或应答超出指定 Le 会关闭并撤销原生会话。
