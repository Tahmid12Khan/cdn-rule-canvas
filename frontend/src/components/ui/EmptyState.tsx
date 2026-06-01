import type { ReactNode } from "react";

// Server-safe component: presentational empty-state placeholder.
interface EmptyStateProps {
  title: string;
  description?: string;
  action?: ReactNode;
}

export function EmptyState({ title, description, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-status-prevBg bg-bg-elevated px-6 py-12 text-center">
      <h2 className="text-lg font-semibold text-nav">{title}</h2>
      {description && (
        <p className="max-w-md text-sm text-status-prevFg">{description}</p>
      )}
      {action}
    </div>
  );
}
