import type { ComponentProps, JSX } from "solid-js";

export interface PanelEmptyStateProps extends ComponentProps<"div"> {
  title?: string;
  description?: string;
  icon?: JSX.Element;
  children?: JSX.Element;
}

export function PanelEmptyState(allProps: PanelEmptyStateProps) {
  const {
    class: className,
    title,
    description,
    icon,
    children,
    ...rest
  } = allProps;

  return (
    <div class={`panel-empty ${className ?? ""}`} {...rest}>
      {children ?? (
        <>
          {icon && <div class="panel-empty-icon">{icon}</div>}
          <div class="panel-empty-text">
            {title && <p class="panel-empty-title">{title}</p>}
            {description && <p class="panel-empty-description">{description}</p>}
          </div>
        </>
      )}
    </div>
  );
}
