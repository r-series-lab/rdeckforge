import { useState } from "react";
import { FolderOpen, FolderOutput, Moon, Stethoscope, Sun } from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ConfigDialog } from "@/components/ConfigDialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EnvironmentPage } from "@/pages/EnvironmentPage";
import type { ThemeMode } from "@/lib/theme";
import {
  outputPatternPreview,
  readOutputPreferences,
  saveOutputPreferences,
  type OutputPreferences,
} from "@/lib/output-preferences";

type SettingsTab = "appearance" | "output" | "diagnostics";

type SettingsDialogProps = {
  open: boolean;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  onClose: () => void;
};

export function SettingsDialog({ open, theme, onThemeChange, onClose }: SettingsDialogProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("appearance");
  const [outputPreferences, setOutputPreferences] = useState<OutputPreferences>(() =>
    readOutputPreferences(),
  );

  function updateOutputPreferences(patch: Partial<OutputPreferences>) {
    const next = { ...outputPreferences, ...patch };
    saveOutputPreferences(next);
    setOutputPreferences(next);
  }

  async function chooseDefaultOutputDirectory() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: "选择默认输出目录",
      defaultPath: outputPreferences.defaultDirectory,
    });
    if (typeof selected === "string") {
      updateOutputPreferences({ defaultDirectory: selected });
    }
  }

  return (
    <ConfigDialog
      open={open}
      title="设置"
      description="工作台与本机运行环境"
      className="settings-dialog"
      iconClose
      onClose={onClose}
    >
      <Tabs className="settings-tabs">
        <TabsList className="settings-tabs-list" aria-label="设置分类">
          <span className="settings-tabs-caption">设置分类</span>
          <TabsTrigger
            type="button"
            active={activeTab === "appearance"}
            onClick={() => setActiveTab("appearance")}
          >
            {theme === "light" ? <Sun size={15} /> : <Moon size={15} />}
            <span>
              <strong>外观</strong>
              <small>主题与显示</small>
            </span>
          </TabsTrigger>
          <TabsTrigger
            type="button"
            active={activeTab === "output"}
            onClick={() => setActiveTab("output")}
          >
            <FolderOutput size={15} />
            <span>
              <strong>输出</strong>
              <small>目录与命名</small>
            </span>
          </TabsTrigger>
          <TabsTrigger
            type="button"
            active={activeTab === "diagnostics"}
            onClick={() => setActiveTab("diagnostics")}
          >
            <Stethoscope size={15} />
            <span>
              <strong>环境体检</strong>
              <small>运行依赖与异常</small>
            </span>
          </TabsTrigger>
        </TabsList>

        <div className="settings-content">
          {activeTab === "appearance" ? (
            <div className="settings-content-pane">
              <header className="settings-content-heading">
                <div>
                  <strong>外观</strong>
                  <span>选择适合当前环境的界面主题。</span>
                </div>
              </header>
              <section className="settings-section" aria-labelledby="appearance-title">
                <div className="settings-section-heading">
                  <div>
                    <strong id="appearance-title">界面主题</strong>
                    <span>设置只保存在当前电脑。</span>
                  </div>
                  <span className="settings-theme-icon" aria-hidden="true">
                    {theme === "light" ? <Sun size={18} /> : <Moon size={18} />}
                  </span>
                </div>
                <div className="settings-theme-options" role="radiogroup" aria-label="界面主题">
                  <button
                    type="button"
                    className={theme === "dark" ? "is-active" : ""}
                    role="radio"
                    aria-checked={theme === "dark"}
                    onClick={() => onThemeChange("dark")}
                  >
                    <Moon size={18} />
                    <span>
                      <strong>深色</strong>
                      <small>适合低光环境</small>
                    </span>
                  </button>
                  <button
                    type="button"
                    className={theme === "light" ? "is-active" : ""}
                    role="radio"
                    aria-checked={theme === "light"}
                    onClick={() => onThemeChange("light")}
                  >
                    <Sun size={18} />
                    <span>
                      <strong>浅色</strong>
                      <small>适合明亮环境</small>
                    </span>
                  </button>
                </div>
              </section>
            </div>
          ) : activeTab === "output" ? (
            <div className="settings-content-pane">
              <header className="settings-content-heading">
                <div>
                  <strong>输出</strong>
                  <span>统一文件目录、命名和生成后的操作。</span>
                </div>
              </header>
              <section className="settings-section" aria-labelledby="output-directory-title">
                <div className="settings-section-heading">
                  <div>
                    <strong id="output-directory-title">默认目录</strong>
                    <span>生成页可以按规则快速创建输出路径。</span>
                  </div>
                  <span className="settings-theme-icon" aria-hidden="true">
                    <FolderOpen size={18} />
                  </span>
                </div>
                <div className="settings-output-directory">
                  <Input
                    value={outputPreferences.defaultDirectory}
                    placeholder="尚未设置默认输出目录"
                    onChange={(event) =>
                      updateOutputPreferences({ defaultDirectory: event.target.value })
                    }
                  />
                  <Button
                    type="button"
                    onClick={chooseDefaultOutputDirectory}
                    disabled={!("__TAURI_INTERNALS__" in window)}
                  >
                    选择
                  </Button>
                </div>
              </section>

              <section className="settings-section" aria-labelledby="output-name-title">
                <div className="settings-section-heading">
                  <div>
                    <strong id="output-name-title">文件名规则</strong>
                    <span>支持 content、template、date 和 format 占位符。</span>
                  </div>
                </div>
                <Input
                  value={outputPreferences.fileNamePattern}
                  aria-label="输出文件名规则"
                  onChange={(event) =>
                    updateOutputPreferences({ fileNamePattern: event.target.value })
                  }
                />
                <code className="settings-output-preview">
                  {outputPatternPreview(outputPreferences.fileNamePattern)}
                </code>
              </section>

              <section className="settings-section" aria-labelledby="output-behavior-title">
                <div className="settings-section-heading">
                  <div>
                    <strong id="output-behavior-title">生成行为</strong>
                    <span>控制同名文件和生成完成后的动作。</span>
                  </div>
                </div>
                <div className="settings-output-options" role="radiogroup" aria-label="同名文件处理">
                  <button
                    type="button"
                    className={outputPreferences.conflictPolicy === "increment" ? "is-active" : ""}
                    role="radio"
                    aria-checked={outputPreferences.conflictPolicy === "increment"}
                    onClick={() => updateOutputPreferences({ conflictPolicy: "increment" })}
                  >
                    <strong>自动编号</strong>
                    <small>保留已有文件，追加 -2、-3</small>
                  </button>
                  <button
                    type="button"
                    className={outputPreferences.conflictPolicy === "overwrite" ? "is-active" : ""}
                    role="radio"
                    aria-checked={outputPreferences.conflictPolicy === "overwrite"}
                    onClick={() => updateOutputPreferences({ conflictPolicy: "overwrite" })}
                  >
                    <strong>覆盖文件</strong>
                    <small>使用相同路径替换已有输出</small>
                  </button>
                </div>
                <label className="settings-control-row">
                  <span>
                    <strong>生成后自动打开</strong>
                    <small>单份文档生成成功后立即打开。</small>
                  </span>
                  <Switch
                    checked={outputPreferences.autoOpen}
                    aria-label="生成后自动打开"
                    onChange={(event) => updateOutputPreferences({ autoOpen: event.target.checked })}
                  />
                </label>
              </section>
            </div>
          ) : (
            <EnvironmentPage embedded />
          )}
        </div>
      </Tabs>
    </ConfigDialog>
  );
}
