# R05 初始化、销毁检查与私钥端口

[状态](DEVELOPMENT_STATUS.md) · [原生 PIN](NATIVE_PIN_RUNTIME_R05.md) · [制作 / 实测](../manufacturing/BUILD_R05_ZH.md)

本页说明新增的实际 SE 命令、Flash 擦除组件和执行检查。**整机生产 Root、受保护 journal、
完整 DeleteAll / 恢复协调器、应用初始化及 USB / NFC 实机接线仍未完成。** 这些组件不能
将设备标为全清空、正式可用或理论上安全。

## 本地 PIN 安装

[`commissioning::Installer`](../crates/se-state/src/commissioning.rs)实现六位本地设置策略的
`ProtectedCommissioning` 后端。只能由受保护 Root 使用，不提供主机 PIN 设置端点。

1. Root 确认封闭启动、精确 C2 身份 / profile、独占硬件与明确的 Blank 生命周期。
   缺失状态、缺少 marker、掉电丢失备份域都不能推断 Blank。
2. 完整认证对象清点与已独立确认的工厂基线一致。基线不能从一次不确定重置后的
   所见对象现学，也不能由管理器传入。
3. 首先持久写入并读回 Installing，再允许第一条 SE 写命令。不确定应答维持非空白，
   同一安装器关闭，不能覆盖对象、补写后假定可用或恢复默认 PIN。
4. 固定 APDU 显式使用 `INS_AUTH_OBJECT`、六字节 ASCII 和 `maxAttempts=11`，禁止
   PIN 对象的读出、更新和删除重建；credit 仅 PIN AuthID 可写，其他用户只能读。
5. 三个计数器分别创建后读取，必须为零。不可写 / 删的 marker 最后创建。
6. 经现有 `NativePin` 后端重新读取类型 / 大小 / marker，预扣一次并验证刚设置的 PIN，
   确认 credit，明确撤权并请求物理断电后才提交 PIN 命名空间这一阶段。

PIN 这一阶段不是全设备 Ready。SCP03 唯一配对、vault / Noise 根、应用与目录、OTA /
回滚与总生命周期仍需生产 Root 完成并最终密封；这里没有 Root journal 的生产适配器。
编码并读回类型不证明真实 ACL、最大尝试数和 NVM 语义，确切实片仍须资格验证。

## SE 全对象清点

[`inventory::verify_baseline`](../crates/se-state/src/inventory.rs)只接受密封的认证 Exchange。
固定全类型过滤，累计对象数量作为下一页 offset，完整解析规范 BER；缺失、额外、
重复、畸形、零进度和 I/O 不确定都关闭交换。逐个核对持久类型；最多64个基线对象，
明确拒绝把 OpenKeys 私有命名空间列入工厂例外。

NXP 固定 SDK `7102413e0457d3dfa4fc4be686ca8ec50bd9f5e2` 的
`SE05x_MoreIndicator_t` 及 AN12413 §4.3.30 规定 `NO_MORE=01`、`MORE=02`，`00` 非法。
不能照搬同一手册 ReadIDList 示例中不一致的0/1叙述，导致少清点一页。

返回的 Report 只是清点观察，不是销毁或初始化能力。它不验证对象 Origin / ACL、
预置公钥身份、完整 DeleteAll、NOR / Root 数据或合法工厂生命周期。Root 必须在
重置后新建认证通道，按此前独立确认的同芯片基线核对；不能放宽普通 SCP03 认证来接受裸 `9000`。

## 全部外部 Flash 擦除

[`flash_erase::Eraser`](../firmware/board/src/flash_erase.rs)消耗唯一、已识别的 W25Q64 所有者。
先逐扇区擦除并读回整个8MiB，再独立逐页读取全部8MiB。未完成、读回异常、应答丢失或
Root 检查失败都不给 ErasedFlash；成功凭据独占同一 Flash 所有者，不可复制。

一次 step 最多一个扇区或一页；没有主机范围、跳过已擦区域或可注入的恢复 offset。
重启后从零重做。Root 必须在此前持久锁存清空意图，先撤销所有会话、清显示和敏感 RAM，
保持正常入口关闭，在步骤间服务看门狗。整个清空可超过30秒，不属于授权续期。
真实最坏扇区时长、看门狗下限和掉电仍需测量。

ErasedFlash 只覆盖该 NOR 的命令 / 读回路径，不证明 SE 对象、MCU 根密钥、备份域、
协议 PIN、配对或持久 journal 已销毁。外部探针、Flash 替换和总线欺骗属于实物评估。

## 每次私钥命令的执行约束

`NativePin` 本身不实现 `PinBoundExchange`；编译失败用例防止重新把它直接传给 Keys。
只有借出的 `PrivatePort` 实现该接口，每条原生命令前后检查同一个 Execution。
不接受 FactoryReset / ChangePin / FirmwareUpdate / UnlockSession 作为通用私钥权限，
也不允许套娃会话或重置命令进入普通私钥端口。

Execution 的时间和撤权检查由同一个不可复制对象保存，通过 Cell 支持只读授权元数据
与活动检查共同借用；类型不支持跨线程共享。重入或回调取消时，外层检查同样失败，
不能因回调返回 Ok 恢复权限。正常释放端口不续期；故障会清输出、取消执行、关闭 SE。

Root 的 State 仍必须接真实时钟、USB / NFC 生命周期、两个 PIN 租约及硬件故障。
私钥端口额外强制要求没有默认实现的 `PrivateState::check_private_request`，核对同一
对端 / 物理场次、硬件 PIN 租约和独立的协议角色 / token / RP / 槽权限；普通启动或
全局硬件状态检查器不能直接替代。该回调前后也检查 Execution，防止回调期间过期或撤权。
接口和 Rust 类型不能抵挡恶意特权固件，也不能凭此证明时钟、SAU / GTZC / DMA、
调试锁定或侧信道安全。所有输出 / 提交之后的整体执行结束检查也仍然需要。

## 软件验证及实片边界

固定转录测试核对有限尝试、策略字节、marker 顺序和原生自测；在每个 APDU 前后模拟
应答丢失，安装中状态不得回到 Blank。另测 journal 应答不确定、供电关闭失败、旧对象、
非可信启动；全 Flash 测试覆盖两 bank、早期扇区后续损坏和不完整 / 故障结束。
执行测试覆盖命令前 / 后过期、代际变化、故障、取消、重复调用及回调重入。

这些是合成设备模型和编码 / 状态检查。真正的 Root journal、销毁协调器与启动部署
未接通，实物测试为 NOT_RUN；不得将此页当成生产验收或攻击免疫证明。
