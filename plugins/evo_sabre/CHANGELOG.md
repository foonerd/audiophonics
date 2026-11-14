# Changelog

All notable changes to the Audiophonics Evo Sabre plugin are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.99.8] - 2026-11-14

- **Click prevention no longer fails the first 24-bit track after a 16-bit one.** The program behind it crashed when a stream followed another at the same sample rate in a wider format, as the first track after Volumio's startup sound often is: the player said it could not open the audio output, and playing again worked. The plugin now ships Audio Keepalive 2.1.2, where that is fixed.

## [2.99.7] - 2026-11-14

- **Meters: gain**, from -12 to 12 dB, added to what the level meters show. It moves the meters only; what is played is not touched.
- **Track time**, a switch, on unless you switch it off: the time played, the track's length and the bar between them. Off, the bottom rows are free, and meters lying under the track take them: taller, with room around them.

## [2.99.6] - 2026-11-14

- **Level meters**, a switch, off until you switch it on: two meters beside the track, one a channel, on a scale of sixty decibels. Each shows the level, the reach of the peak, and a mark that holds the highest peak for a moment. They move twenty-five times a second.
- **Meters: where they stand.** Lying under the track, the left channel above the right, where the artist and the album share a line to make room; or upright at the left or the right of the track, which keeps its three lines.
- The meters read evotap, a tap in the player's audio path that every stream passes through unchanged. It is in the path only while the meters are on, and joins it after a restart of the player on a plugin just installed or updated.
- The clock's unlit segments are Very faint unless set otherwise.

## [2.99.5] - 2026-11-14

- **Clock digits: unlit segments** gains Very faint, between Dark and Faint. The display now has two greys under its ordinary scale, one and two ticks of its clock where the ordinary scale starts at eight.

## [2.99.4] - 2026-11-14

- **The clock is easier to read.** The segments of its digits that are not lit were drawn in the faintest grey the display's standard grey scale has, which beside the lit ones was still too bright. The display now has one fainter grey, a quarter of that, and the unlit segments use it.
- **Clock digits: unlit segments**, a new setting: Dark, Faint (as above) or Dim (as before).

## [2.99.3] - 2026-11-14

- **Click prevention plays on a player with Volumio's multiroom in its audio path.** With the switch on, nothing played, the startup sound included: Volumio's multiroom switcher refused the program behind Click prevention as its output. The music now reaches that program through a pass-through the switcher accepts; every sample arrives as it was sent.
- **Switching Click prevention on or off no longer moves the volume or leaves the player showing itself muted.** Volumio restores its stored mixer settings whenever its audio path is rebuilt; the volume and the mute the player had are now put back afterwards.
- An update while Click prevention is on starts its program again, instead of leaving the player silent until it restarts.
- The settings page and the README say how to have this display and the DAC's own show the same volume: Volume Curve Mode on Linear.

## [2.99.2] - 2026-11-14

- **Updating over version 2 no longer leaves the display dark until the player restarts.** Version 2's display took the display's two GPIO lines through `/sys/class/gpio` and did not give them back when it stopped, so the new display was refused them. The installer now releases those two lines, and only those.
- The installer says whether the display stayed up after an update, instead of saying it is running as soon as it has been started.
- A display that is refused a GPIO line says in its journal who holds the line.

## [2.99.1] - 2026-11-14

- **Click prevention**, a switch on the settings page, off until you switch it on. It is for a DAC that clicks, or loses the first moment of a track, when playback starts or resumes. With it on, the audio output is held open so the DAC's clocks never stop: a small program, Audio Keepalive, takes over the output, passes the music through untouched, and sends noise far too quiet to hear while nothing plays. On a Raspberry Pi 5 the DAC is otherwise given its clocks only while something plays. A change of sample rate still reopens the output.
- After the plugin is installed or updated, Click prevention joins the player's audio path once the player has restarted; the plugin asks for the restart when it is needed. Where the Audio Keepalive plugin is already switched on, it is left to do the work.

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
