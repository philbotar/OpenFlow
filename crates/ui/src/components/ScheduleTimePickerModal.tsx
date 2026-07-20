import { For } from "solid-js";
import {
  ALL_WEEKDAYS,
  WEEKDAY_OPTIONS,
  WEEKDAY_PRESET,
  normalizeWeekdays,
  toggleWeekday,
} from "../lib/schedule";
import type { ScheduleDraft } from "../lib/schedule";
import { AnimatedModal } from "./AnimatedModal";
import { Button } from "./Button";
import { ButtonRow } from "./ButtonRow";

interface ScheduleTimePickerModalProps {
  open: boolean;
  draft: ScheduleDraft;
  onClose: () => void;
  onChange: (patch: Partial<ScheduleDraft>) => void;
}

export function ScheduleTimePickerModal(props: ScheduleTimePickerModalProps) {
  const isDaySelected = (day: string) => props.draft.weekdays.includes(day);

  const setWeekdays = (weekdays: string[]) => {
    props.onChange({
      preset: "timed",
      weekdays: normalizeWeekdays(weekdays),
    });
  };

  return (
    <AnimatedModal
      open={props.open}
      onClose={props.onClose}
      ariaLabel="Edit schedule time and days"
      cardClass="schedule-time-picker-card"
    >
      <div class="schedule-time-picker-header">
        <div class="eyebrow">At time</div>
        <h3>Time and days</h3>
      </div>

      <label class="schedule-time-picker-time">
        <span>Time</span>
        <input
          class="text-input"
          type="time"
          value={props.draft.time}
          onInput={(event) =>
            props.onChange({
              preset: "timed",
              time: event.currentTarget.value,
            })
          }
        />
      </label>

      <div class="schedule-day-select">
        <div class="schedule-day-select-header">
          <span class="schedule-day-select-label">Days</span>
          <div class="schedule-day-select-shortcuts">
            <button type="button" onClick={() => setWeekdays([...ALL_WEEKDAYS])}>
              All
            </button>
            <button type="button" onClick={() => setWeekdays([...WEEKDAY_PRESET])}>
              Weekdays
            </button>
          </div>
        </div>
        <div class="schedule-day-grid" role="group" aria-label="Days of week">
          <For each={WEEKDAY_OPTIONS}>
            {([value, label]) => (
              <button
                type="button"
                classList={{ active: isDaySelected(value) }}
                aria-pressed={isDaySelected(value)}
                onClick={() =>
                  props.onChange({
                    preset: "timed",
                    weekdays: toggleWeekday(props.draft.weekdays, value),
                  })
                }
              >
                {label}
              </button>
            )}
          </For>
        </div>
      </div>

      <ButtonRow align="end">
        <Button variant="primary" onClick={props.onClose}>
          Done
        </Button>
      </ButtonRow>
    </AnimatedModal>
  );
}
