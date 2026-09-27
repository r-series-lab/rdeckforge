import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Command({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("ui-command", className)} {...props} />;
}
