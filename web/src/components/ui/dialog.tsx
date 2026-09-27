import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Dialog({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("ui-dialog", className)} {...props} />;
}
