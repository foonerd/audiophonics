'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const tap = require('../lib/tap');

test('the contribution names the tap by its path and its ring by the plugin\'s tag', () => {
    const text = tap.contribution('/data/plugins/system_hardware/raspdac_mini_lcd');
    assert.equal(text, [
        'pcm_type.evotap {',
        '    lib "/data/plugins/system_hardware/raspdac_mini_lcd/evotap/libevotap.so"',
        '}',
        '',
        'pcm.raspdacminiTap {',
        '    type evotap',
        '    slave.pcm "raspdacminiTapOut"',
        '    ring "raspdacmini"',
        '}',
        ''
    ].join('\n'));
});

// Volumio orders contributions by rank, highest first; one without a rank,
// as the player's own volume has, stands at 0.
test('the tap stands ahead of the volume', () => {
    assert.equal(tap.CONTRIBUTION, 'raspdacminiTap.raspdacminiTapOut.4.conf');
    assert.ok(tap.RANK > 0);
});

test('the chain carries the tap once Volumio has written it in', () => {
    const chain = 'pcm.volumio {\n    slave.pcm "raspdacminiTap"\n}\n' + tap.contribution('/p') + 'pcm.raspdacminiTapOut {\n';
    assert.equal(tap.inChain(chain), true);
    assert.equal(tap.inChain('pcm.raspdacminiTapOut {\n}\n'), false);
    assert.equal(tap.inChain(undefined), false);
});

// The display looks for rings of this tag: the panel's program names it.
test('the tag is the one the display reads', () => {
    const program = fs.readFileSync(path.join(__dirname, '..', '..', '..', 'bins', 'raspdacmini-panel', 'src', 'main.rs'), 'utf8');
    assert.ok(program.indexOf('const TAP_TAG: &str = "' + tap.TAG + '";') !== -1);
});

// Two plugins of this house can stand in one chain: each has its own PCMs
// and its own tag.
test('nothing of the tap is named as the EVO Sabre plugin names its own', () => {
    const other = require('../../audiophonics_evosabre/lib/tap');
    assert.notEqual(tap.IN, other.IN);
    assert.notEqual(tap.OUT, other.OUT);
    assert.notEqual(tap.TAG, other.TAG);
});
