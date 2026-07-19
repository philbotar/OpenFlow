import type { JSX, ParentProps } from "solid-js";
import { AnimatedModal } from "./AnimatedModal";
import { Button } from "./Button";
import { ButtonRow } from "./ButtonRow";

interface PickerModalProps extends ParentProps {
  open: boolean;
  onClose: () => void;
  ariaLabel: string;
  backdropClass?: string;
  eyebrow: string;
  title: string;
  description?: string;
  toolbar?: JSX.Element;
}

export function PickerModal(props: PickerModalProps) {
  return (
    <AnimatedModal
      open={props.open}
      onClose={props.onClose}
      ariaLabel={props.ariaLabel}
      backdropClass={props.backdropClass}
    >
      <div class="node-picker-header">
        <div>
          <div class="eyebrow">{props.eyebrow}</div>
          <h3>{props.title}</h3>
          {props.description ? <p>{props.description}</p> : null}
        </div>
      </div>
      {props.toolbar}
      {props.children}
      <ButtonRow align="end">
        <Button variant="secondary" onClick={props.onClose}>
          Cancel
        </Button>
      </ButtonRow>
    </AnimatedModal>
  );
}
