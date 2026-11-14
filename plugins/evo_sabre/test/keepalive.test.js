'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('fs');
const path = require('path');
const keepalive = require('../lib/keepalive');

test('the contribution names the ALSA client by its path and the daemon by its socket', () => {
    const text = keepalive.contribution('/data/plugins/system_hardware/audiophonics_evo_sabre');
    assert.equal(text, [
        'pcm_type.keepalive {',
        '    lib "/data/plugins/system_hardware/audiophonics_evo_sabre/keepalive/libasound_module_pcm_keepalive.so"',
        '}',
        '',
        'pcm.evosabreKeepalive {',
        '    type copy',
        '    slave.pcm "evosabreKeepaliveClient"',
        '}',
        '',
        'pcm.evosabreKeepaliveClient {',
        '    type keepalive',
        '    socket "/run/evosabre-keepalive/ctl.sock"',
        '}',
        ''
    ].join('\n'));
    assert.equal(keepalive.CONTRIBUTION, 'evosabreKeepalive.evosabreKeepaliveOut.-1.conf');
});

// Volumio's multiroom switcher refuses a target whose hardware parameters
// ALSA reports as narrowed, which it does for the client itself.
test('the players reach the client through a copy PCM, never directly', () => {
    const text = keepalive.contribution('/p');
    const first = text.slice(text.indexOf('pcm.' + keepalive.IN + ' {'));
    assert.match(first, /^pcm\.evosabreKeepalive \{\n {4}type copy\n {4}slave\.pcm "evosabreKeepaliveClient"\n\}/);
    assert.notEqual(keepalive.CLIENT, keepalive.IN);
    assert.notEqual(keepalive.CLIENT, keepalive.OUT);
});

test('the daemon is started on the PCM and the socket the contribution names', () => {
    const unit = fs.readFileSync(path.join(__dirname, '..', 'systemd', 'evosabre-keepalive.service'), 'utf8');
    const start = unit.split('\n').filter((line) => line.indexOf('ExecStart=') === 0)[0];
    assert.ok(start.indexOf(' --pcm ' + keepalive.OUT + ' ') !== -1, start);
    assert.ok(start.endsWith(' --socket ' + keepalive.SOCKET), start);
    assert.ok(start.indexOf('/audiophonics_evo_sabre/keepalive/audio-keepalive-daemon ') !== -1, start);
});

test('the chain carries the contribution only once Volumio has written it in', () => {
    const chain = 'pcm.volumio {\n    slave.pcm "evosabreKeepalive"\n}\n' + keepalive.contribution('/p') + 'pcm.evosabreKeepaliveOut {\n';
    assert.equal(keepalive.inChain(chain), true);
    assert.equal(keepalive.inChain('pcm.volumio {\n    slave.pcm "volumioOutput"\n}\n'), false);
    assert.equal(keepalive.inChain(undefined), false);
    assert.equal(keepalive.inChain('pcm.evosabreKeepaliveClient {\n    type keepalive\n}\n'), false, 'the client alone is not the chain');
});

test('its names are its own, apart from the Audio Keepalive plugin', () => {
    assert.ok(keepalive.STANDALONE.indexOf('/audio_keepalive/asound/keepaliveProxy.') !== -1);
    assert.notEqual(keepalive.IN, 'keepaliveProxy');
    assert.notEqual(keepalive.SOCKET, '/run/audio-keepalive/ctl.sock');
});
