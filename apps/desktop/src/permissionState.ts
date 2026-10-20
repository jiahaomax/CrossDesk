import type {Status} from './types';

// Same capability rules as crossdesk-platform::InputPermissions. Accessibility
// includes event listening/posting; the raw checks need not all be true.
export function macPermissionCapabilities(p: Status['input_permissions']) {
  const listening = p ? p.accessibility || p.input_monitoring : undefined;
  const posting = p ? p.accessibility || p.post_event : undefined;
  return {listening, posting, ready: listening === true && posting === true};
}
