# R05 原生 UserID 会话：样板验证模块


当前经典版已改选 SE051C2 / OEF A8FA / Applet 7.2；本页旧 E2 / 通用 SE05x API 研究不能视为新实片资格证明。见[选型与待验项目](CLASSICAL_SE_POWER_R05.md)。
[原生 PIN 候选评估](SE_NATIVE_PIN_FEASIBILITY_R05.md) · [源码](../crates/se-state/src/native_uid.rs) · [测试](../crates/se-state/src/native_uid_tests.rs)

新增 `native-uid-qualification` 功能，**默认关闭**。它仅创建临时会话、提交一次六位
UserID 验证并关闭 / 断电，用于合成对象的字节级与实片验证。没有创建 / 覆盖对象、
任意 APDU 转发、私钥操作或验证码 / notes 接口；不实现 `ProtectedPin`、`Exchange`
或 Gate 授权转换，因此成功返回不是生产解锁凭据。

当前 R05 电气诊断仍关闭 SE，未启用此功能。完整生产 Root、原生对象 ACL、持久
尝试 / 全清空和硬件验证仍未完成；不得把此模块作为三把成品的认证固件。

另有[原生 PIN v3 后端](NATIVE_PIN_RUNTIME_R05.md)，使用独立生产候选 ID
`0x4F4B0801`；该后端仍未接通生产 Root。不要把本页的合成对象 / 成功状态混入它的权限。

## 软件检查

```sh
cargo +1.99.1 test -p openkeys-se-state --features native-uid-qualification native_uid:: --locked
cargo +1.99.1 clippy -p openkeys-se-state --features native-uid-qualification --all-targets --locked -- -D warnings
cargo +1.99.1 check -p openkeys-se-state --features native-uid-qualification --target thumbv8m.main-none-eabihf --locked
```

六项测试覆盖独立命令字节、畸形 / 截断 / 额外响应、零 SID、错误与不确定应答、
非法 / 重复输入、关闭应答失败、物理断电接口报错及未完成对象的销毁。
它们验证模型与编码，不证明真实芯片接受命令或实际电源已经切断。

## 已固定的协议范围

| 项目 | 候选编码 |
|---|---|
| 合成认证对象 | 固定 `0x4F4B0800`，不允许调用方选择厂商保留管理 ID |
| 本地输入 | 六个 0–9 数值，转为六字节 ASCII；无默认 PIN |
| 创建会话 | `80 04 00 1B`，TLV `41 04` 加大端对象 ID |
| 会话应答 | 仅接受 `41 08` 加八字节非零 SID；SID 不公开、不实现 Debug / Clone |
| 验证 / 关闭 | `80 05 00 00` PROCESS，`10 08` SID 与 `41` 内嵌管理 APDU；只支持固定的 VerifySessionUserID / CloseSession |
| 保护 | 仅接受已密封的 Platform SCP03 `Exchange`；本模块不提供明文传输或降级 |
| 结束 | 关闭后物理禁用 SE；错误 / 丢应答不重试 APDU，销毁时不做阻塞 I2C 清理 |

`verify_local_pin(&[u8; 6])` 的类型不能证明输入来自实体键盘。最终 Root 必须独占 GPIO、
内存和 DMA，并从可信输入流程调用；管理器传来的六个数字不能作为替代。
`PowerCut` 也只是平台契约，返回成功不能代替 VDD / EN / 复位波形测量。

字节依据为 NXP 固定提交
`7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2` 的 `se05x_tlv.c::se05x_Transform`
及会话接口，见[来源清单](../references-r05.lock.json)。没有编译或复制 NXP C 实现。
常规命令按该 SDK 的无外层 Le 形式编码；实际 E2 Applet 的互通性仍须确认。

## 交给工程师 / 实验室的样板任务

1. 使用独立工程样板和合成秘密，记录 SE050E2 完整料号、批次、Applet / profile、
   板 / 固件摘要和 SCP03 配置。禁止导入真实账户、恢复码或生产私钥。
2. 在受控初始化过程中创建合成 UserID 对象：ID `0x4F4B0800`，六字节 ASCII PIN，
   显式认证对象标记、最大尝试数 **11**、明确审查过的对象策略。
   不采用示例的 NULL 策略或最大次数 0；模块本身不会创建或修正该对象。
3. 提供真实 `ScpSession` 与 Root 独占的 SE 禁用引脚；每次探针只提交一次完整猜测。
   外层要求完整加密和完整性，不能把 Applet UserID 会话当成主机管理加密。
4. 记录正确 / 错误结果；跨创建、验证、关闭的每个 APDU 边界断电或丢应答。
   不得自动重试不确定验证；失败后测量实际 SE 禁用，重新建立 Platform SCP03。
5. 分别核对十次错误后第十一次正确，以及第十一次错误；跨掉电 / 重建会话仍限次，
   正确比较后的计数语义按实际 Applet 记录，不能仅根据 API 名称推断。
   AN12413 §3.2.4.4 说明 UserID 通用 counter 保持 0、不报告失败次数；请直接测试
   原生拒绝行为，不能把该属性当作失败次数读回。ReadSize 也不能读取 UserID 长度。
6. 用合成私钥 / 包装密钥验证 ACL：普通 SCP03、未验证 / 错 UserID、错会话均不能用钥匙；
   认证后仅允许预期用途。检查 PIN 对象更新、删除重建、同 AuthID 复用、管理员旁路及
   半完成 DeleteAll，不能保留旧秘密并跳过旧 PIN。
7. 测 USB 重置 / 挂起、NFC 离场 / 重入、手动锁定与空闲过期时的硬件撤权。
   本探针没有 30 秒租约，相关结果必须由最终 Root / Gate 集成后提供，不能拿模型代替。

报告必须区分“收到成功状态”“软件请求关闭”“测量证实断电”“原生 ACL 强制成功”。
原生次数限制不等于自动删除所有用户对象；现有擦除锁存也不等于全清空完成。
所有生产阻断项关闭前，状态保持 BLOCKED。
