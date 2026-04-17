export type RecallEvent = {
  time: string;
  source: string;
  title: string;
  detail?: string;
  url?: string;
};

export type IndexedEvent = { event: RecallEvent; index: number };
export type HourGroup = { hour: string; items: IndexedEvent[] };

export function todayIso(): string {
  return new Date().toISOString().slice(0, 10);
}

export function addDaysIso(iso: string, days: number): string {
  const d = new Date(iso + "T12:00:00");
  d.setDate(d.getDate() + days);
  return d.toISOString().slice(0, 10);
}

export function formatDayHeading(iso: string): string {
  const d = new Date(iso + "T12:00:00");
  const today = todayIso();
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);
  const yesterdayStr = yesterday.toISOString().slice(0, 10);

  const weekday = d.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" });
  if (iso === today) return `Today — ${weekday}`;
  if (iso === yesterdayStr) return `Yesterday — ${weekday}`;
  return weekday;
}

export function groupEventsByHour(events: RecallEvent[]): HourGroup[] {
  const groups: HourGroup[] = [];
  let currentHour = "";
  for (let i = 0; i < events.length; i++) {
    const ev = events[i];
    const hour = ev.time.slice(0, 2) + ":00";
    if (hour !== currentHour) {
      currentHour = hour;
      groups.push({ hour, items: [] });
    }
    groups[groups.length - 1].items.push({ event: ev, index: i });
  }
  return groups;
}
