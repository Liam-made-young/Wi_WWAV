import { useSheets, useTabActs } from '../frame';

export function Tasks() {
  const { newTask } = useSheets();
  useTabActs({ plus: { run: () => newTask() }, secondary: { run: () => {} } });
  return <div className="heat-stub">Tasks</div>;
}
