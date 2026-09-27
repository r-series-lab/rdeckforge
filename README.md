# rDeckForge

`rDeckForge` 是一个本地 Tauri 文档工作台，用来把外部 AI 生成的结构化内容安全地校验并渲染成 Office 文档。它围绕「私有模板包 + AI 交接 prompt + 内容验收 + PPTX/DOCX/XLSX 生成」组织工作流，适合把医院、公司、课程、咨询交付等场景里的固定版式沉淀为可复用模板。

应用本身不做大模型推理。rDeckForge 负责把模板协议、写作规范、内容骨架和业务 brief 打包成 prompt，交给你选择的外部 AI；AI 返回 `content.json` 或 `content.md` 后，再由 rDeckForge 在本机完成校验、修复提示和 Office 渲染。

## 核心定位

- 私有模板工作台：链接本机模板包，管理 PPTX/DOCX/XLSX/脚本型模板，不把真实模板、Logo、客户资料打包进仓库或应用。
- AI 交接层：把模板契约、内容骨架、brief 和模板作者说明组合成可复制给网页 AI 的 prompt。
- 内容验收层：检查 JSON / Markdown 是否符合模板协议、内置内容 profile、资源路径和绑定关系。
- Office 渲染层：生成 PPTX、DOCX、XLSX，写入前做结构完整性检查，失败时不覆盖已有文件。
- 任务中心：保存轻量任务记录和生成历史，方便继续、复制、重新生成或定位输出文件。

## 应用界面

### 工作台外壳

桌面端使用紧凑的 r 系列工作台布局：

- 左侧导航：`AI 交接包`、`模板库`、`输出校验`、`生成`、`任务中心`。
- 顶部窗口工具：收起左栏、收起生成页右侧检查器、打开设置。
- 右侧检查器：在 `生成` 页显示当前模板、内容、输出路径、状态、预检摘要和生成指标。
- 快速恢复：如果存在未完成任务，侧栏底部会出现继续入口。
- 设置面板：集中管理深色/浅色主题、默认输出目录、文件名规则、同名文件策略、生成后自动打开，以及环境体检。

### 生成

`生成` 是默认主页面，用四步流程把内容变成 Office 文件：

1. 选择模板：从模板库选择已链接模板包，也可以手动选择本地模板目录。
2. 内容源：选择 AI 输出的 JSON / Markdown 文件。
3. 输出设置：设置输出路径，或按设置里的目录和命名规则自动生成。
4. 生成结果：先预检，再生成 PPTX/DOCX/XLSX，完成后可打开、定位、复制路径或继续同类任务。

生成页会同时显示当前配置、结构与输出预览、输入协议、渲染方式、可用样式数量和预检清单。预检失败时会给出修复清单，例如缺少模板、缺少内容、输出路径不可用、字段不符合协议、模板绑定缺失等。模板属于同一系列时，可以按兼容样式批量生成多份输出。

### AI 交接包

`AI 交接包` 用来准备给外部 AI 的输入：

- 选择目标模板和需求 brief。
- 生成交接 prompt，并在应用内预览、复制、打开或定位。
- 把网页 AI 返回的 Markdown / JSON 粘贴回应用。
- 保存 AI 输出后，直接跳转到 `输出校验` 或 `生成`。

这个页面的边界很清晰：AI 负责根据 brief 写内容，rDeckForge 负责提供模板协议、约束、校验和 Office 渲染。

### 模板库

`模板库` 是模板资产的管理入口：

- 链接包含 `template.manifest.json` 的本地模板包。
- 搜索模板名称、ID、输入协议，按 PPTX/DOCX/XLSX 筛选。
- 按模板系列查看不同样式，预览页面模板、文档块、工作表结构和输入协议。
- 检查模板 readiness，运行模板包自测，查看文件、引擎、运行时和依赖是否可用。
- 重新定位已移动的模板目录，刷新校验结果，移除本地记录。
- 导入 / 导出可迁移的 `.rdeckpack` 模板包。
- 从现有 PPTX/DOCX/XLSX 创建草稿模板包。
- 为 Python、Node、Shell、PowerShell 等脚本渲染器创建脚本型模板适配器。
- 导出模板契约和内容骨架，供 AI 或自动化流程使用。

模板库只保存路径和校验元数据。真实 Office 模板、素材和业务内容仍然留在你选择的本机目录。

