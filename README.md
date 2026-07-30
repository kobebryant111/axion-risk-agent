# 智联鉴控 · Axiom Risk Agent

合作机构全生命周期 AI 智能风控桌面客户端（Tauri 2 + Rust + React + Tailwind）。

## 功能定位

- **风险吹哨**：六类主体差异化舆情监测 + 四级预警
- **经营守护**：财报四维分析 + 融担专项
- **准入瞭望**：一票否决 / 重大违规 / 关注事项研判

需求文档见 `docs/需求文档.md`。

## 技术栈

- Tauri 2.x / Rust
- React 19 + TypeScript + Vite
- Tailwind CSS（Soft UI，对齐参考仪表盘）

## 本地运行

```bash
# 安装依赖
npm install

# 仅前端（浏览器预览，自动回退 Demo 数据）
npm run dev

# Tauri 桌面端
npm run tauri dev
```

前端开发端口：`http://localhost:1421`

## 目录

```
src/                 React UI
src-tauri/           Rust / Tauri 命令
docs/需求文档.md      PRD
```

## 当前进度（MVP 骨架）

- [x] 工程脚手架
- [x] Soft UI 总览 Dashboard（六类卡片 / 趋势 / 健康分 / 线索列表 / 右侧事件）
- [x] Rust 命令返回 Demo 仪表盘与线索
- [ ] 黑猫等外部源适配器
- [ ] 规则引擎与准入清单
- [ ] 财报解析与报告导出
