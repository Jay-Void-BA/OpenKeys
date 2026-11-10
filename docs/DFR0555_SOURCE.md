# DFR0555 来源与版本

核验日期：2026-11-10。规格采用厂家一手资料，不使用兼容 LCD 的猜测寄存器。

| 资料 | URL / 固定版本 | 作用 |
|---|---|---|
| 中国大陆官方商城 | https://www.dfrobot.com.cn/goods-1697.html | DFR0555、¥55、页面显示有货；3.3–5V、≤20mA、87×32×13mm |
| 官方 Wiki | https://wiki.dfrobot.com/dfr0555/docs/24390 | V1.0 / V1.1 的背光地址差异 |
| 厂家参考库 | https://github.com/DFRobot/DFRobot_RGBLCD1602/tree/8e2b24daf12847dc1ebe6405580c7e8b1101551f | 初始化 / 控制寄存器顺序参考，不编译或捆绑厂家 C++ 运行代码 |

固定源码内容摘要：

```text
DFRobot_RGBLCD1602.cpp 5740dda25b9f1522901d75f05520b23291440d0163180553611c64ce09deaed5
DFRobot_RGBLCD1602.h   5bebe3b73bc6113c14e1f20cc145ce77625ff09a45cb056a980122edf81e5f60
```

本项目驱动为独立 Rust `no_std` 实现，参照上述 MIT 库中的公开寄存器事实。厂家源码仍受其 [MIT 许可](https://github.com/DFRobot/DFRobot_RGBLCD1602/blob/8e2b24daf12847dc1ebe6405580c7e8b1101551f/LICENCE)约束；不把第三方参考代码宣称为自研。参考仓库同时记录于 [references.lock.tsv](../references.lock.tsv)。

供电锁存与隔离电路原始依据：TI [TPS22919](https://www.ti.com/lit/ds/symlink/tps22919.pdf)、[SN74LVC1G74](https://www.ti.com/lit/ds/symlink/sn74lvc1g74.pdf)、[SN74LVC1G17](https://www.ti.com/lit/ds/symlink/sn74lvc1g17.pdf)、[SN74LVC1G08](https://www.ti.com/lit/ds/symlink/sn74lvc1g08.pdf)、[SN74LVC2G66](https://www.ti.com/lit/ds/symlink/sn74lvc2g66.pdf)的数据手册引脚表与电气条件。DCU 封装 1G74 的 CLK=1、D=2、反相 Q=3、GND=4、Q=5、CLR=6、PRE=7、VCC=8，不能套用其他 D 触发器的脚序。

商城显示有货不等于已下单或保证三件同批。模块的实际背面器件、安装孔、工作电压容差、内置上拉、寄生电容和失电文字消退需要供应商确认 / 样品验证。
