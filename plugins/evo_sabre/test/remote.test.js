'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const remote = require('../lib/remote');
const en = require('../i18n/strings_en.json').EVOSABRE;

const strings = { receiver: en.REMOTE_RECEIVER, last: en.REMOTE_LAST, none: en.REMOTE_NONE };

test('a length of time reads in seconds, then minutes, then hours', () => {
    assert.equal(remote.ago(0), '0 s');
    assert.equal(remote.ago(89), '89 s');
    assert.equal(remote.ago(90), '2 min');
    assert.equal(remote.ago(3600), '60 min');
    assert.equal(remote.ago(7200), '2 h');
    assert.equal(remote.ago(-5), '0 s', 'a clock that stepped back');
});

test('a receiver being read and a key that arrived are both said', () => {
    const report = remote.describe({ receiver: 'reading rc0 at /dev/input/event4', last_key: 'play', last_key_at: 1000 }, 1012, strings);
    assert.equal(report.text, 'Receiver: reading rc0 at /dev/input/event4. Last key: play, 12 s ago.');
    assert.equal(report.reading, true);
});

test('no key yet, a receiver not found, and a decoder that is off', () => {
    const quiet = remote.describe({ receiver: 'reading rc0 at /dev/input/event4' }, 1012, strings);
    assert.equal(quiet.text, 'Receiver: reading rc0 at /dev/input/event4. No key has arrived since the display started.');
    assert.equal(quiet.reading, true);
    const absent = remote.describe({ receiver: 'not found: no infrared receiver' }, 1012, strings);
    assert.equal(absent.reading, false);
    const deaf = remote.describe({ receiver: 'reading rc0 at /dev/input/event4, but its nec decoder is off and could not be switched on: Permission denied' }, 1, strings);
    assert.equal(deaf.reading, false);
    assert.equal(remote.describe({}, 1, strings).reading, false);
});
