# R04：SE会话、SAES和RNG核查

日期2026-11-09。此页记录实际源码修正及可重复检查，不是整机认证。

## 使用的原始资料

| 资料 | 版本 / PDF SHA256 |
|---|---|
| [RM0481](https://www.st.com/resource/en/reference_manual/dm00733993.pdf) | Rev5，2026-07；`0001ab2a6ed8d31553cf6bbdf732fde16545ab5b3a7147fc8835432c3707657a` |
| [AN4230](https://www.st.com/resource/en/application_note/an4230-introduction-to-random-number-generation-validation-using-the-nist-statistical-test-suite-for-stm32-mcus-and-mpus-stmicroelectronics.pdf) | Rev15，2026-10；`944ccbf86841982b170b7cd2fb2bedbc165ea278d3d8d132f8a4721339e489e8` |
| [UM3125](https://www.st.com.cn/resource/en/user_manual/um3125-stm32h573xx-security-guidance-for-sesip-3-certification-stmicroelectronics.pdf) | Rev2，2023-06；`367005ac52491b1deee81004c404a11312b6afe870543e2308fc2d35c409ed9f` |
| [NXP nano-package](https://github.com/NXPPlugNTrust/nano-package/tree/26d55b43225bc84de364057660cba676fa69f6e4) | 提交锁在references.lock.tsv；只作T=1/SCP03协议参考，不编译其C代码 |

厂商PDF不随仓库再分发。选定die revision和勘误清单仍需在样件/制造资料中冻结。

## 本次发现与修正

1. **原SAES HAL的IV映射与H573手册不符。** Embassy-stm320.6.0的`saes::load_iv`按`ivr(i)`写第一至第四个大端字。RM0481表373要求第一nonce字写IVR3、计数器2写IVR0。现vault不再调用该HAL密码路径，改为专用寄存器序列；模型断言`nonce0102030405060708090A0B0C`对应`IVR3=01020304,IVR2=05060708,IVR1=090A0B0C,IVR0=2`。旧路径无任何真实秘密部署。此为目标适配问题，不擅自宣称已验证上游产品漏洞/CVE。
2. **完成标志不能掩盖密码故障。** 原HAL块读取清除全部ICR位；新状态机在轮询、输出和清理前后检查错误，正常进度只写CCF。KEIF、RNGEIF、RWEIF/读写错误和超时使实例永久失败，输出全零；IPRST擦除key/context，擦除卡死重置整机。模型注入在key加载和输出读取时出现的错误，确认不被当作成功。
3. **H573随机源不能沿用通用常数。** 本版metapac的通用推荐HTCR为AA74，缺少RNG_NSCR。AN4230 Rev15表3针对H563/573/562、48MHz列出CR00F01E00、HTCR6A91、NSCR3AF66。新端口明确设置并回读这些值，关闭自动错误恢复、保持时钟检测并锁配置；保留所有运行中错误，禁止重置重试后伪装成成功抽取。基线丢弃、连续32位重复检查和部分输出零化为附加筛查，不声称提供熵证明。
4. **Secure地址与外设归属要实际配置。** 现PAC常量使用Non-secure别名；新密码端口使用RM0481列出的Secure别名，并检查SAES/RNG的Secure及privileged属性和HSI48源。它要求真正部署可信启动/TZEN、TAMP、RAM/DMA隔离及串行引导阶段；不会替调用方伪造这些前提。
5. **安全通道不是单向握手。** 新SCP03强制C-MAC/C-ENC/R-MAC/R-ENC，拒绝降低保护、未知profile、错误card cryptogram、错误MAC和未认证状态。未认证6A88等错误不会被解释为空白设备/缺失记录。接收MAC在解密前常时比较，坏包关闭实例；实时授权在每段阻塞操作之后、最终输出之前再次检查。
6. **传输异常不能无限等待或重复私密指令。** T=1固定总预算、WTX/帧数/重试上限，拒绝错误I/R序号与畸形ATR；数据/未知NACK不作为地址busy重试。电源/链路状态不确定时禁止自动重放或退款持久预算。

密钥寄存器采用下降写序；RM0481明确允许正常key模式的单调上升或下降写序，这不是缺陷。SCP03 S8仍只有64位wire MAC，即使使用AES256也不会增加认证tag位数；每会话4096命令上限只是预算约束，整机寿命/攻击请求预算仍需评审。

## 内存与代码边界

[Secure MMIO](../firmware/secure-mmio/README.md)是本版显式的unsafe Rust例外：一个函数中六个固定地址PAC构造（含新增Secure I2C1），地址不来自主机，不构造RAM引用，不做指针切片或FFI。其他自研协议/策略/密码状态机继续`forbid(unsafe_code)`，运行构建CC/CXX拒绝C/C++。R03“所有自研运行代码无unsafe”的表述只适用于当时源码。

SAES被单个所有者持有，vault与SCP适配器按操作互斥借用；会话密钥和响应缓冲单独零化，不通过日志/Debug或主机原始密码API导出。芯片ROM、外部库、MCU特权保护域及传感器分类器仍属于可信边界。

## 实际证据与未完成项

本地测试包括NIST AES/CMAC/GCM固定向量、独立OpenSSL生成的AES128/256 SCP03转录、最大/空消息、部分GCM块、4080字节vault块、MAC逐位变异、重放/乱序、断言守卫撤销、超时与故障标志。模型使用独立RustCrypto密码计算，并不仿真H573硅片。ARM编译/Clippy与完整CI结果见[验证记录](VALIDATION.md)。

仍为NOT_RUN：真实Secure别名访问/时钟/健康测试、H573芯片KAT、全部错误与内部TAMP联动、SE E2配对/强制SCP/对象ACL/原子ledger、传感器配对与随机IV故障、实际密钥擦除时间、优化二进制的功耗/近场EM/故障注入。Masked SAES不保护CPU上的CMAC子密钥/XOR、所有RAM/栈路径、总线或整个指纹分类流程；不能从这些模型推出整机抗侧信道。

完整CTAP运行时、受保护调度器、启动/更新/生命周期和真实管理通道仍未完成，当前诊断镜像不启用这些新模块，不接受真实账户秘密。
