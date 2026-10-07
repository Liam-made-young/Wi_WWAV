import { describe, expect, test } from 'vitest';
import { stageGeometry, moonCentre, fxCentre } from './geometry';
import { HOLD_MS, SLOP_PT, TAP_MS, idleGesture, stemGesture, type GestureInput, type GestureState, type Surface } from './gesture';
import { applyToMix, freshMix, type Mix, type StageAction } from './mix';

const stage = stageGeometry(560, 560);

// Drives the machine the way a surface does: every input, then the mix
// takes the actions. Returns each input's actions and the final mix.
function drive(inputs: GestureInput[], start: { mix?: Mix; stage?: Surface['stage']; state?: GestureState } = {}) {
  let state = start.state ?? idleGesture();
  let mix = start.mix ?? freshMix();
  const surfaceStage = start.stage === undefined ? stage : start.stage;
  const steps: StageAction[][] = [];
  for (const input of inputs) {
    const out = stemGesture(state, input, { mix, stage: surfaceStage });
    state = out.state;
    mix = applyToMix(mix, out.actions);
    steps.push(out.actions);
  }
  return { steps, mix, state };
}

const vocals = { kind: 'stem', stem: 'vocals' } as const;
const drums = { kind: 'stem', stem: 'drums' } as const;
const at = (x: number, y: number) => ({ x, y });
const onVocals = moonCentre(stage, 'vocals', 0.8);

describe('the thresholds of 4.6', () => {
  test('are 250 ms, 400 ms and 8 pt', () => {
    expect([TAP_MS, HOLD_MS, SLOP_PT]).toEqual([250, 400, 8]);
  });

  test('a click under 250 ms mutes at once, on release, with no wait for a second click', () => {
    const { steps, mix } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 1000 },
      { type: 'up', time: 1249 },
    ]);
    expect(steps[1]).toEqual([{ type: 'mute', stem: 'vocals', muted: true }]);
    expect(mix.vocals.muted).toBe(true);
  });

  test('a press of 250 ms is not a click', () => {
    const { steps } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 1000 },
      { type: 'up', time: 1250 },
    ]);
    expect(steps[1]).toEqual([]);
  });

  test('travel under 8 pt is still a click; 8 pt is a drag', () => {
    const still = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'move', at: at(onVocals.x + 7.9, onVocals.y), time: 50 },
      { type: 'up', time: 100 },
    ]);
    expect(still.mix.vocals.muted).toBe(true);
    const moved = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'move', at: at(onVocals.x, onVocals.y - 8), time: 50 },
      { type: 'up', time: 100 },
    ]);
    expect(moved.mix.vocals.muted).toBe(false);
    expect(moved.steps[1]).toEqual([{ type: 'level', stem: 'vocals', level: 0.8 + 8 / stage.span }]);
  });

  test('a second click within 250 ms reverts that mute and solos instead', () => {
    const { steps, mix } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'up', time: 80 },
      { type: 'down', target: vocals, at: onVocals, time: 200 },
      { type: 'up', time: 329 },
    ]);
    expect(steps[3]).toEqual([
      { type: 'mute', stem: 'vocals', muted: false },
      { type: 'solo', stem: 'vocals', soloed: true },
    ]);
    expect(mix.vocals).toMatchObject({ muted: false, soloed: true });
  });

  test('a second click 250 ms after the first is just another click', () => {
    const { steps, mix } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'up', time: 80 },
      { type: 'down', target: vocals, at: onVocals, time: 250 },
      { type: 'up', time: 330 },
    ]);
    expect(steps[3]).toEqual([{ type: 'mute', stem: 'vocals', muted: false }]);
    expect(mix.vocals).toMatchObject({ muted: false, soloed: false });
  });

  test('the revert goes back to how the stem was before the first click', () => {
    const before = freshMix();
    before.drums.muted = true;
    const { mix } = drive(
      [
        { type: 'down', target: drums, at: moonCentre(stage, 'drums', 0.8), time: 0 },
        { type: 'up', time: 60 },
        { type: 'down', target: drums, at: moonCentre(stage, 'drums', 0.8), time: 120 },
        { type: 'up', time: 180 },
      ],
      { mix: before },
    );
    expect(mix.drums).toMatchObject({ muted: true, soloed: true });
  });

  test('clicks on two different moons are two mutes', () => {
    const { mix } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'up', time: 50 },
      { type: 'down', target: drums, at: moonCentre(stage, 'drums', 0.8), time: 100 },
      { type: 'up', time: 150 },
    ]);
    expect([mix.vocals.muted, mix.drums.muted, mix.drums.soloed]).toEqual([true, true, false]);
  });

  test('holding 400 ms blooms that moon’s four FX moons; a second hold puts them away', () => {
    const { steps, state } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'tick', time: 399 },
      { type: 'tick', time: 400 },
      { type: 'up', time: 600 },
    ]);
    expect(steps[1]).toEqual([]);
    expect(steps[2]).toEqual([{ type: 'bloom', owner: 'vocals' }]);
    expect(steps[3]).toEqual([]);
    const again = drive(
      [
        { type: 'down', target: vocals, at: onVocals, time: 1000 },
        { type: 'tick', time: 1400 },
      ],
      { state },
    );
    expect(again.steps[1]).toEqual([{ type: 'bloom', owner: null }]);
  });

  test('a hold is judged by the clock of the next input, so a missed tick is no click', () => {
    const { steps } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'up', time: 450 },
    ]);
    expect(steps[1]).toEqual([{ type: 'bloom', owner: 'vocals' }]);
  });

  test('a drag before 400 ms means no hold', () => {
    const { steps } = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'move', at: at(onVocals.x, onVocals.y - 20), time: 100 },
      { type: 'tick', time: 500 },
    ]);
    expect(steps[2]).toEqual([]);
  });
});

