import type { HTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export function Sheet({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("ui-sheet", className)} {...props} />;
}
