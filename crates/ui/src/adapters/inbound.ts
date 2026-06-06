import type { RunStateEventSink, UiDesktopOutboundPort } from "../ports";
import { createUiDesktopOutboundAdapter } from "./outbound";

export function bindRunStateEvents(
	sink: RunStateEventSink,
	outboundPort: UiDesktopOutboundPort = createUiDesktopOutboundAdapter(),
) {
	return outboundPort.listenToRunState((runState) => sink.handleRunStateUpdate(runState));
}
