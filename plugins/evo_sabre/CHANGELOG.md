# Changelog

All notable changes to the Audiophonics Evo Sabre plugin are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [3.0.0] - 2026-11-14

Version 3 is a rewrite. The display and the remote are now one small native program, in place of a Node application, a native add-on per Node version, lirc and irexec. Installing over version 2 removes its three services, releases the GPIO lines its display left taken, starts the new display and keeps the settings; restart the player afterwards to finish the update.

- **Nothing to rebuild.** The program is a single static binary. It does not depend on Volumio's Node version, and the plugin no longer installs lirc or any other package.
- **The same program runs on the Raspberry Pi 4 and 5 families**, the Compute Modules included.
- **The display.** The track's title, artist and album each have their own line and their own grey, with the sample rate and bit depth, the volume, the player's address, and the time played against the track's length. Titles and artists too long for the glass scroll. Repeat and shuffle show beside the volume while they are on.
- **Level meters**, a switch, off until you switch it on: two meters beside the track, one a channel, on a scale of sixty decibels. Each shows the level, the reach of the peak, and a mark that holds the highest peak for a moment. They lie under the track or stand upright at its left or its right, and have a gain of their own, from -12 to 12 dB, that moves the meters and not the music. They read evotap, a tap in the player's audio path that every stream passes through unchanged; it is in the path only while the meters are on.
- **Track time**, a switch, on unless you switch it off: the time played, the track's length and the bar between them. Off, the bottom rows are free, and meters lying under the track are drawn taller in them.
- **Any script.** Text beyond Latin-1, Chinese, Japanese, Korean, Cyrillic, Greek and the rest, is drawn from GNU Unifont, as before; a line that needs it is drawn whole in it.
- **A clock while the player rests**, paused or stopped, after a time you choose (6 seconds unless changed), with the day and the date. The segments of its digits that are not lit can be left dark or shown in one of three faint greys.
- **A greeting and a goodbye.** The Audiophonics mark from the moment the program starts until the player answers; a word when the player shuts down or restarts.
- **The display looks after itself.** It dims after a time without a key on the remote and goes dark after a longer one while nothing plays, and the whole picture moves by a pixel once a minute. These take the place of the screensaver. *Screensaver after* is now *Dim after* and *Sleep after* is now *Dark after*; the values you had are kept.
- **Brightness** now has 255 steps, set from the same number as before; the same number is a little brighter than it was.
- **The remote** is read from the kernel's own decoder, which tells a key's frame from the repeats that follow it while the key is held. The same five keys act on the player: play and pause, next, previous, and the two seeks, which step on while held. A key on the remote wakes a dark display. A receiver that is not there when the display starts is looked for again every few seconds.
- **Click prevention**, a switch, off until you switch it on, for a DAC that clicks, or loses the first moment of a track, when playback starts or resumes. With it on, the audio output is held open so the DAC's clocks never stop: a small program, Audio Keepalive, takes over the output, passes the music through untouched, and sends noise far too quiet to hear while nothing plays. On a Raspberry Pi 5 the DAC is otherwise given its clocks only while something plays. A change of sample rate still reopens the output. Where the Audio Keepalive plugin is already switched on, it is left to do the work.
- **Changes to the audio path keep clear of the DAC's clocks.** On the Raspberry Pi 5 family the DAC board can lose a volume or a mute written while its audio clocks stop or start, and enough of them leave it deaf until its power is taken away. When Click prevention or the meters are switched, the plugin now rebuilds the audio path with nothing of its own holding the output, puts the volume back a moment later and opens the output a moment after that. The program behind Click prevention idles at the rate of Volumio's startup sound and waits before it reopens the output for a changed configuration.
- **A track change does not disturb the display.** The moment the player reports between two tracks, or a state it is slow to give, neither wakes a dimmed display nor darkens it.
- **Check the remote** on the settings page says whether the receiver is being read and which key arrived last.
- **Settings take effect when saved.** No restart of the display is needed.
- **The settings page is in eleven languages:** English, German, Spanish, French, Italian, Japanese, Dutch, Polish, Portuguese, Russian and Chinese.
- **The display's program runs as the volumio user** and listens on no network port.
- **Uninstalling** removes everything the plugin installed, its lines in the boot configuration included.

Not carried over from version 2: the track's file type in the status row, and the snake screensaver.

The 2.99 pre-releases were this version on its way here; what each changed is in its own release notes.