describe('dragging a moon', () => {
  test('along its arm sets level = clamp(level0 + (d · armDir) / span, 0, 1)', () => {
    const out = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'move', at: at(onVocals.x + 3, onVocals.y - stage.span * 0.1), time: 30 },
      { type: 'move', at: at(onVocals.x, onVocals.y + stage.span * 0.5), time: 60 },
      { type: 'move', at: at(onVocals.x, onVocals.y - stage.span), time: 90 },
      { type: 'up', time: 120 },
    ]);
    expect(out.steps[1]).toEqual([{ type: 'level', stem: 'vocals', level: expect.closeTo(0.9, 12) }]);
    expect(out.steps[2]).toEqual([{ type: 'level', stem: 'vocals', level: expect.closeTo(0.3, 12) }]);
    expect(out.steps[3]).toEqual([{ type: 'level', stem: 'vocals', level: 1 }]);
    expect(out.steps[4]).toEqual([]);
  });

  test('bass slides left to get louder', () => {
    const mix = freshMix();
    mix.bass.level = 0.5;
    const onBass = moonCentre(stage, 'bass', 0.5);
    const out = drive(
      [
        { type: 'down', target: { kind: 'stem', stem: 'bass' }, at: onBass, time: 0 },
        { type: 'move', at: at(onBass.x - stage.span * 0.25, onBass.y), time: 30 },
      ],
      { mix },
    );
    expect(out.mix.bass.level).toBeCloseTo(0.75, 12);
  });

  test('across its arm pans only while its FX moons are out', () => {
    const across = (state?: GestureState) =>
      drive(
        [
          { type: 'down', target: vocals, at: onVocals, time: 5000 },
          { type: 'move', at: at(onVocals.x + stage.span * 0.25, onVocals.y - 2), time: 5030 },
        ],
        { state },
      );
    const plain = across();
    expect(plain.steps[1]).toEqual([{ type: 'level', stem: 'vocals', level: expect.closeTo(0.8 + 2 / stage.span, 12) }]);
    const bloomed = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'tick', time: 400 },
      { type: 'up', time: 500 },
    ]).state;
    const panned = across(bloomed);
    expect(panned.steps[1]).toEqual([{ type: 'pan', stem: 'vocals', pan: expect.closeTo(0.25, 12) }]);
    expect(panned.mix.vocals.level).toBe(0.8);
  });

  test('keeps to the axis it chose, however it wobbles', () => {
    const bloomed = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'tick', time: 400 },
      { type: 'up', time: 500 },
    ]).state;
    const out = drive(
      [
        { type: 'down', target: vocals, at: onVocals, time: 1000 },
        { type: 'move', at: at(onVocals.x, onVocals.y - 10), time: 1030 },
        { type: 'move', at: at(onVocals.x + 40, onVocals.y - 12), time: 1060 },
      ],
      { state: bloomed },
    );
    expect(out.steps[2]).toEqual([{ type: 'level', stem: 'vocals', level: expect.closeTo(0.8 + 12 / stage.span, 12) }]);
  });
});

describe('the planet and the sky', () => {
  test('a click on the planet plays or pauses; a hold blooms the master FX moons', () => {
    const planet = { kind: 'planet' } as const;
    const click = drive([
      { type: 'down', target: planet, at: stage.centre, time: 0 },
      { type: 'up', time: 100 },
    ]);
    expect(click.steps[1]).toEqual([{ type: 'playPause' }]);
    const hold = drive([
      { type: 'down', target: planet, at: stage.centre, time: 0 },
      { type: 'tick', time: 400 },
    ]);
    expect(hold.steps[1]).toEqual([{ type: 'bloom', owner: 'master' }]);
  });

  test('with another body bloomed, a click only puts its FX moons away', () => {
    const bloomed = drive([
      { type: 'down', target: drums, at: moonCentre(stage, 'drums', 0.8), time: 0 },
      { type: 'tick', time: 400 },
      { type: 'up', time: 450 },
    ]).state;
    const out = drive(
      [
        { type: 'down', target: vocals, at: onVocals, time: 1000 },
        { type: 'up', time: 1050 },
      ],
      { state: bloomed },
    );
    expect(out.steps[1]).toEqual([{ type: 'bloom', owner: null }]);
    expect(out.mix.vocals.muted).toBe(false);
  });

  test('a click on empty sky puts them away too', () => {
    const bloomed = drive([
      { type: 'down', target: vocals, at: onVocals, time: 0 },
      { type: 'tick', time: 400 },
      { type: 'up', time: 450 },
    ]).state;
    const out = drive([{ type: 'down', target: { kind: 'sky' }, at: at(5, 5), time: 900 }], { state: bloomed });
    expect(out.steps[0]).toEqual([{ type: 'bloom', owner: null }]);
  });
});

