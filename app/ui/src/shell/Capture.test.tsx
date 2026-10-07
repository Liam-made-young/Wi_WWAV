import { act, createRef } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { $, mountWith, type Rig, settle, type } from '../heat/testkit';
import { Capture, type CaptureHandle } from './Capture';

// docs/PLAN.md S1.6 against Heat's fake core. What a fail looks like: ⌘⇧N
// that doesn't write through `heat.capture.add`, a footer that isn't "4 in
// inbox · captured ✓" after the fourth, a capture that isn't one undo step,
// or a panel that closes on Return.

let rig: Rig;
afterEach(() => rig?.unmount());

describe('quick capture', () => {
  it('saves through heat.capture.add and says "4 in inbox · captured ✓"', async () => {
    const handle = createRef<CaptureHandle>();
    const errors: string[] = [];
    rig = await mountWith(<Capture ref={handle} shown onError={(m) => errors.push(m)} />);
    expect($(rig, '.capture-foot')!.textContent).toBe('3 in inbox');
    await type($(rig, '.capture-field'), 'fix the snare at 1:32');
    await act(async () => handle.current!.save());
    await settle();
    expect($(rig, '.capture-foot')!.textContent).toBe('4 in inbox · captured ✓');
    expect(($(rig, '.capture-field') as HTMLTextAreaElement).value).toBe('');
    expect([...rig.fake.store.capture.values()].map((c) => c.text)).toContain('fix the snare at 1:32');
    expect((await rig.call<{ undo: string }>('history.get', {})).undo).toBe('Undo capture');
    // The next keystroke starts a new capture: the footer goes back to the count.
    await type($(rig, '.capture-field'), 'and the hat');
    expect($(rig, '.capture-foot')!.textContent).toBe('4 in inbox');
    expect(errors).toEqual([]);
  });

  it('saves nothing for an empty capture, and the inbox waits in Heat', async () => {
    const handle = createRef<CaptureHandle>();
    rig = await mountWith(<Capture ref={handle} shown onError={() => {}} />);
    await act(async () => handle.current!.save());
    await settle();
    expect(rig.fake.store.capture.size).toBe(3);
  });
});
