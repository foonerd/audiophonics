'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const sequence = require('../lib/sequence');

const writes = (steps) => steps.filter((s) => s.step === 'volume');
const start = (steps) => steps.filter((s) => s.step === 'start')[0];

test('a rebuild that changed nothing restored nothing: the keeper starts at once and no volume is asked for', () => {
    assert.deepEqual(sequence.after(false, { volume: 35, mute: false }, true), [{ at: 0, step: 'start' }]);
    assert.deepEqual(sequence.after(false, { volume: 35, mute: false }, false), []);
});

test('after a rebuild that changed the path the volume waits for quiet, and the keeper for quiet after the volume', () => {
    const steps = sequence.after(true, { volume: 35, mute: false }, true);
    assert.deepEqual(steps, [
        { at: sequence.SETTLE, step: 'volume', value: 35 },
        { at: 2 * sequence.SETTLE, step: 'start' }
    ]);
});

test('a player that was muted gets its volume, then its mute, then the keeper', () => {
    const steps = sequence.after(true, { volume: 35, mute: true }, true);
    assert.deepEqual(steps.map((s) => s.step === 'volume' ? s.value : s.step), [35, 'mute', 'start']);
    assert.equal(steps[1].at - steps[0].at, sequence.MUTE_AFTER);
});

// The rule the whole order exists for.
test('no write to the DAC board is closer to the output being opened than the quiet asked for', () => {
    for (const was of [null, { volume: 0, mute: false }, { volume: 80, mute: false }, { volume: 80, mute: true }]) {
        for (const keeper of [true, false]) {
            const steps = sequence.after(true, was, keeper);
            const opened = start(steps);
            assert.equal(Boolean(opened), keeper);
            for (const w of writes(steps)) {
                assert.ok(w.at >= sequence.SETTLE, 'the volume waits after the restore of the mixer');
                if (opened) assert.ok(opened.at - w.at >= sequence.SETTLE, 'the keeper waits after the volume');
            }
            if (opened) assert.ok(opened.at >= sequence.SETTLE, 'the keeper waits after the restore of the mixer');
            assert.deepEqual(steps.map((s) => s.at), steps.map((s) => s.at).slice().sort((a, b) => a - b), 'in order');
        }
    }
});

test('with no volume to put back the keeper still waits out the restore', () => {
    assert.deepEqual(sequence.after(true, null, true), [{ at: sequence.SETTLE, step: 'start' }]);
    assert.deepEqual(sequence.after(true, null, false), []);
});
