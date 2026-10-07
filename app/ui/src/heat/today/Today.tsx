import { useSheets } from '../frame';
import { useTabActs } from '../frame';

export function Today() {
  const { newTask } = useSheets();
  useTabActs({ plus: { run: () => newTask({ intoPlan: true }) }, secondary: { run: () => {} } });
  return <div className="heat-stub">Today</div>;
}
