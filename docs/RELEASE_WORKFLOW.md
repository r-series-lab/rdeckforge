# rDeckForge 发布流程

公开发布前只使用脱敏的测试模板和内容，禁止把真实客户文档、Logo、凭据或渲染产物提交到仓库。

1. 检查 `README.md`、`docs/`、模板协议和 CLI 合同是否一致。
2. 运行脚本测试、前端构建和 Rust 检查；按需在本机验证模板渲染。
3. 使用 `npm run release:preflight` 检查版本、平台和输出目录，再生成发行构建。
4. 检查安装包、CLI sidecar、PPTX renderer sidecar、校验和及更新说明。
5. 先创建 Draft Release，人工复核文件名、版本、截图和测试数据，再决定是否发布。

发布流程不会自动上传私有模板包，也不会把业务文档复制到应用或 GitHub Release。
