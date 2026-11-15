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
    const keys = ['lcd_active', 'display_driver', 'boot_splash', 'sleep_after'];
    const good = form.take({ lcd_active: true, boot_splash: 'false', sleep_after: '900' }, keys);
    assert.deepEqual(good, { values: { lcd_active: true, boot_splash: false, sleep_after: 900 } });
    assert.deepEqual(form.take({ sleep_after: 0 }, keys), { values: { sleep_after: 0 } }, 'nought is never, and is taken');
    for (const seconds of [-1, 3601, '1.5', '', 'soon', null]) {
        assert.deepEqual(form.take({ lcd_active: true, sleep_after: seconds }, keys), { bad: { key: 'sleep_after', min: 0, max: 3600 } }, String(seconds));
    }
    assert.deepEqual(form.take({ sleep_after: 10, other: 1 }, ['sleep_after']), { values: { sleep_after: 10 } }, 'only the keys asked for');
    assert.deepEqual(form.take(undefined, keys), { values: {} });
});

test('a select arrives as an object or as its value, and only a known choice is kept', () => {
    assert.deepEqual(form.take({ log_level: { value: 'info', label: 'Changes' } }, ['log_level']), { values: { log_level: 'info' } });
    assert.deepEqual(form.take({ log_level: 'debug' }, ['log_level']), { values: { log_level: 'debug' } });
    assert.deepEqual(form.take({ log_level: 'loud' }, ['log_level']), { values: {} });
    assert.deepEqual(form.take({ remote_active: 'false' }, ['remote_active']), { values: { remote_active: false } });
    assert.deepEqual(form.take({ display_driver: { value: 'spi', label: 'Direct' } }, ['display_driver']), { values: { display_driver: 'spi' } });
    assert.deepEqual(form.take({ display_driver: 'hdmi' }, ['display_driver']), { values: {} });
});

// What the store holds for the driver goes to a helper that runs as root:
// it is one of two words, whatever was stored.
test('the way the display is driven is one of two words, and the framebuffer unless it says otherwise', () => {
    assert.equal(form.driver('spi'), 'spi');
    assert.equal(form.driver('framebuffer'), 'framebuffer');
    for (const odd of [undefined, null, '', 'SPI', 'spi; reboot', { value: 'spi' }, true]) {
        assert.equal(form.driver(odd), 'framebuffer', String(odd));
    }
    assert.equal(defaults.display_driver.value, 'framebuffer');
});

test('stored values fill the page and a key the store lacks leaves the page its default', () => {
    const page = JSON.parse(JSON.stringify(uiconf));
    const stored = { lcd_active: 'false', sleep_after: 42, log_level: 'debug' };
    form.fill(page, (key) => stored[key]);
    const find = (id) => page.sections.flatMap((s) => s.content).filter((e) => e.id === id)[0];
    assert.equal(find('lcd_active').value, false);
    assert.equal(find('sleep_after').value, 42);
    assert.equal(find('log_level').value.value, 'debug');
    assert.equal(find('log_level').value.label, 'TRANSLATE.RASPDACMINI.LOG_DEBUG');
    assert.equal(find('boot_splash').value, true, 'not stored: the default stays');
    assert.equal(find('remote_active').value, true);
});

// The three settings version 1 kept are kept by their names, with the
// defaults it had, so an update finds what the player was set to.
test('the settings of version 1 keep their names and their defaults', () => {
    assert.equal(defaults.lcd_active.value, true);
    assert.equal(defaults.boot_splash.value, true);
    assert.equal(defaults.sleep_after.value, 900);
    assert.deepEqual(form.NUMBERS.sleep_after, [0, 3600]);
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

test('the meters\' settings are taken as the display reads them', () => {
    const keys = ['meters', 'meters_style', 'meters_gain'];
    assert.deepEqual(
        form.take({ meters: true, meters_style: { value: 'horizontal', label: 'Over the track, lying' }, meters_gain: '-6' }, keys),
        { values: { meters: true, meters_style: 'horizontal', meters_gain: -6 } });
    for (const style of ['vertical_right', 'vertical_left', 'horizontal']) {
        assert.deepEqual(form.take({ meters_style: style }, keys), { values: { meters_style: style } });
    }
    assert.deepEqual(form.take({ meters_style: 'diagonal' }, keys), { values: {} }, 'a place that is none is not stored');
    for (const gain of [-13, 13, '0.5', '']) {
        assert.deepEqual(form.take({ meters_gain: gain }, keys), { bad: { key: 'meters_gain', min: -12, max: 12 } }, String(gain));
    }
    // The page offers the places the form takes, and the range it checks.
    const display = uiconf.sections.filter((s) => s.id === 'section_display')[0];
    const style = display.content.filter((e) => e.id === 'meters_style')[0];
    assert.deepEqual(style.options.map((o) => o.value), ['vertical_right', 'vertical_left', 'horizontal']);
    const gain = display.content.filter((e) => e.id === 'meters_gain')[0];
    assert.deepEqual([gain.attributes[0].min, gain.attributes[1].max], form.NUMBERS.meters_gain);
    // Off, at the right, with no gain, until the user says otherwise.
    assert.equal(defaults.meters.value, false);
    assert.equal(defaults.meters_style.value, 'vertical_right');
    assert.equal(defaults.meters_gain.value, 0);
    assert.deepEqual(display.saveButton.data.slice(-3), keys);
});
