import { useEffect } from "react";
import type { ReactNode } from "react";
import { X } from "lucide-react";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type ConfigDialogProps = {
  open: boolean;
  title: string;
  description?: string;
  className?: string;
  iconClose?: boolean;
  onClose: () => void;
  children: ReactNode;
};

export function ConfigDialog({
  open,
  title,
  description,
  className,
  iconClose = false,
  onClose,
  children,
}: ConfigDialogProps) {
  useEffect(() => {
    if (!open) {
      return;
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose, open]);

  if (!open) {
    return null;
  }

  return (
    <div className="dialog-layer">
      <button
        type="button"
        className="dialog-backdrop"
        aria-label="关闭高级配置"
        onClick={onClose}
      />
      <Dialog
        className={cn("config-dialog", className)}
        role="dialog"
        aria-modal="true"
        aria-label={title}
      >
        <div className="config-dialog-header">
          <div>
            <strong>{title}</strong>
            {description ? <span>{description}</span> : null}
          </div>
          {iconClose ? (
            <Button
              className="config-dialog-close"
              size="icon"
              title="关闭"
              aria-label={`关闭${title}`}
              onClick={onClose}
            >
              <X size={16} strokeWidth={1.8} />
            </Button>
          ) : (
            <Button size="sm" onClick={onClose}>
              完成
            </Button>
          )}
        </div>
        <div className="config-dialog-body">{children}</div>
      </Dialog>
    </div>
  );
}
