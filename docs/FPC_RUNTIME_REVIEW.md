# R04 指纹运行时通信核查

当前实现是已配对设备的候选组件，**不是已投入运行的完整指纹认证器**。
真实配对、模板配置、Secure引导与dispatcher、芯片时序和物理攻击资格尚未完成。

## 原始依据与范围

- FPC `SPC27317 / 6`，2025-04-16，§3.4、§4.2：v4小端帧；CS拉低至首个时钟必须大于500µs，发送帧期间CS连续保持；IRQ高表示有待读数据。PDF SHA256 `6a90b159e61626ba4a6581f11e1f211c6db69e4bf92437d7f5af3789aed1d022`。
- FPC `SPC27387 / 3`，2026-01-28，§2：AES128/256-GCM、12字节随机IV、16字节tag，帧头为AAD；出厂安全通信未开启。PDF SHA256 `73330b63ebaa315a00a97016024c8ab1de958e8e489697d5c0eb2bf644e246d6`。厂商PDF未随项目重分发。
- [SparkFun参考](https://github.com/sparkfun/SparkFun_FPC2534_Arduino_Library/tree/5d0d1727ea05e79ad3c2e3c84fb933dba37f2723)：MIT，`fpc_api.h`及SPI时序作交叉参考；未编译其C/C++。其参考采用600µs CS唤醒延迟。本项目Rust实现采用相同工程余量，1MHz而非模块10MHz上限，需实板测边沿/EMI。

## 实现的调用链

`device_policy::biometric::Session::begin_encrypted`先调用真实持久适配预扣重试/tag，
重新读取设备时间，构造指定模板的10字节Identify命令，再由受保护AEAD生成IV、
密文和tag，最后再次检查期限。存储应答不确定、熵/密码故障、时钟回退或过期均
不返回可发送的`EncryptedIdentify`；已预扣tag不退款。

目标`HardwareFingerprintGcm`使用同一独占共享SAES/RNG服务和配对的256位key。
发出46字节报文，接收48字节Status/Identify帧。无测试key、公开UID派生、软件AES
或外部传入IV的生产回退。可信root仍须只提供唯一且已验证的配对key。
类型不能阻止已攻陷的特权固件安装恶意trait实现，也不能当作主机/NSC输入证明。

板级`fingerprint::Transport::send_identify`消费该报文，发送前后显式调用
`Guard::check_request`核对当前请求/连接，并检查设备期限。底层`write`只检查帧形状，
不能从密文判断命令ID或证明MAC；它不是任意主机指令接口。收到的
`UntrustedFrame`必须送回Session做GCM、类型、template/tag/request及持久credit检查。
返回的`VerifiedRequest`仍须经过追加PIN策略、LCD完整复核及单次grant消费。

## 板级时序与错误处理

R04固定连接：SPI3 PC10 SCK / PC12 MOSI / PC11 MISO，PA15 CS，PC6 IRQ，
PC7 active-low reset；Mode0/MSB，1MHz，低GPIO驱动速度。新建端口先保持reset，
显式release在实时guard允许后保持10ms再释放；10ms是工程选择，不是实测最小值。
每次SPI事务在CS低期间等待600µs，再完成整帧，并flush/取消片选。

IRQ空闲一次poll即返回，不忙等、不喂狗、不延长授权。每条发送后的接收上限32帧；
不自动重发有歧义的SPI命令。通信、引脚、格式、期限、时间回退或guard错误均
锁住实例并尝试拉低传感器reset；析构也拉低reset。reset引脚本身失败时只报告失败，
不能宣称芯片已实际复位。root必须另外撤销所有grant、清LCD并处理硬件fault。

100ms I/O预算只在阻塞调用前后检查。Embassy0.6.0 SPI内有等待硬件标志的循环，
因此**不能声称这能抢占卡死的HAL**：独立硬件watchdog必须已开启，不能在这个
等待中喂狗；其时钟、复位后LCD/SE安全状态及FI下有效性需实测。

## 自查结果与未满足条件

1. 使用真实`ExclusiveDevice`配合模型SPI总线核对CS、delay、flush及整帧顺序；
   测试半次传输错误、通信后撤销、过期/错请求、超时、时钟回退、IRQ/reset故障、
   坏头/明文和超量帧。有效报文往返另用独立Python cryptography 50.0.0向量核对；
   固定key/IV只在测试中存在，不能用于量产。
2. tag只防结果重放，**不保证GCM IV唯一**。硬件RNG连续健康检查不排除一切
   复位、降压、时钟或EM故障造成的长周期/重复IV。双方共享key；传感器生成的
   随机IV也必须资格验证，主控缓存最近IV不能覆盖攻击者截获但从未送达的报文。
3. HAL引脚类型只证明引脚匹配，不证明TrustZone归属。所用PAC的SPI3
   为`0x40003c00`、GPIOA/C为`0x42020000/0x42020800`（非安全别名范围）。
   实际事务属性取决于IDAU/SAU与执行状态，且还须GTZC、GPIO、RCC、中断及DMA
   保护。不能把整段非安全别名范围交给NS世界后仍假定这些HAL调用安全。
   Embassy0.6.0的`trustzone-secure`功能只出现在U5 flash路径，不会自动替H573
   重映射这些端口。真实root必须证明所用地址产生正确安全事务，或改用受审查的
   Secure别名驱动；未验证前禁止私密服务。
4. 配对、禁用危险维护/模板导出/工厂重置的策略、密钥封装、首次入组、取消/Abort、
   root状态机、闭合调试及通信与授权组合仍须实现/部署和验证。SPI短报文I/O和
   `begin_encrypted`不能替代这些工作。SE对象ACL与8次失败后的维护恢复仍未合格。

`firmware/dist`仍是OPEN/TZEN=0诊断，指纹/SE/充电/私密操作均关闭。上述源码
组件不会被偷偷启用到这个镜像中，不可将它当作三把正式钥匙的认证固件。
