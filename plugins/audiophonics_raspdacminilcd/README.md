# RaspDacMini LCD

A Volumio 4 plugin for the Audiophonics RaspDac Mini. It drives the 2.4 inch display on the front and the remote control.

## What it does

**The display** shows the cover of what plays, filling the screen from top to bottom; where a cover does not reach the sides, its own colours, blurred, fill the rest. Two bars lie over it. The one along the top carries what the player is doing, the unit's address, the time, repeat and shuffle, and the volume. The one along the bottom carries the track: its title, its artist and its album on one line, which runs from right to left when it is wider than the glass. A thin line above it fills as the track plays.

Text in any script is drawn: DejaVu Sans has Latin, Greek, Cyrillic and many more, and Chinese, Japanese and Korean come from GNU Unifont on the same line.

**The second page**, reached with the Down key of the remote, keeps the track at the top and lists what is known of the stream: sample rate, bit depth, bit rate, channels, the kind of track, its place in the queue, and the DAC's filter. Down again goes back.

While the DAC takes its other input the screen says which, in large letters. While nothing plays and no key is pressed the screen goes black after a time you choose. The plugin shows the Audiophonics mark while the player starts, and says so when it shuts down or restarts.

**The remote.** The streamer acts on every key:

| Key | Action |
| --- | --- |
| Play | play or pause |
| The keys beside Play | seek back, seek forward; they step on while held |
| Left, Right | previous track, next track |
| OK | stop |
| Volume up, Volume down | the player's volume; they step on while held |
| Mute | mute, and back |
| Input | the DAC's input: the player, or SPDIF |
| Up | the DAC's next filter |
| Down | the second page, and back |
| Option | repeat on or off; held for half a second, shuffle on or off |
| Power | shuts the player down |

## Requirements

- A RaspDac Mini with the 2.4 inch display, on a Raspberry Pi.
- Volumio 4.
- For the Input and Up keys and the DAC line of the second page: the DAC chosen in Volumio's Playback Options as *Audiophonics I-Sabre ES9028Q2M*.

## Installing

Install the plugin from Volumio's plugin store, under System Hardware, and enable it. The first installation switches on the SPI bus and the remote's receiver in the boot configuration: **restart the player once** when the plugin asks.

Installing over version 1 of the plugin replaces it and keeps your settings. Version 1 had the kernel drive the display, and that holds the display until the player restarts: **restart the player** after the update, and the display comes up on the new version. Until then it stays dark.

## Settings

| Setting | What it does |
| --- | --- |
| Display on | Off stops the display's program: the screen stays black and the remote is not read. |
| Mark at start and at shutdown | The Audiophonics mark with a word under it while the player starts, shuts down or restarts. Off leaves the screen black at those times. |
| Black after | Seconds after the last key on the remote, while nothing plays, before the screen goes black. 0 never goes black. |
| Obey the remote | Off leaves every key without effect. |
| Check the remote | Says whether the receiver is being read and which key arrived last. |
| Detail | How much the display's program writes to the journal. |

Settings take effect when saved.

The display has no line to switch its light off, so a black screen still glows a little.

## What it installs

One small native program runs the display and the remote; nothing is compiled on the player, and no package is installed. Outside its own folder, the plugin installs one systemd unit (`raspdacmini-panel.service`), udev rules for the receiver's decoder and its device, a sudoers entry for switching the service, and a marked block of two lines in `/boot/userconfig.txt`. Uninstalling removes them all.

Installing also removes what version 1 installed: its five services, its helper scripts, its sudoers entry, its display overlay and its lines in the boot configuration; and it gives back the system's lirc services and boot splash, which version 1 had masked. The packages version 1 installed are left in place.

Covers are fetched with the player's own `curl`.

## Troubleshooting

The display's program writes to the system journal:

    journalctl -u raspdacmini-panel -f

Set *Detail* to Changes to see each change of the player's state, each key, and the DAC's input and filter as they change.

| What you see | What to look at |
| --- | --- |
| The screen stays dark after installing over version 1 | The player has not restarted since. |
| The screen stays dark after a first installation | The SPI bus is switched on at the next restart. The journal says `/dev/spidev0.0` is not there until then. |
| The remote does nothing | *Check the remote* says whether the receiver is read. The receiver is switched on at the first restart after installing. |
| Input and Up do nothing, and the second page shows no filter | The journal says `no sound card named I-Sabre Q2M DAC`: choose the DAC in Playback Options. |
| No cover | The journal says why once for each cover: the address and what `curl` or the decoder said. |

## Credits

Audiophonics donated the hardware this plugin is developed on. Olivier Schwach wrote the original RaspDac Mini plugin for Audiophonics, and the work started with him; version 2 is a rewrite from the ground up by Just a Nerd.

Source, hardware notes and build: https://github.com/foonerd/audiophonics

## Support this project

The plugin is free and open source, maintained by one developer in his own time. If it has earned a place on your player, you can support its development through [GitHub Sponsors](https://github.com/sponsors/foonerd) or [Ko-fi](https://ko-fi.com/justanerd). Sponsorship is a thank-you, not a purchase: it buys no priority, no features and no support agreement.

## Licence

MIT. Two things shipped with the plugin are separate works under their own licences: DejaVu Sans, as `assets/DejaVuSans.ttf` (see `assets/DEJAVU.txt`), and GNU Unifont, as `assets/unifont.hex` (see `assets/UNIFONT.txt`).
