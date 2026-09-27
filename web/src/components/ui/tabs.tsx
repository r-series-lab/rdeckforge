import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";
import { cn } from "@/lib/utils";

export function Tabs({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("ui-tabs", className)} {...props} />;
}

export function TabsList({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn("ui-tabs-list", className)} {...props} />;
}

export function TabsTrigger({
  className,
  active,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { active?: boolean }) {
  return <button className={cn("ui-tabs-trigger", active && "is-active", className)} {...props} />;
}

export function TabsContent({ children }: { children: ReactNode }) {
  return <>{children}</>;
}
