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
