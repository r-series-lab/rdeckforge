import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Tooltip({ className, ...props }: HTMLAttributes<HTMLSpanElement>) {
  return <span className={cn("ui-tooltip", className)} {...props} />;
}
