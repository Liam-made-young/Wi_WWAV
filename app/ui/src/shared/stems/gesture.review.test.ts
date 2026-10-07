import { describe, expect, test } from 'vitest';
import { moonCentre, stageGeometry } from './geometry';
import { idleGesture, stemGesture, type GestureInput, type Surface } from './gesture';
import { applyToMix, audible, freshMix, stemShape, voiceLabel, type Mix, type StageAction } from './mix';

// Adversarial review of build/spacemodel. Each test here exposed a defect
// the review found; they were skipped until the defect was fixed.

const stage = stageGeometry(560, 560);
const vocals = { kind: 'stem', stem: 'vocals' } as const;
const drums = { kind: 'stem', stem: 'drums' } as const;
const onVocals = moonCentre(stage, 'vocals', 0.8);
const onDrums = moonCentre(stage, 'drums', 0.8);

function drive(inputs: GestureInput[], mix0: Mix, surfaceStage: Surface['stage'] = stage) {
  let state = idleGesture();
  let mix = mix0;
  const steps: StageAction[][] = [];
  for (const input of inputs) {
    const out = stemGesture(state, input, { mix, stage: surfaceStage });
    state = out.state;
    mix = applyToMix(mix, out.actions);
    steps.push(out.actions);
  }
  return { steps, mix };
}

describe('review: the one stem gesture', () => {
  // Finding: a click on a soloed stem sets a hidden mute. Solo outranks
  // mute in audible(), stemState() and voiceLabel(), so the click changes
  // nothing you can hear, see or have read to you, against 2.2 and 4.6
  // ("A click ... mutes at once"). The mute then surfaces later, when the
  // solo is cleared, as a stem that is unexpectedly silent.
  for (const [name, surfaceStage] of [
    ['a moon', stage],
    ['a Now strip light', null],
  ] as const) {
    test(`a click on a soloed stem is perceivable on ${name}`, () => {
      const before = freshMix();
      before.vocals.soloed = true;
      const { mix: after } = drive(
        [
          { type: 'down', pointer: 1, command: false, target: vocals, at: onVocals, time: 0 },
          { type: 'up', pointer: 1, time: 100 },
        ],
        before,
        surfaceStage,
      );
      expect(after.vocals.muted).toBe(true); // the machine did mute it...
      const seen = (m: Mix) => [audible(m).vocals, voiceLabel(m, 'vocals'), JSON.stringify(stemShape(m, 'vocals', false, 'night'))];
      // ...and what can be heard, seen and read has changed with it.
      expect(seen(after)).not.toEqual(seen(before));
    });
  }

  // Finding: inputs carry no pointer id, so the machine cannot tell which
  // pointer lifted. The comment in press() says a second finger on another
  // moon "waits its turn", but its lift ends the first finger's press: on
  // a Windows touch screen, resting one finger on vocals and tapping drums
  // mutes vocals while the first finger is still down.
  test('a second pointer lifting does not end the first pointer’s press', () => {
    const { steps, mix } = drive(
      [
        { type: 'down', pointer: 1, command: false, target: vocals, at: onVocals, time: 0 },
        { type: 'down', pointer: 2, command: false, target: drums, at: onDrums, time: 40 },
        { type: 'up', pointer: 2, time: 90 }, // the drums finger lifts; vocals is still held
      ],
      freshMix(),
    );
    expect(steps[2]).toEqual([]);
    expect(mix.vocals.muted).toBe(false);
  });

  test('a second pointer’s travel does not drag the first pointer’s moon, and the first still clicks', () => {
    const { steps, mix } = drive(
      [
        { type: 'down', pointer: 1, command: false, target: vocals, at: onVocals, time: 0 },
        { type: 'down', pointer: 2, command: false, target: drums, at: onDrums, time: 20 },
        { type: 'move', pointer: 2, at: { x: onDrums.x, y: onDrums.y + 60 }, time: 40 },
        { type: 'up', pointer: 2, time: 60 },
        { type: 'up', pointer: 1, time: 100 },
      ],
      freshMix(),
    );
    expect(steps.slice(1, 4).flat()).toEqual([]);
    expect(steps[4]).toEqual([{ type: 'mute', stem: 'vocals', muted: true }]);
    expect([mix.vocals.muted, mix.drums.muted, mix.drums.level]).toEqual([true, false, 0.8]);
  });

  // Finding: with no modifiers in the model, a ⌘-click (5.8: "enters it")
  // and ⌘S or ⌘M arrive as a plain mute or solo.
  test('a ⌘-click enters the stem and never mutes it', () => {
    const { steps, mix } = drive(
      [
        { type: 'down', pointer: 1, command: true, target: vocals, at: onVocals, time: 0 },
        { type: 'up', pointer: 1, time: 80 },
      ],
      freshMix(),
    );
    expect(steps[1]).toEqual([{ type: 'enter', stem: 'vocals' }]);
    expect(mix.vocals.muted).toBe(false);
    const planet = drive(
      [
        { type: 'down', pointer: 1, command: true, target: { kind: 'planet' }, at: stage.centre, time: 0 },
        { type: 'up', pointer: 1, time: 80 },
      ],
      freshMix(),
    );
    expect(planet.steps[1]).toEqual([]);
  });

  test('⌘S and ⌘M are the app’s shortcuts, not a solo and a mute', () => {
    const key = (k: string) => ({ type: 'key', key: k, shift: false, command: true, stem: 'vocals' }) as const;
    const { steps, mix } = drive([key('s'), key('S'), key('m'), key('ArrowUp'), key('Tab')], freshMix());
    expect(steps.flat()).toEqual([]);
    expect(mix).toEqual(freshMix());
  });

  // Finding: one last click for all stems, where iOS keeps one per moon.
  test('a second click on a stem solos it even after a click on another stem between', () => {
    const click = (target: typeof vocals | typeof drums, at: { x: number; y: number }, from: number): GestureInput[] => [
      { type: 'down', pointer: 1, command: false, target, at, time: from },
      { type: 'up', pointer: 1, time: from + 40 },
    ];
    const { mix } = drive([...click(vocals, onVocals, 0), ...click(drums, onDrums, 80), ...click(vocals, onVocals, 160)], freshMix());
    expect(mix.vocals).toMatchObject({ muted: false, soloed: true });
    expect(mix.drums).toMatchObject({ muted: true, soloed: false });
  });
});
