import Check from "lucide-solid/icons/check";
import { For } from "solid-js";
import type { ToolCallStatus } from "../../lib/types";

export const UPDATE_TODO_LIST_TOOL = "openflow_update_todo_list";

export type TodoItemStatus = "pending" | "in_progress" | "completed";

export interface TodoItem {
  content: string;
  status: TodoItemStatus;
}

function isTodoItemStatus(value: unknown): value is TodoItemStatus {
  return value === "pending" || value === "in_progress" || value === "completed";
}

export function todoItemsFromArguments(argumentsValue: unknown): TodoItem[] | null {
  if (
    !argumentsValue ||
    typeof argumentsValue !== "object" ||
    Array.isArray(argumentsValue)
  ) {
    return null;
  }
  const todos = (argumentsValue as Record<string, unknown>).todos;
  if (!Array.isArray(todos) || todos.length === 0) {
    return null;
  }

  const parsed: TodoItem[] = [];
  for (const todo of todos) {
    if (!todo || typeof todo !== "object" || Array.isArray(todo)) {
      return null;
    }
    const content = (todo as Record<string, unknown>).content;
    const status = (todo as Record<string, unknown>).status;
    if (typeof content !== "string" || !content.trim() || !isTodoItemStatus(status)) {
      return null;
    }
    parsed.push({ content: content.trim(), status });
  }
  return parsed;
}

export function TodoChecklist(props: {
  items: TodoItem[];
  toolStatus: ToolCallStatus;
}) {
  const completedCount = () =>
    props.items.filter((item) => item.status === "completed").length;

  return (
    <section
      class="todo-checklist"
      data-status={props.toolStatus}
      aria-label="Agent phase checklist"
    >
      <header class="todo-checklist-header">
        <span class="todo-checklist-title">Progress</span>
        <span class="todo-checklist-count">
          {completedCount()}/{props.items.length}
        </span>
      </header>
      <ol class="todo-checklist-items">
        <For each={props.items}>
          {(item) => (
            <li
              class={`todo-checklist-item status-${item.status}`}
              aria-label={`${item.status.replace("_", " ")}: ${item.content}`}
              aria-current={item.status === "in_progress" ? "step" : undefined}
            >
              <span class="todo-checklist-marker" aria-hidden="true">
                {item.status === "completed" ? <Check width={11} height={11} /> : null}
              </span>
              <span class="todo-checklist-content">{item.content}</span>
            </li>
          )}
        </For>
      </ol>
    </section>
  );
}