describe('the FX moons', () => {
  const bloomed = (mix: Mix) =>
    drive(
      [
        { type: 'down', target: vocals, at: onVocals, time: 0 },
        { type: 'tick', time: 400 },
        { type: 'up', time: 450 },
      ],
      { mix },
    ).state;

  test('are dry/wet by their distance along their diagonal', () => {
    const mix = freshMix();
    const reverb = { kind: 'fx', owner: 'vocals', effect: 'reverb' } as const;
    const start = fxCentre(stage, 'vocals', 'reverb', mix);
    const out = drive(
      [
        { type: 'down', target: reverb, at: start, time: 1000 },
        { type: 'move', at: at(start.x - 100, start.y - 100), time: 1030 },
      ],
      { mix, state: bloomed(mix) },
    );
    expect(out.steps[1]).toEqual([{ type: 'fx', owner: 'vocals', effect: 'reverb', wet: 1 }]);
  });

  test('a click wakes a dry one at 45% and steps a wet one to its next character', () => {
    const mix = freshMix();
    const delay = { kind: 'fx', owner: 'vocals', effect: 'delay' } as const;
    const click = (m: Mix) =>
      drive(
        [
          { type: 'down', target: delay, at: fxCentre(stage, 'vocals', 'delay', m), time: 1000 },
          { type: 'up', time: 1050 },
        ],
        { mix: m, state: bloomed(m) },
      ).steps[1];
    expect(click(mix)).toEqual([{ type: 'fx', owner: 'vocals', effect: 'delay', wet: 0.45 }]);
    mix.vocals.fx.delay = 0.6;
    expect(click(mix)).toEqual([{ type: 'fxCharacter', owner: 'vocals', effect: 'delay' }]);
  });
});

describe('keys on a focused moon', () => {
  const key = (k: string, shift = false, stem: 'vocals' | 'bass' = 'vocals') => ({ type: 'key', key: k, shift, stem }) as const;

  test('arrows move the level 5%, M mutes, S solos, Tab goes on in file order', () => {
    const out = drive([key('ArrowUp'), key('ArrowLeft'), key('ArrowLeft'), key('m'), key('S'), key('Tab'), key('Tab', true)]);
    expect(out.steps[0]).toEqual([{ type: 'level', stem: 'vocals', level: expect.closeTo(0.85, 12) }]);
    expect(out.steps[2]).toEqual([{ type: 'level', stem: 'vocals', level: expect.closeTo(0.75, 12) }]);
    expect(out.steps[3]).toEqual([{ type: 'mute', stem: 'vocals', muted: true }]);
    expect(out.steps[4]).toEqual([{ type: 'solo', stem: 'vocals', soloed: true }]);
    expect(out.steps[5]).toEqual([{ type: 'focus', stem: 'drums' }]);
    expect(out.steps[6]).toEqual([{ type: 'focus', stem: null }]);
  });

  test('levels stop at 0 and 1', () => {
    const loud = freshMix();
    loud.vocals.level = 0.98;
    expect(drive([key('ArrowUp')], { mix: loud }).mix.vocals.level).toBe(1);
    const quiet = freshMix();
    quiet.bass.level = 0.02;
    expect(drive([key('ArrowDown', false, 'bass')], { mix: quiet }).mix.bass.level).toBe(0);
  });
});

describe('the Now strip’s stem lights', () => {
  const light = (inputs: GestureInput[]) => drive(inputs, { stage: null });

  test('click to mute, click again within 250 ms to solo, as a moon does', () => {
    const out = light([
      { type: 'down', target: vocals, at: at(22, 22), time: 0 },
      { type: 'up', time: 70 },
      { type: 'down', target: vocals, at: at(22, 22), time: 140 },
      { type: 'up', time: 210 },
    ]);
    expect(out.steps[1]).toEqual([{ type: 'mute', stem: 'vocals', muted: true }]);
    expect(out.mix.vocals).toMatchObject({ muted: false, soloed: true });
  });

  test('have no arm to drag along and no room to bloom', () => {
    const dragged = light([
      { type: 'down', target: vocals, at: at(22, 22), time: 0 },
      { type: 'move', at: at(40, 22), time: 30 },
      { type: 'up', time: 60 },
    ]);
    expect(dragged.steps.flat()).toEqual([]);
    const held = light([
      { type: 'down', target: vocals, at: at(22, 22), time: 0 },
      { type: 'tick', time: 500 },
      { type: 'up', time: 520 },
    ]);
    expect(held.steps.flat()).toEqual([]);
  });
});
