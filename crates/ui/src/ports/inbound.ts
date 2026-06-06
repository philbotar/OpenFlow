import type { WorkflowRunState } from "../lib/types";

export interface RunStateEventSink {
	handleRunStateUpdate: (runState: WorkflowRunState) => void;
}
