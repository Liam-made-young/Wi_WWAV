import { useSheets, useTabActs } from '../frame';

export function Calendar() {
  const { newTask } = useSheets();
  useTabActs({ plus: { run: () => newTask() }, secondary: { run: () => {} } });
  return <div className="heat-stub">Calendar</div>;
}
