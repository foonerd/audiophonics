'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const tap = require('../lib/tap');
const keepalive = require('../lib/keepalive');

test('the contribution names the tap by its path and its ring by the plugin\'s tag', () => {
    const text = tap.contribution('/data/plugins/system_hardware/audiophonics_evo_sabre');
    assert.equal(text, [
        'pcm_type.evotap {',
        '    lib "/data/plugins/system_hardware/audiophonics_evo_sabre/evotap/libevotap.so"',
        '}',
        '',
        'pcm.evosabreTap {',
        '    type evotap',
        '    slave.pcm "evosabreTapOut"',
        '    ring "evosabre"',
        '}',
        ''
    ].join('\n'));
});

// Volumio orders contributions by rank, highest first; one without a rank,
// as the player's own volume has, stands at 0.
test('the tap stands ahead of the volume and of the keeper, which is last', () => {
    assert.equal(tap.CONTRIBUTION, 'evosabreTap.evosabreTapOut.4.conf');
    assert.ok(tap.RANK > 0);
    const keeper = Number(keepalive.CONTRIBUTION.split('.')[2]);
    assert.ok(tap.RANK > keeper);
});

test('the chain carries the tap once Volumio has written it in', () => {
    const chain = 'pcm.volumio {\n    slave.pcm "evosabreTap"\n}\n' + tap.contribution('/p') + 'pcm.evosabreTapOut {\n';
    assert.equal(tap.inChain(chain), true);
    assert.equal(tap.inChain('pcm.evosabreTapOut {\n}\n'), false);
    assert.equal(tap.inChain(undefined), false);
});

// The display looks for rings of this tag; see the panel's levels module.
test('the tag is the one the display reads', () => {
    const fs = require('fs');
    const path = require('path');
    const levels = fs.readFileSync(path.join(__dirname, '..', '..', '..', 'bins', 'evosabre-panel', 'src', 'levels.rs'), 'utf8');
    assert.ok(levels.indexOf('pub const TAG: &str = "' + tap.TAG + '";') !== -1);
});
