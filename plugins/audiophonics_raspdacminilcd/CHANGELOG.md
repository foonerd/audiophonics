# Changelog

All notable changes to the RaspDacMini LCD plugin are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html); a version numbered x.99.z is a build for the bench of the next major version.

## [1.99.0] - 2026-11-14

A build for the bench of what will be version 2: a rewrite. The display and the remote are now one small native program, in place of a Node application drawing through the kernel's framebuffer, a native add-on per Node version, lirc and irexec.

- **Nothing to rebuild, nothing installed beside the plugin.** The program is a single static binary. It does not depend on Volumio's Node version, and the plugin installs no package.
- **The player's own screens show on the display.** Volumio's start-up and shutdown screens and the progress of a system update are drawn on it by the system, and the plugin draws once they are done; version 1 had masked them. The display's driver is now the kernel's own `fbtft` overlay, which a system update cannot take away, in place of an overlay file the plugin installed.
- **A second way to drive the display**, *Direct*: the plugin's program drives it over SPI and sends only the part of the picture that changed. Nothing of the system's shows on it then. *Display driver* in the settings chooses; a change takes effect when the player has restarted.
- **The same screens**: the cover with the top bar and the track along the bottom, the second page, the SPDIF screen, and a mark at start and at shutdown, now the Audiophonics mark.
- **The page waits for the player to be ready.** The player answers well before it has finished starting. The mark stays up meanwhile, with *Getting ready* under it and three dots that step, and the track's page comes up when the player says it is ready.
- **Level meters**, off unless switched on: two meters on the first page, one a channel, on a scale of sixty decibels, with the level, the peak and a mark that holds the highest peak. They stand upright at the right of the cover, at the left or one at each side, or lie over the track's line, and have a gain of their own. They read evotap, a tap in the player's audio path that every stream passes through unchanged and that is in the path only while the meters are on.
- **A track's line that runs moves a pixel at a time**, fifty times a second.
- **Text in any script**: DejaVu Sans, with Chinese, Japanese and Korean from GNU Unifont on the same line.
- **Covers** in JPEG, PNG, GIF, WebP, BMP and SVG. Each is fetched and decoded by a second process, so a cover that cannot be read costs the cover and nothing else. A cover that fails is tried again; the cover of the track before does not stay up over a track that has none.
- **The remote is read from the kernel's own decoder.** Every key does what it did, with three differences: Option turns repeat over, and shuffle when it is held, where it turned both over at once; Power shuts the player down through Volumio, so every plugin is told first; and the DAC's input is followed when it is changed from elsewhere.
- **Black after** 0 now means never, as its description always said. A saved setting takes effect at once.
- **No listener on the network.** Version 1's display answered commands from anywhere on port 4153; this one opens no port.
- **Installing over version 1** removes its five services, its scripts, its sudoers entry, its overlay and its lines in the boot configuration, gives back the lirc services and the boot splash it had masked, keeps your settings, and starts the new display at once. Restart the player afterwards to finish the update.
