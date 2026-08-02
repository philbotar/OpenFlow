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
  hasContentAbove: Accessor<boolean>;
  hasContentBelow: Accessor<boolean>;
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
  const [hasContentAbove, setHasContentAbove] = createSignal(false);
  const [hasContentBelow, setHasContentBelow] = createSignal(false);

  const updateScrollState = () => {
    if (!scrollEl) return;
    const remaining =
      scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight;
    setIsAtBottom(remaining < threshold);
    setHasContentAbove(scrollEl.scrollTop > 1);
    setHasContentBelow(remaining > 1);
  };

  const registerEl = (el: HTMLDivElement) => {
    scrollEl = el;
    updateScrollState();
  };

  const onScroll = () => updateScrollState();

  const scrollToBottom = (smooth = true) => {
    if (!scrollEl) return;
    scrollEl.scrollTo({
      top: scrollEl.scrollHeight,
      behavior: smooth ? "smooth" : "instant",
    });
    updateScrollState();
  };

  const conversation: ConversationApi = {
    isAtBottom,
    hasContentAbove,
    hasContentBelow,
    scrollToBottom,
    registerEl,
    onScroll,
  };

  return (
    <div
      class={`conversation ${className ?? ""}`}
      classList={{
        "has-content-above": hasContentAbove(),
        "has-content-below": hasContentBelow(),
      }}
      role="log"
      aria-live="polite"
      {...rest}
    >
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
  let mo: MutationObserver | undefined;
  let rafId: number | undefined;

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
      if (conversation.isAtBottom()) {
        conversation.scrollToBottom(false);
        return;
      }
      conversation.onScroll();
    });
    ro.observe(ref);
    mo = new MutationObserver(() => {
      if (!conversation.isAtBottom()) {
        conversation.onScroll();
        return;
      }
      if (rafId != null) return;
      rafId = requestAnimationFrame(() => {
        rafId = undefined;
        conversation.scrollToBottom(false);
      });
    });
    mo.observe(ref, { childList: true, subtree: true, characterData: true });
    onCleanup(() => {
      ro?.disconnect();
      mo?.disconnect();
      if (rafId != null) cancelAnimationFrame(rafId);
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