### 输出校验

`输出校验` 适合在生成前审查 AI 内容：

- 读取或编辑 `content.json`、`content.md`、`markdown`。
- 选择模板和 recipe 后自动校验，也可以手动触发。
- 展示 schema 错误、模板 warning、绑定 warning、资源路径问题和 acceptance summary。
- 支持未保存内容的临时文本校验，也支持保存微调后的内容文件。
- 验收通过后可直接调用统一 Office 渲染入口。

内置内容 profile 包括：

- `teaching_deck_v1`：教学课件内容，支持 JSON / Markdown，可渲染 PPTX 或 DOCX。
- `design_doc_v1`：设计文档内容，支持 Markdown 派生结构，可渲染 DOCX。
- `feature_assessment_v1`：功能点评估表内容，支持 JSON，可渲染 XLSX。

### 任务中心

`任务中心` 保存两类本地记录：

- 文档任务：从 AI 交接、内容校验或生成页创建，可按全部、进行中、已生成、需处理筛选。
- 生成历史：每次生成后的输出记录，可重新生成、打开文件、定位文件或复制路径。

任务只记录路径、格式、recipe、阶段、校验状态和最近错误，不复制模板文件或业务内容到 SQLite。

### 设置与环境体检

设置面板包含三个页签：

- 外观：深色 / 浅色主题，保存在当前电脑。
- 输出：默认输出目录、文件名模板、同名文件自动编号或覆盖、生成后自动打开。
- 环境体检：检查应用存储、PPTX sidecar、脚本运行时、系统打开文件能力等本机环境。

文件名规则支持 `{content}`、`{template}`、`{date}`、`{format}` 占位符，默认规则是 `{content}-{template}`。

## 常见工作流

### 1. 从需求到 Office 文档

1. 在 `模板库` 链接或导入模板包。
2. 在 `AI 交接包` 选择模板和 brief，生成交接 prompt。
3. 把 prompt 复制给外部 AI，让它返回 `content.json` 或 `content.md`。
4. 将 AI 输出粘贴回应用并保存。
5. 在 `输出校验` 查看协议、字段、资源和绑定问题。
6. 在 `生成` 页预检并导出 PPTX/DOCX/XLSX。

### 2. 已有内容直接生成

1. 在 `生成` 页选择模板。
2. 选择已有 JSON / Markdown 内容文件。
3. 按规则生成输出路径，或手动选择保存位置。
4. 运行预检，确认修复清单为空或只有可接受 warning。
5. 生成文件并在系统里打开或定位。

### 3. 模板作者流程

1. 从 PPTX/DOCX/XLSX 创建草稿模板包，或链接已有模板包。
2. 检查模板结构、输入协议和 authoring instructions。
3. 导出 `template-contract.json` 和内容 skeleton。
4. 运行 readiness 和模板自测。
5. 需要分发时导出 `.rdeckpack`，在另一台机器导入并链接。

### 4. 自动化 / Agent 流程

CLI 的 `--json` 输出采用稳定信封：`ok`、`command`、`data` 或 `error`。校验失败、参数错误、运行时缺失、帮助和版本输出在 `--json` 模式下仍保持机器友好格式。

推荐入口：

```bash
rdeckforge --json workflow prepare \
  --prompt-pack /absolute/prompt-pack \
  --brief /absolute/brief.md \
  --template /absolute/template-pack \
  --prompt-out /absolute/ai-handoff-prompt.md \
  --contract-out /absolute/template-contract.json \
  --skeleton-out /absolute/content.skeleton.json

rdeckforge --json workflow run \
  --template /absolute/template-pack \
  --content /absolute/ai-output.json \
  --normalize \
  --out /absolute/output.pptx \
  --validation-out /absolute/validation-report.json
```

详见 [docs/AGENT_CLI_WORKFLOW.md](docs/AGENT_CLI_WORKFLOW.md)。

## 支持能力

### 模板与输入

- 模板包：本地链接模板包、`.rdeckpack` 导入导出、路径重定位、readiness、health check、自测。
- Office 草稿：从 PPTX 提取 slide shape，从 DOCX 提取 `{{placeholder}}`，从 XLSX 提取工作表和 named range。
- 输入格式：`content.json`、`content.md`、content pack 目录、带本地 assets 的内容包。
- 模板作者上下文：模板包可声明 authoring instructions 和 examples，交接 prompt 会注入这些说明。

