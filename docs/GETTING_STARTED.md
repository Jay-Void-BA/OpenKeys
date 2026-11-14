# 开发起步

[文档导航](README.md) · [当前状态](DEVELOPMENT_STATUS.md) · [贡献指南](../CONTRIBUTING.md)

本页用于阅读源码、运行软件检查和查看开发界面。当前没有可用的生产认证镜像；电气诊断和桌面开发界面各有独立用途。

## 1. 准备源码与工具

```sh
git clone https://github.com/Jay-Void-BA/OpenKeys.git
cd OpenKeys
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
```

仓库的 `rust-toolchain.toml` 固定 Rust 1.99.1。保留 `Cargo.lock`，使用 `--locked`；首次构建需要下载锁定的依赖。普通软件检查不需要安装 KiCad 或购买调试器。

## 2. 检查软件

在仓库根目录运行：

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

三项分别检查行为、静态诊断和格式。它们不会烧录或配对设备。旧 [R04 CI](ci-evidence-r04.json)仅作历史追溯，新版结果以对应提交为准。

检查 Cortex-M33 核心与目标适配：

```sh
rustup target add thumbv8m.main-none-eabihf --toolchain 1.99.1
cargo check -p openkeys-authenticator-core -p openkeys-h573-r05 --target thumbv8m.main-none-eabihf --locked
```

目标编译成功不代表可信启动、外设隔离或实片时序已经部署。

## 3. 查看桌面开发界面

在有图形桌面的 macOS、Linux 或 Windows 上运行：

```sh
cargo run -p openkeys-manager --features gui --locked
```

界面为英文。真实设备后端尚未完成，保持锁定，不能通过主机按钮或主机 PIN 解锁。三个平台的候选开发包与构建限制见[R05 开发构建](NATIVE_BUILDS_R05.md)；导入样例只使用[合成测试数据](IMPORT_FORMAT.md)。

## 4. 查看硬件与来源检查

无需 CAD 环境的摘要检查：

```sh
python3 hardware/r05/tools/manifest.py --check
python3 scripts/freeze-r05-board.py --check
python3 scripts/export-r05-native-pin-layout.py --check
python3 scripts/export-r05-pin-layout.py --check
python3 scripts/check-r05-diagnostic.py
python3 scripts/check-diagnostic-manifest.py
python3 scripts/export-se-state-layout.py --check
python3 scripts/check-opensk-vendor.py
```

Windows 如使用 Python Launcher，可将 `python3` 换成 `py -3`。需要 Python 3.9 或更新版本。
原生 UserID v4 是当前 PIN 布局候选；旧 v2 布局检查仅用于历史回归，均不会配置真实设备。

- 看图：[原理图 PDF](../hardware/r05/exports/OpenKeys-schematic.pdf)、[尺寸 PDF](../hardware/r05/exports/OpenKeys-dimensions.pdf)。
- 编辑 / 重导出：[R0.5 工具与文件说明](../hardware/r05/README.md)。需要指定版本的 KiCad、pcbnew、OpenSCAD 等环境。
- 电气诊断：[R0.5 目标镜像说明](../firmware/r05/README.md)。诊断默认关闭 SE、充电和私密服务。
- 制作样板：[制造入口](../manufacturing/README.md)。先取得个人、小批量接单确认和工程审核。

`hardware/r05/tools/board.py` 不带参数会覆盖现有走线。仅查看或重新导出时使用硬件说明中的安全导出步骤。

## 5. 查看交付门槛

```sh
python3 scripts/check-production-ready.py
```

该命令是 R0.4 历史生产预检，会更新 `manufacturing/readiness-report.json`；R0.5 的放行条件见[当前复查](SECURITY_REVIEW_R05.md)。当前应返回 **退出码 2 / BLOCKED**，列出未完成项；这不是普通构建失败，也不能改写成 PASS。生产放行还需要真实固件、安装包和逐台硬件证据。
