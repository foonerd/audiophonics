'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const Module = require('node:module');
const path = require('node:path');
const tap = require('../lib/tap');

// The plugin's file needs three of Volumio's modules, which a player has
// and this test does not. Promises are stood in for by the little of kew
// the plugin uses, the store by nothing, and the files by a player's that
// holds only what the plugin's audio path reads and writes.
function wrap(promise) {
    return {
        then: (f) => wrap(promise.then(f)),
        fail: (f) => wrap(promise.catch(f)),
        native: promise
    };
}
const kew = {
    resolve: (value) => wrap(Promise.resolve(value)),
    reject: (error) => wrap(Promise.reject(error)),
    defer: () => {
        const d = {};
        d.promise = wrap(new Promise((resolve, reject) => { d.resolve = resolve; d.reject = reject; }));
        return d;
    }
};
let files;
const disk = {
    existsSync: (file) => files.has(file),
    readFileSync: (file) => {
        if (!files.has(file)) throw new Error('ENOENT: ' + file);
        return files.get(file);
    },
    writeFileSync: (file, text) => { files.set(file, text); },
    unlinkSync: (file) => { files.delete(file); },
    ensureDirSync: () => {}
};
const load = Module._load;
Module._load = function (request) {
    if (request === 'kew') return kew;
    if (request === 'fs-extra') return disk;
    if (request === 'v-conf') return function () {};
    return load.apply(this, arguments);
};
const RaspDacMini = require('../index.js');
Module._load = load;

const PLUGIN_DIR = path.join(__dirname, '..');
const CONTRIBUTION = path.join(PLUGIN_DIR, 'asound', tap.CONTRIBUTION);
const ASOUND = '/etc/asound.conf';
const strings = require('../i18n/strings_en.json').RASPDACMINI;

// A player: whether the meters are on, the state it reports, whether a
// rebuild of its ALSA configuration writes a new one, and whether it takes
// the tap into the chain when it does.
function standIn(options) {
    files = new Map(options.files || []);
    const seen = { rebuilds: 0, volumes: [], modals: [], toasts: [] };
    const context = {
        coreCommand: {
            getI18nString: (key) => (key.indexOf('RASPDACMINI.') === 0 ? strings[key.slice(12)] : key),
            executeOnPlugin: (type, name, method) => {
                assert.deepEqual([type, name, method], ['audio_interface', 'alsa_controller', 'updateALSAConfigFile']);
                seen.rebuilds += 1;
                if (options.joins !== false && files.has(CONTRIBUTION)) {
                    files.set(ASOUND, 'pcm.volumio {\n    slave.pcm "' + tap.IN + '"\n}\n' + files.get(CONTRIBUTION));
                } else {
                    files.set(ASOUND, 'pcm.volumio {\n}\n');
                }
                return kew.resolve({ changed: options.rewrites !== false });
            },
            volumioGetState: () => options.state || { volume: 35, mute: false },
            volumiosetvolume: (value) => seen.volumes.push(value),
            broadcastMessage: (name, message) => seen.modals.push(message.message),
            pushToastMessage: (type, title, message) => seen.toasts.push(type + ': ' + message)
        },
        logger: { info: () => {}, error: () => {} },
        configManager: {}
    };
    const plugin = new RaspDacMini(context);
    plugin.config = { get: (key) => (key === 'meters' ? options.meters : undefined) };
    return { plugin: plugin, seen: seen };
}

test('meters switched on put the tap into the player\'s audio path, once', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const p = standIn({ meters: true });
    await p.plugin.applyAudioPath().native;
    assert.equal(files.get(CONTRIBUTION), tap.contribution(PLUGIN_DIR));
    assert.equal(p.seen.rebuilds, 1);

    // The volume the player had is asked for again after a quiet moment.
    assert.deepEqual(p.seen.volumes, []);
    t.mock.timers.tick(1500);
    assert.deepEqual(p.seen.volumes, [35]);
    // The chain carries the tap: nothing is asked of the user.
    t.mock.timers.tick(5000);
    assert.deepEqual(p.seen.modals, []);

    // As the settings stand already: nothing is rebuilt.
    await p.plugin.applyAudioPath().native;
    assert.equal(p.seen.rebuilds, 1);
});

test('a player that was muted is muted again, after its volume', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const p = standIn({ meters: true, state: { volume: '40', mute: true } });
    await p.plugin.applyAudioPath().native;
    t.mock.timers.tick(1500);
    assert.deepEqual(p.seen.volumes, [40]);
    t.mock.timers.tick(1000);
    assert.deepEqual(p.seen.volumes, [40, 'mute']);
});

test('the volume is left alone where the rebuild wrote nothing new, or the player has none', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const same = standIn({ meters: true, rewrites: false });
    await same.plugin.applyAudioPath().native;
    t.mock.timers.tick(10000);
    assert.deepEqual(same.seen.volumes, []);

    const fixed = standIn({ meters: true, state: { volume: 100, disableVolumeControl: true } });
    await fixed.plugin.applyAudioPath().native;
    t.mock.timers.tick(10000);
    assert.deepEqual(fixed.seen.volumes, []);
});

test('a player that does not take the tap in until it restarts is asked to restart', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const p = standIn({ meters: true, joins: false });
    await p.plugin.applyAudioPath().native;
    t.mock.timers.tick(4999);
    assert.deepEqual(p.seen.modals, []);
    t.mock.timers.tick(1);
    assert.deepEqual(p.seen.modals, [strings.PATH_RESTART_MSG]);
});

test('meters switched off take the tap out again', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const p = standIn({ meters: false, files: [[CONTRIBUTION, tap.contribution(PLUGIN_DIR)]] });
    await p.plugin.applyAudioPath().native;
    assert.equal(files.has(CONTRIBUTION), false);
    assert.equal(p.seen.rebuilds, 1);
    t.mock.timers.tick(10000);
    assert.deepEqual(p.seen.modals, [], 'nothing to look for in the chain');

    // Off and not there: nothing to do.
    const q = standIn({ meters: false });
    await q.plugin.applyAudioPath().native;
    assert.equal(q.seen.rebuilds, 0);
});

test('a plugin that stops leaves the audio path, whatever the settings say', async (t) => {
    t.mock.timers.enable({ apis: ['setTimeout'] });
    const p = standIn({ meters: true, files: [[CONTRIBUTION, tap.contribution(PLUGIN_DIR)]] });
    await p.plugin.leaveAudioPath().native;
    assert.equal(files.has(CONTRIBUTION), false);
    assert.equal(p.seen.rebuilds, 1);
    t.mock.timers.tick(1500);
    assert.deepEqual(p.seen.volumes, [35]);

    const q = standIn({ meters: true });
    await q.plugin.leaveAudioPath().native;
    assert.equal(q.seen.rebuilds, 0);
});

test('a contribution that cannot be written is said, and nothing is rebuilt', async () => {
    const p = standIn({ meters: true });
    disk.writeFileSync = () => { throw new Error('read-only file system'); };
    try {
        await p.plugin.applyAudioPath().native;
    } finally {
        disk.writeFileSync = (file, text) => { files.set(file, text); };
    }
    assert.equal(p.seen.rebuilds, 0);
    assert.deepEqual(p.seen.toasts, ['error: ' + strings.PATH_FAILED.replace('{{error}}', 'read-only file system')]);
});
