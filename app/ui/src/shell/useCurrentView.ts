// Whether the view a component sits in is the one showing. RoomViews.tsx keeps
// every view mounted and marks the current one with data-current, so a view
// can hold its work (a sky, a timer) while hidden and wake when shown.

import { type RefObject, useEffect, useState } from 'react';

export function useCurrentView(ref: RefObject<HTMLElement | null>): boolean {
  const [current, setCurrent] = useState(false);
  useEffect(() => {
    const section = ref.current?.closest('[data-room]');
    if (!section) {
      setCurrent(true); // on its own, as in a test: always showing
      return;
    }
    const read = () => setCurrent(section.getAttribute('data-current') === 'true');
    read();
    const watch = new MutationObserver(read);
    watch.observe(section, { attributes: true, attributeFilter: ['data-current'] });
    return () => watch.disconnect();
  }, [ref]);
  return current;
}
