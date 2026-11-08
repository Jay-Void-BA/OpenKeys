# R0.4 当前状态

当前入口：[主README](../README.md)、[R04硬件](../hardware/r04/README.md)、[安全复查](SECURITY_REVIEW_R04.md)、[安全测试步骤](SECURITY_TEST_PLAN.md)。本文件取代旧版OTP/双USB/PIN备用状态表。

| 已完成 | 证据 |
|---|---|
| 10页原理图、130位置完整电气PCB、1430段走线及216过孔 | ERC/DRC0违规、0未连接，624项独立板网核对 |
| Gerber/钻孔/CPL、外壳/按键/指纹托源文件和尺寸图 | R04 exports；89个CAD文件SHA256绑定 |
| 被动线圈和复位RC模拟、eFuse DC角落计算 | 模型与明确假设，未冒充EMC/物理攻击模拟 |
| 设备授权/认证指纹帧/四键PIN/可信LCD策略 | no_std Rust库，真实硬件适配未部署 |
| Notes/recovery导入解析、三平台英文GUI开发构建 | 真实硬件后端保持拒绝；不是可用管理器 |
| 家用/工厂/实验室测试方法 | 逐台证据格式，NOT_RUN不写PASS |

当前必须继续：按R04引脚实现目标板运行时和实际驱动；认证SE/FPC配对与持久状态；完整CTAP/vault/认证管理通道；可信启动/调试/防回滚配置；三系统与手机的真实设备互通；对样板进行上电、结构/RF/电源、EMC、侧信道和故障注入资格。缺少实物不阻止完成独立可做的源码、图纸、检查与制造资料。

固定范围：指纹必须先通过，PIN仅附加；白背光16×2字符LCD；四个实体上下文按钮；唯一USB-C母口兼充电，NFC；FIDO2/U2F/hmac-secret、凭据、加密notes及recovery codes；无OTP。自己的运行代码Rust，按模块允许Zig，不引入自写C/C++。
