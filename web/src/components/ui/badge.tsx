import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Badge({ className, ...props }: ComponentProps<"span">) {
  return <span className={cn("badge", className)} {...props} />;
}