### 渲染器

- PPTX：声明式页面、文本/list/image/table/chart 绑定、重复分页、overflow variant、同族模板批量生成。
- DOCX：无模板 Markdown 设计文档、占位符替换、列表/表格/图片、语义 document blocks、表格原型样式扩展。
- XLSX：assessment JSON 内置输出、单元格/命名区域绑定、列表和表格扩展、表头/数据行原型样式保留。
- Script：Python、Node、Shell、PowerShell、外部命令 runtime、hybrid template、post-render pipeline。
- 安全写入：先写事务文件，再检查 Office ZIP、XML parts、content types 和 relationship，成功后才替换目标输出。

## 快速开始

```bash
npm run web:install
npm run dev
```

常用检查：

```bash
npm run rust-check
npm run web:build
npm run pptx:build
npm --prefix web run test
npm run test:scripts
cargo run -- --json info
```

运行 macOS 桌面冒烟测试：

```bash
npm run build
npm run smoke:macos-ui
```

冒烟测试会打开打包后的应用，访问主要工作台页面，检查标题、输出校验和统一 UI 错误记录。运行测试的终端需要 macOS Accessibility 权限。可使用 `-- --keep-open` 保持应用打开，或使用 `-- --app /absolute/rDeckForge.app` 指定其他包。

## CLI 常用命令

开发环境可用 `cargo run -- --json <command>`，安装后可直接使用 `rdeckforge --json <command>`。

```bash
rdeckforge --json info
rdeckforge --json capabilities
rdeckforge --json profile list

rdeckforge --json template list
rdeckforge --json template link --template /absolute/template-pack
rdeckforge --json template readiness --template /absolute/template-pack
rdeckforge --json template test --template /absolute/template-pack --case smoke --out-dir /absolute/test-outputs
rdeckforge --json template export --template /absolute/template-pack --out /absolute/template.rdeckpack
rdeckforge --json template import --archive /absolute/template.rdeckpack --out-dir /absolute/external-template-root
rdeckforge --json template contract --template /absolute/template-pack --out /absolute/template-contract.json
rdeckforge --json template content-skeleton --template /absolute/template-pack --mode minimal --out /absolute/content.skeleton.json

rdeckforge --json template create-pptx-draft --input /absolute/template.pptx --out-dir /absolute/draft-pack
rdeckforge --json template create-docx-draft --input /absolute/template.docx --out-dir /absolute/draft-pack
rdeckforge --json template create-xlsx-draft --input /absolute/template.xlsx --out-dir /absolute/draft-pack
rdeckforge --json template create-script-adapter --script /absolute/build.mjs --out-dir /absolute/script-pack --id my-script-template --name "My Script Template" --format pptx --input-format md

rdeckforge --json prompt build --pack /absolute/prompt-pack --brief /absolute/brief.md --template /absolute/template-pack --out /absolute/ai-handoff-prompt.md
rdeckforge --json content validate --input /absolute/content.json --template /absolute/template-pack --recipe teaching_deck --out /absolute/validation-report.json
rdeckforge --json content normalize --input /absolute/ai-output.json --template /absolute/template-pack --recipe teaching_deck --out /absolute/normalized.json

rdeckforge --json render office --template /absolute/template-pack --input /absolute/content.json --out /absolute/output.pptx
rdeckforge --json render family --template /absolute/template-pack --input /absolute/content.json --out-dir /absolute/outputs --base-name demo
rdeckforge --json render docx --input /absolute/front-end-design.md --out /absolute/front-end-design.docx
rdeckforge --json render xlsx --input /absolute/feature-assessment.json --out /absolute/feature-assessment.xlsx

rdeckforge --json diagnostics env
rdeckforge --json diagnostics xlsx --input /absolute/content.json --template /absolute/template-pack --recipe teaching_deck --out /absolute/report.xlsx
rdeckforge --json history list --limit 50
```

完整模板 manifest 说明见 [docs/TEMPLATE_MANIFEST.md](docs/TEMPLATE_MANIFEST.md)，多格式适配说明见 [docs/MULTIFORMAT_ADAPTERS.md](docs/MULTIFORMAT_ADAPTERS.md)。

## 本地数据与隐私边界

rDeckForge 的原则是：私有模板和业务内容留在你选择的位置，应用只保存路径和必要元数据。

