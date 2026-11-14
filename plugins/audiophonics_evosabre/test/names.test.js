'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const here = path.join(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(here, file), 'utf8');
const pkg = JSON.parse(read('package.json'));

// The name the plugin store lists the plugin under, and the folder a player
// installs it in. A player that has version 2 updates to this one only while
// both are what version 2 had.
const STORE_NAME = 'audiophonics_evo_sabre';
const CATEGORY = 'system_hardware';
const INSTALLED = '/data/plugins/' + CATEGORY + '/' + STORE_NAME + '/';

test('the plugin keeps the name and the category the store has it under', () => {
    assert.equal(pkg.name, STORE_NAME);
    assert.equal(pkg.volumio_info.plugin_type, CATEGORY);
});

test('the settings page calls the plugin by that name', () => {
    const endpoints = read('UIConfig.json').match(/"endpoint":\s*"[^"]*"/g) || [];
    assert.ok(endpoints.length > 0);
    endpoints.forEach((endpoint) => {
        assert.ok(endpoint.indexOf('"' + CATEGORY + '/' + STORE_NAME + '"') !== -1, endpoint);
    });
});

test('what runs from the plugin\'s folder on a player names that folder', () => {
    ['systemd/evosabre-panel.service', 'systemd/evosabre-keepalive.service'].forEach((unit) => {
        const start = read(unit).split('\n').find((line) => line.indexOf('ExecStart=') === 0);
        assert.ok(start && start.indexOf('ExecStart=' + INSTALLED) === 0, unit + ': ' + start);
    });
});

// The folder this plugin is kept in is its folder in Volumio's plugin
// sources, which is not the store's name for it.
test('the plugin is kept in the folder Volumio\'s plugin sources have it in', () => {
    assert.equal(path.basename(here), 'audiophonics_evosabre');
});
