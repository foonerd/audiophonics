# Audiophonics Evo Sabre

A Volumio 4 plugin for the Audiophonics EVO Sabre. It drives the second display, the one on the right of the front panel, and the transport keys of the remote control.

## What it does

**The display** shows what is playing: the title, the artist and the album, the sample rate and bit depth, the volume, the player's address, and the time played against the track's length. A title too long for the glass scrolls. Text in any script is drawn; Chinese, Japanese, Korean, Cyrillic and Greek come from GNU Unifont.

While the player rests, paused or stopped, a clock takes the place of the track, with the day and the date. Left alone, the display dims, and while nothing plays it then goes dark; a key on the remote or playback starting brings it back. It greets with the Audiophonics mark when it starts and says goodbye when the player shuts down.

**The remote.** The streamer obeys five keys:

| Key | Action |
| --- | --- |
| Play | play or pause |
| Right, Left | next track, previous track |
| Up, Down | seek forward, seek back; they step on while held |

The volume, the mute, the input and the power keys are read by the DAC board itself.

## Requirements

- An EVO Sabre on a Raspberry Pi 4 or 5, or a Compute Module 4 or 5 on a carrier board with the 40-pin header.
- Volumio 4.

## Installing

Install the plugin from Volumio's plugin store, under System Hardware, and enable it. The first installation switches on the SPI bus and the remote's receiver in the boot configuration: **restart the player once** when the plugin asks.

Installing over version 2 of the plugin replaces it; your settings are kept.

## Settings

| Setting | What it does |
| --- | --- |
| Display on | Off stops the display's program: the display stays dark and the transport keys are not read. |
| Brightness | 1 to 255. |
| Clock after | Seconds of rest before the clock replaces the track. 0 keeps the track. |
| Dim after | Seconds after the last key on the remote, or after playback started, before the display dims. 0 never dims. |
| Dark after | Seconds after the last key, while nothing plays, before the display goes dark. 0 never goes dark. |
| Obey the transport keys | Off leaves the remote to the DAC board alone. |
| Check the remote | Says whether the receiver is being read and which key arrived last. |
| Detail | How much the display's program writes to the journal. |

Settings take effect when saved.

## Setting up the DAC

In Playback Options, switch I2S DAC on and choose **Audiophonics I-Sabre ES9028Q2M**, then restart the player when asked. Set DSD Playback Mode to **DSD over PCM (DoP)**. Under Volume Options, set Mixer Type to **Hardware** and Mixer Control Name to **Digital**.

The DAC board switches the power itself: wait for the player to finish shutting down before switching the unit off.

## If something is wrong

The display's program is a service, `evosabre-panel`. Set Detail to Changes on the settings page, then:

    journalctl -u evosabre-panel -f

It says which board it found, how it opened the display, what the player is doing, and each key that arrives from the remote.

**The display stays dark after installing.** Restart the player: the SPI bus is switched on in the boot configuration, which is read at start.

**The remote does nothing.** Press a key, then Check the remote on the settings page. *Not found* means the receiver's overlay is not loaded: restart the player. A receiver that is read but reports no key has not had its decoder switched on:

    cat /sys/class/rc/rc*/protocols

`[nec]` in square brackets is what it needs to show.

## What it installs

Outside its own folder, the plugin installs a systemd unit (`evosabre-panel.service`), a udev rule for the receiver's decoder, a sudoers entry for switching the service, and a marked block of two lines in `/boot/userconfig.txt`. Uninstalling removes all four.

## Credits

Audiophonics donated the hardware this plugin is developed and tested on. Olivier Schwach wrote the original EVO Sabre plugin for Audiophonics, and the work started with him; version 3 is a rewrite from the ground up by Just a Nerd.

Source, hardware notes and build: https://github.com/foonerd/audiophonics

## Support this project

The plugin is free and open source, maintained by one developer in his own time. If it has earned a place on your player, you can support its development through [GitHub Sponsors](https://github.com/sponsors/foonerd) or [Ko-fi](https://ko-fi.com/justanerd). Sponsorship is a thank-you, not a purchase: it buys no priority, no features and no support agreement.

## Licence

MIT. GNU Unifont, shipped as `assets/unifont.hex`, is under its own licence; see `assets/UNIFONT.txt`.
