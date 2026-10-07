// Get Info for the selected task or block: it slides over the right column
// while something is selected, and Esc slides it back (docs/SPEC.md 3.5).

import { useSelection } from '../frame';
import { useHeat } from '../store';

export function InfoPanel() {
  const { snap, idx } = useHeat();
  const { taskId, blockId } = useSelection();
  const task = taskId ? idx.task.get(taskId) : blockId ? idx.task.get(idx.block.get(blockId)?.taskId ?? '') : undefined;
  void snap;
  return (
    <aside className="heat-info" aria-label="Get Info">
      <h2 className="sheet-title">{task?.title ?? 'Block'}</h2>
    </aside>
  );
}
