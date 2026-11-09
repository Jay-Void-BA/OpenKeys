# SE050 I2C1进度与重试规则

R04软件使用STM32H573 I2C1 Secure别名`0x50005400`，固定7位地址0x48。
PB6/PB7仍由唯一HAL owner配置100kHz/开漏；运行时不调用HAL的Unknown-NACK
read/write。`openkeys-board::se_i2c`执行有界寄存器状态机，`secure-mmio`提供
固定地址PAC视图并保持原owner生命周期。此变更不启用SE、不烧写任何密钥或fuse。

依据ST RM0481 Rev5 §53（controller transmitter/receiver、NBYTES、RELOAD及ISR）：
发送TXIS在地址/前一字节ACK之后出现；本实现从不预装TXDR。仅在尚未访问任何
数据寄存器、无RXNE/TXIS/TC/TCR、TXE空、NACK出现且STOP/idle确认后，返回
AddressNotReady。清除NACK/STOP后还须读回干净状态。已读/写一个字节、FIFO歧义、
总线错误、异常STOP、配置改变、时钟倒退或超时均关闭PE并永久锁住实例。

每次调用共用固定100ms和200万次状态采样预算；分块不延长期限。256字节以上
使用RELOAD/TCR，只有首块发START，末块AUTOEND。接收错误清零全部调用者缓冲。
不使用外设reset/九脉冲恢复/自动重发来掩盖未知完成状态。持续读取GTZC
I2C1SEC/PRIV、RCC使能与原kernel选择；PIN/时钟/内存归属的完整验证仍由root完成。

11项新增测试含1/3/254/255/256/259字节边界、每个字节后的NACK、地址忙可重试、
错误后不可恢复、STOP未清、晚故障、定时/轮询双重上限、配置变化、DMA/IRQ禁止，
以及把实际T=1 core接到此驱动的组合。模型不宣称芯片时序已经验证。

## 你或厂家需要执行的验证

使用一块无真实秘密的测试板、可控I2C应答器/代理和逻辑分析仪，保存固件SHA256、
板号、芯片revision及原始波形。先使用公共测试SCP密钥；完成后不得保留在生产设备。

1. 确认PB6/PB7开漏、外部上拉与100kHz时序，SE地址为0x48；读取真实E2 ATR和版本。
2. 只在地址ACK位返回NACK：应看到STOP，未发数据字节；允许新START重试，最终成功。
3. 在第1、2、254、255、256及末数据字节ACK位NACK：不得重发该命令；整条SCP实例锁定。
4. 读出头后断开/拉低SDA，或将NACK/RXNE错误同时呈现：不得把它解释成地址忙；无可信APDU输出。
5. 检查256/259字节帧中仅有一个START，255边界保持事务，最终STOP；没有额外字节。
6. 卡住BUSY/STOP/TXIS/RXNE、冻结计时源、改变clock selector或GTZC归属：拒绝输出；
   软件预算有限，AHB/时钟本身卡死还须由独立watchdog重置，不能在等待循环喂狗。
7. 对真实SE连续执行配对SCP03、counter预扣/读回、head写入/提交，再逐点断电。
   对方应答丢失时绝不自动重发或退款，重启不得重建已配对命名空间。

判定：无任何未授权签名、解密/私密显示、未知状态重试或counter回退；否则记录FAIL并停用
该版本。当前上述实物项目均为NOT_RUN，不能由11个模型测试改写为PASS。
