'use strict';

// What the display's program says of its remote, put into the words the
// settings page shows. The program leaves a small file while it runs:
// { receiver, last_key, last_key_at }, the last two once a key has arrived.

// A length of time as a person says it: seconds, then minutes, then hours.
function ago(seconds) {
    const s = Math.max(0, Math.floor(seconds));
    if (s < 90) return s + ' s';
    if (s < 5400) return Math.round(s / 60) + ' min';
    return Math.round(s / 3600) + ' h';
}

// The report for the page: its text, and whether the receiver is being read.
// `strings` carries the page's own sentences, with {{receiver}}, {{key}} and
// {{ago}} where the values go.
function describe(status, now, strings) {
    const receiver = String((status && status.receiver) || '');
    const lines = [strings.receiver.replace('{{receiver}}', receiver)];
    if (status && status.last_key) {
        lines.push(strings.last
            .replace('{{key}}', String(status.last_key))
            .replace('{{ago}}', ago(now - Number(status.last_key_at || now))));
    } else {
        lines.push(strings.none);
    }
    return { text: lines.join(' '), reading: receiver.indexOf('reading ') === 0 && receiver.indexOf('decoder is off') === -1 };
}

module.exports = { ago, describe };
