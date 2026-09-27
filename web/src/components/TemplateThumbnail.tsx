import { convertFileSrc } from "@tauri-apps/api/core";
import { FileText, Presentation, Table2 } from "lucide-react";
import { templatePreviewImage, type TemplatePackRecord } from "@/lib/template-packs";

type TemplateThumbnailProps = {
  template?: TemplatePackRecord | null;
  format?: string | null;
  className?: string;
  iconSize?: number;
};

export function TemplateThumbnail({
  template,
  format,
  className = "",
  iconSize = 22,
}: TemplateThumbnailProps) {
  const image = templatePreviewImage(template);
  const src = image ? previewAssetSrc(image) : "";
  const normalizedFormat = template?.format ?? format ?? "pptx";

  return (
    <span className={`template-thumbnail${src ? " has-image" : ""} ${className}`.trim()}>
      {src ? (
        <img src={src} alt="" draggable={false} />
      ) : (
        <TemplateThumbnailIcon format={normalizedFormat} size={iconSize} />
      )}
    </span>
  );
}

function TemplateThumbnailIcon({ format, size }: { format: string; size: number }) {
  if (format === "xlsx") {
    return <Table2 size={size} />;
  }
  if (format === "docx") {
    return <FileText size={size} />;
  }
  return <Presentation size={size} />;
}

function previewAssetSrc(path: string) {
  if (/^(?:https?:|data:|asset:|file:|\/)/.test(path)) {
    if (/^(?:\/preview\/|\/rdeckforge-app-icon\.png)/.test(path)) {
      return path;
    }
    if (isTauriRuntime() && path.startsWith("/")) {
      return convertFileSrc(path);
    }
    return path;
  }
  return path;
}

function isTauriRuntime() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
