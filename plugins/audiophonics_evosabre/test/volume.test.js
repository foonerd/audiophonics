'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const volume = require('../lib/volume');

test('the volume and the mute of a player state are held', () => {
    assert.deepEqual(volume.held({ volume: 17, mute: false }), { volume: 17, mute: false });
    assert.deepEqual(volume.held({ volume: '65', mute: true }), { volume: 65, mute: true });
    assert.deepEqual(volume.held({ volume: 0, mute: null }), { volume: 0, mute: false });
});

test('nothing is held where the player has no volume to put back', () => {
    assert.equal(volume.held(undefined), null);
    assert.equal(volume.held({}), null);
    assert.equal(volume.held({ volume: null }), null);
    assert.equal(volume.held({ volume: '' }), null);
    assert.equal(volume.held({ volume: 'loud' }), null);
    assert.equal(volume.held({ volume: 140 }), null);
    assert.equal(volume.held({ volume: 100, disableVolumeControl: true }), null);
});

test('the volume is asked for first, and the mute only where the player was muted', () => {
    assert.deepEqual(volume.steps({ volume: 17, mute: false }), [17]);
    assert.deepEqual(volume.steps({ volume: 17, mute: true }), [17, 'mute']);
    assert.deepEqual(volume.steps(null), []);
});
