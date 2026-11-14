'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const Module = require('node:module');

// The plugin's file needs three of Volumio's modules, which a player has
// and this test does not: each is stood in for with the little of it the
// plugin touches here.
const kew = {
    resolve: (value) => ({ state: 'resolved', value: value }),
    reject: (error) => ({ state: 'rejected', error: error }),
    defer: () => ({ promise: {}, resolve() {}, reject() {} })
};
const load = Module._load;
Module._load = function (request) {
    if (request === 'kew') return kew;
    if (request === 'fs-extra') return require('node:fs');
    if (request === 'v-conf') return function () {};
    return load.apply(this, arguments);
};
const RaspDacMini = require('../index.js');
Module._load = load;

function plugin() {
    const said = [];
    const asked = [];
    const context = {
        coreCommand: { shutdown: () => asked.push('shutdown') },
        logger: { info: (line) => said.push(line), error: (line) => said.push(line) },
        configManager: {}
    };
    return { plugin: new RaspDacMini(context), said: said, asked: asked };
}

test('the display\'s program can ask the player for a shutdown and for nothing else', () => {
    const p = plugin();
    const done = p.plugin.panelAsks('shutdown');
    assert.equal(done.state, 'resolved');
    assert.deepEqual(p.asked, ['shutdown']);

    for (const word of ['reboot', '', undefined, 'shutdown; rm -rf /', { ask: 'shutdown' }]) {
        const q = plugin();
        const refused = q.plugin.panelAsks(word);
        assert.equal(refused.state, 'rejected', String(word));
        assert.deepEqual(q.asked, [], String(word));
        assert.equal(q.said.length, 1);
    }
});
