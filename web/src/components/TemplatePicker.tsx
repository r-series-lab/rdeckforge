import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { TemplateThumbnail } from "@/components/TemplateThumbnail";
import {
  groupTemplateFamilies,
  templateSupportsContent,
  templateVariantLabel,
  type TemplatePackRecord,
} from "@/lib/template-packs";

type TemplatePickerProps = {
  templates: TemplatePackRecord[];
  value: string;
  contentPath?: string;
  disabled?: boolean;
  emptyLabel?: string;
  onSelect: (template: TemplatePackRecord | null) => void;
  onRefresh?: () => void;
  onManualPath?: () => void;
};

export function TemplatePicker({
  templates,
  value,
  contentPath = "",
  disabled = false,
  emptyLabel = "手动路径",
  onSelect,
  onRefresh,
  onManualPath,
}: TemplatePickerProps) {
  const families = groupTemplateFamilies(templates);
  const selectedTemplate = templates.find((template) => template.path === value);
  const selectedFamily = selectedTemplate
    ? families.find((family) =>
        family.templates.some((template) => template.id === selectedTemplate.id),
      )
    : undefined;
  const familyValue = selectedFamily?.key ?? "";

  function selectFamily(key: string) {
    if (!key) {
      onSelect(null);
      return;
    }
    const family = families.find((item) => item.key === key);
    const firstCompatible =
      family?.templates.find((template) => templateSupportsContent(template, contentPath)) ??
      family?.templates.find((template) => template.pathAvailable) ??
      family?.templates[0];
    if (firstCompatible) {
      onSelect(firstCompatible);
    }
  }

  function selectTemplate(id: string) {
    const template = templates.find((item) => item.id === id);
    if (template) {
      onSelect(template);
    }
  }

  return (
    <div className="template-picker">
      <div className="template-picker-controls">
        <select
          className="ui-select"
          aria-label="选择模板系列"
          value={familyValue}
          onChange={(event) => selectFamily(event.target.value)}
          disabled={disabled}
        >
          <option value="">{emptyLabel}</option>
          {families.map((family) => (
            <option
              value={family.key}
              key={family.key}
              disabled={family.templates.every((template) => !template.pathAvailable)}
            >
              {family.name} · {family.format.toUpperCase()}
              {family.templates.length > 1 ? `（${family.templates.length} 种样式）` : ""}
            </option>
          ))}
        </select>
        {selectedFamily && selectedFamily.templates.length > 1 ? (
          <select
            className="ui-select"
            aria-label="选择模板样式"
            value={selectedTemplate?.id ?? ""}
            onChange={(event) => selectTemplate(event.target.value)}
            disabled={disabled}
          >
            {selectedFamily.templates.map((template) => (
              <option
                value={template.id}
                key={template.id}
                disabled={!templateSupportsContent(template, contentPath)}
              >
                {templateVariantLabel(template, selectedFamily)}
              </option>
            ))}
          </select>
        ) : null}
        {onRefresh ? (
          <Button type="button" onClick={onRefresh} disabled={disabled}>
            <RefreshCw size={14} aria-hidden="true" />
            刷新
          </Button>
        ) : null}
      </div>
      <div className={`template-picker-preview${selectedTemplate ? "" : " is-empty"}`}>
        <TemplateThumbnail template={selectedTemplate} format={selectedTemplate?.format} iconSize={22} />
        <div>
          <strong>{selectedTemplate?.name ?? "未选择模板"}</strong>
          <span>
            {selectedTemplate
              ? `${selectedTemplate.format.toUpperCase()} · ${
                  selectedTemplate.pathAvailable ? "本地可用" : "路径失效"
                }`
              : "从模板库选择，或在高级配置里填入本地路径。"}
          </span>
          {value ? <small className="path-text">{value}</small> : null}
        </div>
        {onManualPath ? (
          <Button type="button" size="sm" onClick={onManualPath} disabled={disabled}>
            手动路径
          </Button>
        ) : null}
      </div>
    </div>
  );
}