- macOS: `~/Library/Application Support/rDeckForge/rdeckforge.sqlite3`
- Windows: `%LOCALAPPDATA%\rDeckForge\rdeckforge.sqlite3`
- Linux: `${XDG_DATA_HOME:-~/.local/share}/rDeckForge/rdeckforge.sqlite3`

可通过 `RDECKFORGE_DATA_DIR` 覆盖数据目录。解析后的路径可在 `rdeckforge --json info` 或设置里的环境体检中查看。

隐私规则：

- 不在仓库中放真实公司、医院、客户或朋友提供的模板。
- 不把私有 PPTX/DOCX/XLSX、Logo、导出背景图和模板专用 prompt 打进应用包。
- SQLite 只存模板路径、任务路径、校验摘要和生成历史，不复制业务内容。
- 环境变量只检查是否存在，不返回变量值。
- `.env` 文件不参与正式发行流程。

## 构建与发行

本地普通构建：

```bash
npm run build
```

构建会同时编译 Tauri 应用、AI 友好的 `rdeckforge` CLI，以及独立的 `rdeckforge-pptx` PPTX renderer。打包后的应用通过 Tauri `externalBin` 携带 sidecar，已安装用户不需要系统 Node.js 来生成声明式 PPTX。

安装本机 CLI：

```bash
npm run build
npm run cli:install
```

macOS 会安装到 `~/Library/Application Support/rDeckForge/bin` 并创建 `~/.local/bin/rdeckforge`。Linux 使用 `~/.local/share/rDeckForge/bin`。Windows 安装到 `%LOCALAPPDATA%\rDeckForge\bin`，需要把该目录加入用户 `PATH`。

正式发行使用带保护的命令：

```bash
npm run release:preflight
npm run release:build
```

正式构建要求 `TAURI_SIGNING_PRIVATE_KEY`。macOS 还需要非 ad-hoc 的 `APPLE_SIGNING_IDENTITY` 和 Apple notarization 凭据；Windows 需要已导入的代码签名证书和 `RDECKFORGE_WINDOWS_CERTIFICATE_THUMBPRINT`。Tauri updater artifact 会生成，但运行时更新检查和 `latest.json` 发布仍保持关闭，直到稳定 HTTPS 发布地址和 updater public key 确定。

Windows 和 macOS 使用同一个 release 命令。Windows 请在原生 x64 Windows 机器上安装 Node 24、stable Rust、WebView2 和 Windows 10/11 SDK 后运行：

```powershell
npm ci --prefix web
npm ci --prefix renderers/pptx-node
npm run build
```

产物在 `target/release/bundle`，包括 NSIS `.exe` 和/或 MSI。`npm run release:verify` 会检查 CLI sidecar、PPTX renderer sidecar 和平台安装包是否存在。

## 示例

生成中性演示 PPTX：

```bash
npm --prefix ./renderers/pptx-node run create-demo-template
cargo run -- --json render pptx \
  --template examples/templates/demo-medical-teaching-v1 \
  --recipe teaching_deck \
  --input examples/content/demo-teaching-deck.json \
  --out target/demo-output.pptx
```

渲染独立设计文档：

```bash
rdeckforge --json render docx \
  --input /path/to/front-end-design.md \
  --out /path/to/front-end-design.docx
```

渲染功能点评估表：

```bash
rdeckforge --json render xlsx \
  --input /path/to/feature-assessment.json \
  --out /path/to/feature-assessment.xlsx
```

## 项目结构

```text
rdeckforge/
  src/                         Rust core: CLI, templates, validation, rendering, storage
  src-tauri/                   Tauri desktop shell and command bridge
  web/                         React + Vite + TypeScript UI
  renderers/pptx-node/         PPTX sidecar renderer
  examples/                    neutral demo briefs, content, templates, prompt packs
  schemas/                     content, prompt pack, template manifest schemas
  docs/                        agent workflow, template manifest, multi-format notes
  scripts/                     build, release, smoke, clean, install helpers
```

公开仓库边界、贡献约定、安全报告和发布合同见 [PUBLIC_REPOSITORY.md](PUBLIC_REPOSITORY.md)、[CONTRIBUTING.md](CONTRIBUTING.md)、[SECURITY.md](SECURITY.md) 和 [RELEASE.md](RELEASE.md)。
