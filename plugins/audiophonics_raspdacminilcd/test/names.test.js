'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const here = path.join(__dirname, '..');
const read = (file) => fs.readFileSync(path.join(here, file), 'utf8');
const pkg = JSON.parse(read('package.json'));

// The name the plugin store lists the plugin under, and the folder a player
// installs it in. A player that has version 1 updates to this one only while
// both are what version 1 had.
const STORE_NAME = 'raspdac_mini_lcd';
const CATEGORY = 'system_hardware';
const INSTALLED = '/data/plugins/' + CATEGORY + '/' + STORE_NAME + '/';

test('the plugin keeps the name, the category and the title the store has it under', () => {
    assert.equal(pkg.name, STORE_NAME);
    assert.equal(pkg.volumio_info.plugin_type, CATEGORY);
    assert.equal(pkg.volumio_info.prettyName, 'RaspDacMini LCD');
});

test('the settings page calls the plugin by that name', () => {
    const endpoints = read('UIConfig.json').match(/"endpoint":\s*"[^"]*"/g) || [];
    assert.ok(endpoints.length > 0);
    endpoints.forEach((endpoint) => {
        assert.ok(endpoint.indexOf('"' + CATEGORY + '/' + STORE_NAME + '"') !== -1, endpoint);
    });
});

test('what runs from the plugin\'s folder on a player names that folder', () => {
    const start = read('systemd/raspdacmini-panel.service').split('\n').find((line) => line.indexOf('ExecStart=') === 0);
    assert.ok(start && start.indexOf('ExecStart=' + INSTALLED) === 0, start);
    assert.ok(read('install.sh').indexOf('PLUGIN_DIR="' + INSTALLED.slice(0, -1) + '"') !== -1);
});

// The display's program asks the player for a shutdown through this plugin,
// by the name the plugin registers; the two have to be the same word.
test('the plugin answers the display\'s program under the name it calls', () => {
    const index = read('index.js');
    assert.ok(index.indexOf("endpoint: '" + STORE_NAME + "'") !== -1);
    assert.ok(index.indexOf("name: '" + STORE_NAME + "'") !== -1);
    assert.ok(index.indexOf("method: 'panelAsks'") !== -1);
    assert.ok(/RaspDacMini\.prototype\.panelAsks = function/.test(index));
});

// The folder this plugin is kept in is its folder in Volumio's plugin
// sources, which is not the store's name for it.
test('the plugin is kept in the folder Volumio\'s plugin sources have it in', () => {
    assert.equal(path.basename(here), 'audiophonics_raspdacminilcd');
});

// As the service is stopped it runs a helper, as root, that tells the
// display's program when the stop is the system going down. The helper is
// where the installer puts it, out of the plugin's own folder, which is
// not root's.
test('the service tells its program when the system is going down, with a helper the installer installs', () => {
    const unit = read('systemd/raspdacmini-panel.service');
    const stop = unit.split('\n').filter((line) => line.indexOf('ExecStop=') === 0);
    assert.deepEqual(stop, ['ExecStop=+/usr/local/bin/raspdacmini-leaving $MAINPID']);
    const installer = read('install.sh');
    assert.ok(installer.indexOf('LEAVING_HELPER="/usr/local/bin/raspdacmini-leaving"') !== -1);
    assert.ok(installer.indexOf('install -m 755 "$PLUGIN_DIR/boot/raspdacmini-leaving" "$LEAVING_HELPER"') !== -1);
    assert.ok(read('uninstall.sh').indexOf('/usr/local/bin/raspdacmini-leaving') !== -1);
});
