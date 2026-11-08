# R0 交付前检查

2026-11-08，工具：KiCad 9.0.2、OpenSCAD 2021.01、Blender 4.3.2、Python 3。环境为Linux容器，未连接硬件。

- 7页原理图解析并导出PDF/SVG/XML网表；117个BOM位号、104个网络。
- 原始ERC：[erc.json](erc.json)。0个电气错误、8个封装链接警告，未修改规则或排除告警。分别为J1/J2/J5/J7、U8/U11、SW1/SW6；保留为投板前阻断项。
- 从KiCad导出的XML检查31项连接/报告条件，见[netlist-check.json](netlist-check.json)及[检查脚本](../tools/check_netlist.py)。不使用生成脚本的连接JSON作为验证输入。
- 已目视检查尺寸图、装配渲染与全部电路页，调整了标签方向、引脚/标注和标题栏避让。矢量图可以放大核对。
- 三个STL均由OpenSCAD实际构造，CGAL报告Simple=yes；底壳/上盖各为一个连通实体，按键件为四个独立实体。未经过打印、支撑、配合公差或强度验证。
- 同一提交前的Rust工作区：17项测试、fmt、clippy -D warnings通过。软件依赖未变化，沿用[既有依赖扫描记录](../../docs/dependency-audit.json)的精确Cargo.lock/RustSec状态。
- Markdown相对链接、SVG/XML语法、待公开文本的常见凭据格式扫描通过。仅测试合成/RFC数据；没有用户的OTP、指纹或recovery codes。

`PASS_WITH_RELEASE_BLOCKERS`只表示上述数字检查通过，并不放行制造或真实秘密使用。[发布门槛](../RELEASE_GATES.md)全部仍需执行。没有Gerber/贴片坐标、没有完整设备固件，也没有侧信道、故障注入、NFC射频或EMC测试报告。

`SHA256SUMS`记录当前硬件源文件与导出物；不包含自身、生成日志或未来的Git提交号。厂商PDF的摘要另见[datasheets.json](datasheets.json)，PDF未上传。
