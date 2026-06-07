import {
  createSignal,
  onMount,
  onCleanup,
  Show,
  type Accessor,
  type ComponentProps,
  type JSX,
} from "solid-js";
import ArrowDown from "lucide-solid/icons/arrow-down";

export interface ConversationApi {
  isAtBottom: Accessor<boolean>;
  scrollToBottom: (smooth?: boolean) => void;
  registerEl: (el: HTMLDivElement) => void;
  onScroll: () => void;
}

// ── Conversation ─────────────────────────────────────────────────────

interface ConversationProps extends Omit<ComponentProps<"div">, "children"> {
  /** Pixels from bottom to still consider "at bottom". Default 60. */
  threshold?: number;
  children?: (conversation: ConversationApi) => JSX.Element;
}

export function Conversation(allProps: ConversationProps) {
  const { class: className, threshold = 60, children, ...rest } = allProps;
  let scrollEl: HTMLDivElement | undefined;
  const [isAtBottom, setIsAtBottom] = createSignal(true);

  const registerEl = (el: HTMLDivElement) => {
    scrollEl = el;
  };

  const onScroll = () => {
    if (!scrollEl) return;
    const diff = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight;
    setIsAtBottom(diff < threshold);
  };

  const scrollToBottom = (smooth = true) => {
    if (!scrollEl) return;
    scrollEl.scrollTo({
      top: scrollEl.scrollHeight,
      behavior: smooth ? "smooth" : "instant",
    });
  };

  const conversation: ConversationApi = {
    isAtBottom,
    scrollToBottom,
    registerEl,
    onScroll,
  };

  return (
    <div class={`conversation ${className ?? ""}`} role="log" aria-live="polite" {...rest}>
      {children?.(conversation)}
    </div>
  );
}

// ── ConversationContent ──────────────────────────────────────────────

interface ConversationContentProps extends ComponentProps<"div"> {
  conversation: ConversationApi;
  setRef?: (el: HTMLDivElement | undefined) => void;
  children?: JSX.Element;
}

export function ConversationContent(allProps: ConversationContentProps) {
  const { class: className, conversation, setRef, children, ...rest } = allProps;
  let ref: HTMLDivElement | undefined;
  let ro: ResizeObserver | undefined;

  const handleRef = (el: HTMLDivElement | undefined) => {
    ref = el;
    setRef?.(el);
  };

  onMount(() => {
    if (!ref) return;
    conversation.registerEl(ref);
    // Initial scroll to bottom when content loads
    conversation.scrollToBottom(false);
    ro = new ResizeObserver(() => {
      if (conversation.isAtBottom()) conversation.scrollToBottom(false);
    });
    ro.observe(ref);
    onCleanup(() => {
      ro?.disconnect();
      setRef?.(undefined);
    });
  });

  return (
    <div
      ref={handleRef}
      onScroll={conversation.onScroll}
      class={`chat-history conversation-content ${className ?? ""}`}
      {...rest}
    >
      {children}
    </div>
  );
}

// ── ConversationEmptyState ───────────────────────────────────────────

interface ConversationEmptyStateProps extends ComponentProps<"div"> {
  title?: string;
  description?: string;
  icon?: JSX.Element;
  children?: JSX.Element;
}

export function ConversationEmptyState(allProps: ConversationEmptyStateProps) {
  const {
    class: className,
    title = "No messages yet",
    description = "Run a workflow or select a paused node to continue.",
    icon,
    children,
    ...rest
  } = allProps;

  return (
    <div class={`conversation-empty ${className ?? ""}`} {...rest}>
      {children ?? (
        <>
          {icon && <div class="conversation-empty-icon">{icon}</div>}
          <div class="conversation-empty-text">
            <p class="conversation-empty-title">{title}</p>
            {description && <p class="conversation-empty-description">{description}</p>}
          </div>
        </>
      )}
    </div>
  );
}

// ── ConversationScrollButton ─────────────────────────────────────────

interface ConversationScrollButtonProps extends ComponentProps<"button"> {
  conversation: ConversationApi;
  children?: JSX.Element;
}

export function ConversationScrollButton(allProps: ConversationScrollButtonProps) {
  const { class: className, conversation, children, ...rest } = allProps;

  return (
    <Show when={!conversation.isAtBottom()}>
      <button
        class={`conversation-scroll-button ${className ?? ""}`}
        onClick={() => conversation.scrollToBottom(true)}
        type="button"
        aria-label="Scroll to latest"
        {...rest}
      >
        {children ?? <ArrowDown class="conversation-scroll-icon" />}
      </button>
    </Show>
  );
}
