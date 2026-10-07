import { describe, expect, test } from 'vitest';
import { moonCentre, stageGeometry } from './geometry';
import { idleGesture, stemGesture, type GestureInput, type Surface } from './gesture';
import { applyToMix, audible, freshMix, stemShape, voiceLabel, type Mix, type StageAction } from './mix';

// Adversarial review of build/spacemodel. Each test here exposes a defect
// the review found; they are skipped so the suite stays green, and run
// with REVIEW_RUN_KNOWN=1 to watch them fail.
const env = (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env;
const known = env?.REVIEW_RUN_KNOWN ? test : test.skip;

const stage = stageGeometry(560, 560);
const vocals = { kind: 'stem', stem: 'vocals' } as const;
const drums = { kind: 'stem', stem: 'drums' } as const;

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
    known(`a click on a soloed stem is perceivable on ${name}`, () => {
      const before = freshMix();
      before.vocals.soloed = true;
      const { mix: after } = drive(
        [
          { type: 'down', target: vocals, at: moonCentre(stage, 'vocals', 0.8), time: 0 },
          { type: 'up', time: 100 },
        ],
        before,
        surfaceStage,
      );
      expect(after.vocals.muted).toBe(true); // the machine did mute it...
      const seen = (m: Mix) => [audible(m).vocals, voiceLabel(m, 'vocals'), JSON.stringify(stemShape(m, 'vocals', false, 'night'))];
      // ...but nothing that can be heard, seen or read has changed.
      expect(seen(after)).not.toEqual(seen(before));
    });
  }

  // Finding: inputs carry no pointer id, so the machine cannot tell which
  // pointer lifted. The comment in press() says a second finger on another
  // moon "waits its turn", but its lift ends the first finger's press: on
  // a Windows touch screen, resting one finger on vocals and tapping drums
  // mutes vocals while the first finger is still down.
  known('a second pointer lifting does not end the first pointer’s press', () => {
    const { steps, mix } = drive(
      [
        { type: 'down', target: vocals, at: moonCentre(stage, 'vocals', 0.8), time: 0 },
        { type: 'down', target: drums, at: moonCentre(stage, 'drums', 0.8), time: 40 },
        { type: 'up', time: 90 }, // the drums finger lifts; vocals is still held
      ],
      freshMix(),
    );
    expect(steps[2]).toEqual([]);
    expect(mix.vocals.muted).toBe(false);
  });
});
