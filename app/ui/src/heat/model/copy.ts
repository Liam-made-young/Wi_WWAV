// Every user-facing Heat string, in the spec's words (docs/SPEC.md chapter 3,
// 2.11 and 8.10). Plain, second person, present tense; no exclamation marks.
// Where the spec gives a pattern rather than a sentence, the function here is
// the one place that fills it in.

// Due phrases (3.1): "Today 4:00 PM", "Tomorrow 11:59 PM", "2d overdue".
export const due = {
  today: (time: string) => `Today ${time}`,
  tomorrow: (time: string) => `Tomorrow ${time}`,
  onDay: (day: string, time: string) => `${day} ${time}`,
  overdue: (amount: string) => `${amount} overdue`,
};

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

// The sidebar's "Your average time" (3.1).
export const averageTime = (type: string, time: string, count: number) => `${type} ${time} (${count})`;

// The LCD's second line (3.1).
export const weeklyLoad = (time: string, count: number) => `This week: ${time} across ${plural(count, 'task')}`;

// Repeat presets, as the PKM's task panel offers them (3.6).
export const repeat = {
  none: 'Does not repeat',
  daily: 'Daily',
  weekdays: 'Every weekday',
  weekly: 'Weekly',
  monthly: 'Monthly',
  custom: 'Custom…',
};
