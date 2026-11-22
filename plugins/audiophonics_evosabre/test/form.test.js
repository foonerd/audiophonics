'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const form = require('../lib/form');
const uiconf = require('../UIConfig.json');
const defaults = require('../config.json');

test('a switch reads from a boolean or its text, and falls back otherwise', () => {
    assert.equal(form.isOn(true, false), true);
    assert.equal(form.isOn('true', false), true);
    assert.equal(form.isOn(false, true), false);
    assert.equal(form.isOn('false', true), false);
    assert.equal(form.isOn(undefined, true), true);
    assert.equal(form.isOn('perhaps', false), false);
});

test('a saved section is checked before anything is stored', () => {
    const keys = ['oled_active', 'contrast', 'clock_after', 'sleep_after', 'deep_sleep_after'];
    const good = form.take({ oled_active: true, contrast: '200', clock_after: 0, sleep_after: 1200, deep_sleep_after: '3600' }, keys);
    assert.deepEqual(good, { values: { oled_active: true, contrast: 200, clock_after: 0, sleep_after: 1200, deep_sleep_after: 3600 } });
    for (const contrast of [0, 256, '1.5', '', 'bright', null]) {
        assert.deepEqual(form.take({ contrast: contrast, clock_after: 5 }, keys), { bad: { key: 'contrast', min: 1, max: 255 } }, String(contrast));
    }
    assert.deepEqual(form.take({ sleep_after: 86401 }, keys), { bad: { key: 'sleep_after', min: 0, max: 86400 } });
    assert.deepEqual(form.take({ contrast: 10, other: 1 }, ['contrast']), { values: { contrast: 10 } }, 'only the keys asked for');
    assert.deepEqual(form.take(undefined, keys), { values: {} });
});

test('a select arrives as an object or as its value, and only a known choice is kept', () => {
    assert.deepEqual(form.take({ log_level: { value: 'info', label: 'Changes' } }, ['log_level']), { values: { log_level: 'info' } });
    assert.deepEqual(form.take({ log_level: 'debug' }, ['log_level']), { values: { log_level: 'debug' } });
    assert.deepEqual(form.take({ log_level: 'loud' }, ['log_level']), { values: {} });
    assert.deepEqual(form.take({ remote_active: 'false' }, ['remote_active']), { values: { remote_active: false } });
});

test('stored values fill the page and a key the store lacks leaves the page its default', () => {
    const page = JSON.parse(JSON.stringify(uiconf));
    const stored = { oled_active: 'false', contrast: 42, log_level: 'debug' };
    form.fill(page, (key) => stored[key]);
    const find = (id) => page.sections.flatMap((s) => s.content).filter((e) => e.id === id)[0];
    assert.equal(find('oled_active').value, false);
    assert.equal(find('contrast').value, 42);
    assert.equal(find('log_level').value.value, 'debug');
    assert.equal(find('log_level').value.label, 'TRANSLATE.EVOSABRE.LOG_DEBUG');
    assert.equal(find('clock_after').value, 6, 'not stored: the default stays');
    assert.equal(find('remote_active').value, true);
});

// What a player has before anything is set: the track with its time at the
// size it has always had, the clock while it rests, and nothing put into
// its audio path.
test('a first install shows the track time at its smallest, and has the meters, the snake and click prevention off', () => {
    assert.equal(defaults.track_time_size.value, 'small');
    assert.equal(defaults.meters.value, false);
    assert.equal(defaults.meters_style.value, 'horizontal');
    assert.equal(defaults.snake.value, false);
    assert.equal(defaults.click_prevention.value, false);
    assert.equal(defaults.oled_active.value, true);
});

test('the track time is saved as a size, and the switch it once was is kept in step', () => {
    for (const size of ['small', 'medium', 'large', 'extra_large']) {
        assert.deepEqual(form.take({ track_time_size: { value: size, label: 'x' } }, ['track_time_size']),
            { values: { track_time_size: size, track_time: true } }, size);
    }
    assert.deepEqual(form.take({ track_time_size: 'off' }, ['track_time_size']), { values: { track_time_size: 'off', track_time: false } });
    assert.deepEqual(form.take({ track_time_size: 'huge' }, ['track_time_size']), { values: {} }, 'a size not known stores nothing');
    assert.deepEqual(form.take({ meters_style: 'horizontal_beside', snake: true }, ['meters_style', 'snake']),
        { values: { meters_style: 'horizontal_beside', snake: true } });
});

test('a store from before the track time had sizes shows its time as it was', () => {
    const page = (store) => {
        const filled = form.fill(JSON.parse(JSON.stringify(uiconf)), form.stored((key) => store[key]));
        return filled.sections.flatMap((s) => s.content).filter((e) => e.id === 'track_time_size')[0].value;
    };
    assert.deepEqual(page({ track_time: false }), { value: 'off', label: 'TRANSLATE.EVOSABRE.TRACK_TIME_OFF' });
    assert.deepEqual(page({ track_time: 'false' }), { value: 'off', label: 'TRANSLATE.EVOSABRE.TRACK_TIME_OFF' });
    assert.deepEqual(page({ track_time: true }), { value: 'small', label: 'TRANSLATE.EVOSABRE.TRACK_TIME_SMALL' });
    assert.equal(page({}).value, 'small', 'neither stored: the default');
    // Once a size is stored it is what the page shows, whatever the switch says.
    assert.equal(page({ track_time: false, track_time_size: 'large' }).value, 'large');
    assert.equal(page({ track_time: true, track_time_size: 'off' }).value, 'off');
    // No other key is read through another.
    const get = form.stored((key) => ({ track_time: false })[key]);
    assert.equal(get('contrast'), undefined);
    assert.equal(get('track_time'), false);
});

test('every choice of a select on the page is one the check keeps, and the reverse', () => {
    const selects = uiconf.sections.flatMap((s) => s.content).filter((e) => e.element === 'select');
    assert.ok(selects.length >= 4);
    for (const select of selects) {
        for (const option of select.options) {
            const kept = form.take({ [select.id]: option }, [select.id]).values[select.id];
            assert.equal(kept, option.value, select.id + ': ' + option.value);
        }
        assert.equal(form.take({ [select.id]: 'no such choice' }, [select.id]).values[select.id], undefined, select.id);
        assert.ok(select.options.some((o) => o.value === select.value.value && o.label === select.value.label), select.id + ': its default is one of its choices');
    }
});

test('the page, the store and the checks agree on every setting', () => {
    const controls = uiconf.sections.flatMap((s) => s.content).filter((e) => e.element !== 'button');
    const ids = controls.map((e) => e.id).sort();
    assert.deepEqual(ids, Object.keys(defaults).sort(), 'every control has a stored default and the reverse');
    for (const control of controls) {
        const stored = defaults[control.id].value;
        const shown = control.element === 'select' ? control.value.value : control.value;
        assert.equal(shown, stored, control.id + ': the page and the store start the same');
        if (control.element === 'input') {
            const [min, max] = form.NUMBERS[control.id];
            const attr = Object.assign({}, ...control.attributes);
            assert.deepEqual([attr.min, attr.max], [min, max], control.id + ': the page and the check share a range');
        }
    }
    for (const section of uiconf.sections) {
        const saved = section.saveButton ? section.saveButton.data.slice().sort() : [];
        const held = section.content.filter((e) => e.element !== 'button').map((e) => e.id).sort();
        assert.deepEqual(saved, held, section.id + ': the save sends every control of its section');
    }
});
