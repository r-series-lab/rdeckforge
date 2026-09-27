import { FileJson, History, Layers3, Library, ScrollText, type LucideIcon } from "lucide-react";

export type PageKey = "prompts" | "templates" | "content" | "render" | "history";

export type AppModule = {
  key: PageKey;
  label: string;
  shortLabel: string;
  description: string;
  icon: LucideIcon;
};

export const appModules: AppModule[] = [
  {
    key: "prompts",
    label: "AI 交接包",
    shortLabel: "交接",
    description: "准备结构契约与 AI 生成提示",
    icon: ScrollText,
  },
  {
    key: "templates",
    label: "模板库",
    shortLabel: "模板",
    description: "链接、检查和管理本地模板包",
    icon: Library,
  },
  {
    key: "content",
    label: "输出校验",
    shortLabel: "验收",
    description: "验证 JSON / Markdown 与模板契约",
    icon: FileJson,
  },
  {
    key: "render",
    label: "生成",
    shortLabel: "生成",
    description: "模板 + 内容 → Office 文档",
    icon: Layers3,
  },
  {
    key: "history",
    label: "任务中心",
    shortLabel: "任务",
    description: "继续文档任务并查看生成记录",
    icon: History,
  },
];
