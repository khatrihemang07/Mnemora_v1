import type { Component } from 'svelte';
import CalendarDays from '~icons/lucide/calendar-days';
import CircleCheck from '~icons/lucide/circle-check';
import Inbox from '~icons/lucide/inbox';
import Layers from '~icons/lucide/layers';
import List from '~icons/lucide/list';
import Sun from '~icons/lucide/sun';
import type { SmartViewId } from './views';

type Icon = Component<{ class?: string }>;

export const VIEW_ICONS: Record<SmartViewId, Icon> = {
  inbox: Inbox,
  today: Sun,
  upcoming: CalendarDays,
  all: Layers,
  done: CircleCheck,
};

/** A List without an emoji. */
export const LIST_ICON: Icon = List;
