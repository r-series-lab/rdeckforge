import type { ReactNode } from "react";
import { Button } from "@/components/ui/button";

type ActionableStatusAction = {
  label: string;
  onClick: () => void;
  disabled?: boolean;
};

type ActionableStatusProps = {
  status: "idle" | "running" | "ok" | "error";
  label: string;
  message: string;
  children?: ReactNode;
  actions?: ActionableStatusAction[];
};

export function ActionableStatus({
  status,
  label,
  message,
  children,
  actions = [],
}: ActionableStatusProps) {
  return (
    <div className={`actionable-status ${statusClass(status)}`}>
      <span className={`status-pill ${statusClass(status)}`}>{label}</span>
      <span className="actionable-status-message">{message}</span>
      {children}
      {actions.length > 0 ? (
        <div className="actionable-status-actions">
          {actions.map((action) => (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              key={action.label}
              onClick={action.onClick}
              disabled={action.disabled}
            >
              {action.label}
            </Button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function statusClass(status: ActionableStatusProps["status"]) {
  if (status === "ok") return "is-ok";
  if (status === "error") return "is-danger";
  if (status === "running") return "is-warn";
  return "";
}
