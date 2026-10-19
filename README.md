# 晾衣绳 Clothesline · Rust

参考 [Clothesline](https://github.com/zhongguozhang/Clothesline) 的 Windows 候选版行为，用 **Rust + egui + 托盘** 实现的本地原生版本。

## 功能（0.1.0）

- 默认监听系统「下载」文件夹，完成检测：两次稳定扫描 + 2 秒静默，过滤 `.crdownload` / `.part` / `.tmp` 等临时项
- 主屏工作区顶部晾衣绳：彩色衣夹、文件卡片，鼠标移入展开、移出自动收起（可托盘切换「一直显示」）
- 双击打开；右键打开、资源管理器中显示、固定、取下（不删除源文件）
- 托盘：挂上文件夹、重新显示已取下、清空、退出
- 设置保存在 `%LocalAppData%\Clothesline-rust\settings.json`

## 构建

```bash
cd clothesline-rust
cargo build --release
```

运行：`target\release\clothesline-rust.exe`（通知区域找托盘图标，鼠标移到屏幕顶部展开）。

## 测试

```bash
cargo test
```

## 与上游差异

- 首版未实现：剪贴板接入、拖出复制、缩略图、重命名、废纸篓删除、多屏、检查更新、开机启动
- 逻辑对齐 `windows/Clothesline.Core/FileIndex.cs` 的核心扫描与 `RopeState` 选取规则
