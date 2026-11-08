# 开源方案比较

检索/拉取日期：2026-11-08。所有项目锁定具体提交，见 references.lock.tsv；不自动跟随分支更新。以下为源码检查与选型推论，不是整机安全审计。

| 项目 | 可参考部分 | 本项目采用边界 |
|---|---|---|
| [Google OpenSK](https://github.com/google/OpenSK) | Rust CTAP、U2F、hmac-secret、credential management、fingerprint/bio enrollment、环境与密码接口抽象、测试 | 当前提交已有指纹接口和流程，不代表传感器信任链或侧信道已获证明。README 明确 develop 是研究平台、不适合日常使用，不能继承历史 CTAP2.0 分支认证。优先研究其模块边界与负向测试。 |
| [Google Wasefire](https://github.com/google/wasefire) | Rust applet 隔离、平台服务、持久存储、更新、指纹示例 | README 声明实验性。FPC2534 参考驱动存在，不等于已经验证防伪、防重放、抗拆或三片可采购。WebAssembly 隔离的 RAM/性能开销需测量。 |
| [Trussed FIDO](https://github.com/trussed-dev/fido-authenticator) | Rust FIDO 应用，Solo2/Nitrokey3 使用；独立 crypto/storage/UI 接口 | 当前版本为 0.4.0-rc.4。底层文件系统与 SE 运算映射需要审查。littlefs2-core 接口本身不等于整个存储实现纯 Rust。 |
| [Solo 2](https://github.com/solokeys/solo2) | USB/NFC、多应用调度、FM11NC08 驱动、CLI/测试、制造与封装配置 | 当前 LPC55 平台不能直接烧到 STM32。develop 特性含 no-encrypted-storage、no-buttons、no-reset-time-window；不能使用该配置处理真实身份。Cargo.toml 明确含 littlefs2-sys / C lfs.c 依赖。 |
| [Solo 2 HW](https://github.com/solokeys/solo2-hw) | KiCad 模块板、原理图与 BOM 生成方式 | README 明确缺 NFC 天线与 cavity PCB，不是完整 NFC 制造包；硬件 CERN-OHL-S v2，衍生与交付保留相应源文件、声明和许可证义务。 |
| [Nitrokey 3](https://github.com/Nitrokey/nitrokey-3-firmware) | Rust 固件工程、SE 后端接入、生产/升级组织、参考板与 CI | 含 littlefs2，README 构建依赖 clang。已有 SE 不代表其所有 FIDO 密钥都在 SE；官方说明 FIDO2 除额外随机数外不使用 SE。不能照搬这一密钥边界后声称本项目 FIDO 抗物理侧信道。 |
| [Zig keylib](https://github.com/Zig-Sec/keylib) | Zig CTAP 库、UV/UP 回调、客户端示例，README 提及 PRF/hmac-secret | GitHub 镜像指向 Codeberg。当前 build.zig.zon 含 hidapi、translate_c，不能把整个构建当成无 C。自行实现存储、UV、硬件运算与验证的责任仍在开发者。 |
| [PassKeeZ](https://github.com/Zig-Sec/PassKeeZ) | Zig Linux 平台认证器、KDBX/管理体验参考 | 是 Linux 平台软件认证器，不是本项目的独立硬件钥匙；README 明确不支持其自身数据库生物解锁，不能充当三平台设备管理器。 |

## 路线决定

1. 平台层与新管理软件优先 Rust。Zig 按模块可选，用户已确认不必两种语言都用。
2. 将 OpenSK 与 Trussed 放在独立适配层后比较；先用同一组协议、容量、SE 不导出、授权、断电、延时测试决定。现在不宣称已完成核心移植。
3. 新工作区不链接 reference 代码，先独立实现授权模型和导入库，避免未审查地继承开发开关与 C 依赖。
4. 如果复用 Trussed/LittleFS 路线，须替换或明确剔除 C 存储实现；重写日志存储不能省略原子性/掉电/磨损测试。
5. 不能以 Rust/Zig、内存安全、品牌或芯片认证代替时序、功耗、EM 和故障注入验证。

## OpenSK 的具体 SE 适配缺口

当前 EcSecretKey::export 文档允许硬件封装表示，因此不能简单说 OpenSK 必然导出裸私钥。但是 KeyStore::cred_random 的返回类型是 Secret<[u8; 32]>，默认密钥存储还会把主封装/认证材料交给软件密码接口。若要求 hmac-secret 根秘密始终不离开 SE，就必须重构相关接口为密钥句柄与 SE 内部 HMAC 操作，不能只接一颗安全芯片便沿用默认实现。

源码：[EcSecretKey](https://github.com/google/OpenSK/blob/2382394d4d61e170ef9ed1a2f7d39bf62e94b7d5/libraries/opensk/src/api/crypto/ec_signing.rs)、[KeyStore](https://github.com/google/OpenSK/blob/2382394d4d61e170ef9ed1a2f7d39bf62e94b7d5/libraries/opensk/src/api/key_store.rs)。

## 直接检查入口

- OpenSK：libraries/opensk/src/env/mod.rs；ctap/fingerprint.rs；ctap/credential_management.rs；api/crypto。
- Wasefire：crates/runner-nordic/src/board/fpc2534.rs；examples/rust/fingerprint；crates/store。
- Trussed：src/ctap2.rs；src/ctap2/credential_management.rs；Cargo.toml。
- Solo2：runners/lpc55/Cargo.toml；components/fm11nc08；Cargo.toml；SECURITY.md。
- 上游 docs/脚本/示例只是参考；未执行上游烧录脚本、安装脚本或代码。
