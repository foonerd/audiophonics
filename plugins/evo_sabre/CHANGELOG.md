# Changelog

All notable changes to the Audiophonics Evo Sabre plugin are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.99.0] - 2026-11-14

The first build of what will be version 3, for the bench. The 2.99 builds are the rewrite on its way to the store; 3.0.0 will be the first of them to have run on a unit and been accepted there.

The display and the remote are now one small native program, in place of a Node application, a native add-on per Node version, lirc and irexec. Installing over version 2 removes its three services, starts the new display, and keeps the settings; restart the player afterwards to finish the update.

- **Nothing to rebuild.** The program is a single static binary. It does not depend on Volumio's Node version, and the plugin no longer installs lirc or any other package.
- **The same program runs on the Raspberry Pi 4 and 5 families**, the Compute Modules included.
- **The display.** The track's title, artist and album each have their own line and their own grey, with the sample rate and bit depth, the volume, the player's address, and the time played against the track's length. Titles and artists too long for the glass scroll. Repeat and shuffle show beside the volume while they are on.
- **Any script.** Text beyond Latin-1, Chinese, Japanese, Korean, Cyrillic, Greek and the rest, is drawn from GNU Unifont, as before; a line that needs it is drawn whole in it.
- **A clock while the player rests**, paused or stopped, after a time you choose (6 seconds unless changed), with the day and the date.
- **A greeting and a goodbye.** The Audiophonics mark from the moment the program starts until the player answers; a word when the player shuts down or restarts.
- **The display looks after itself.** It dims after a time without a key on the remote and goes dark after a longer one while nothing plays, and the whole picture moves by a pixel once a minute. These take the place of the screensaver. *Screensaver after* is now *Dim after* and *Sleep after* is now *Dark after*; the values you had are kept.
- **Brightness** now has 255 steps, set from the same number as before; the same number is a little brighter than it was.
- **The remote** is read from the kernel's own decoder, which tells a key's frame from the repeats that follow it while the key is held. The same five keys act on the player: play and pause, next, previous, and the two seeks, which step on while held. A key on the remote wakes a dark display. A receiver that is not there when the display starts is looked for again every few seconds.
- **A track change does not disturb the display.** The moment the player reports between two tracks, or a state it is slow to give, neither wakes a dimmed display nor darkens it.
- **Check the remote** on the settings page says whether the receiver is being read and which key arrived last.
- **Settings take effect when saved.** No restart of the display is needed.
- **The display's program runs as the volumio user** and listens on no network port.
- **Uninstalling** removes everything the plugin installed, its lines in the boot configuration included.

Not carried over from version 2: the track's file type in the status row, and the snake screensaver.
