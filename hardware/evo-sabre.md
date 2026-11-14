# EVO Sabre

A streamer built around a Raspberry Pi, a dual ES9038Q2M DAC board with its own microcontroller, two OLED displays and an infrared receiver.

## What the Raspberry Pi drives

| Part | Connection | Notes |
| --- | --- | --- |
| Second display (the right-hand one) | SPI0, CE0 (`/dev/spidev0.0`), mode 0, 4.8 MHz | SSD1322, 256 x 64, sixteen greys, 4-wire SPI |
| Display D/C | GPIO 27 | |
| Display reset | GPIO 24 | active low |
| Infrared receiver | GPIO 4 | `dtoverlay=gpio-ir,gpio_pin=4` |
| DAC | I2S, and I2C address 0x48 | `dtoverlay=i-sabre-q2m`; ALSA card "I-Sabre Q2M DAC" |

The first display, on the left, belongs to the DAC board's microcontroller. So do the volume, the mute, the input selection and the power: the microcontroller reads the same infrared receiver for those keys and acts on them itself.

## Boot configuration

    dtparam=spi=on
    dtoverlay=gpio-ir,gpio_pin=4

The plugin writes these into its own block of `/boot/userconfig.txt`. The DAC's overlay is written by Volumio when the DAC is chosen in Playback Options, as "Audiophonics I-Sabre ES9028Q2M".

## The display

The SSD1322's RAM is 480 columns of four pixels; the 256-pixel glass sits in the middle of it, from column address 0x1C to 0x5B. A frame is 8192 bytes, two pixels a byte, the left pixel in the high nibble, written whole.

Brightness is the contrast current (command 0xC1, 0 to 255) with the master current (0xC7) at full.

## The DAC's clocks

The DAC takes its bit clock and frame clock from the Raspberry Pi, and has to lock on to them before it can play.

| | Raspberry Pi 4, Compute Module 4 | Raspberry Pi 5, Compute Module 5 |
| --- | --- | --- |
| The clocks start | when a stream is set up, before its first sample | with the stream's first sample |
| On pause | they keep running | they stop |
| The clocks stop | when the output is closed | when the output is closed |

So on the Pi 5 family the DAC has no clocks between tracks unless something holds the output open. The plugin's Click prevention does that: with it on, the clocks run without a break on either family, except across a change of sample rate, which reopens the output.

## The DAC board's microcontroller

The player sets the DAC through the board's microcontroller, over I2C at address 0x48. The kernel's `i-sabre-codec` driver does the writing; ALSA shows it as the mixer of the "I-Sabre Q2M DAC" card.

| Register | What it holds |
| --- | --- |
| 0x01, 0x02 | device ID and API revision, read once when the driver starts |
| 0x10 | whether the sample rate is above 192 kHz |
| 0x20 | the volume, as attenuation: 100 minus the mixer's *Digital Playback Volume* |
| 0x21 | the mute: 1 muted, 0 not. The mixer's *Digital Playback Switch*, and also what the kernel writes when a stream starts and stops |
| 0x22 | the filter, 0 to 6 in the order of the mixer's *FIR Filter Type* |
| 0x24 | the input: 0 I2S, 1 S/PDIF |

The registers can be read past the driver, as the `volumio` user:

    /usr/sbin/i2cget -f -y 1 0x48 0x20

The kernel keeps its own copy of what it last wrote and shows that, without reading the microcontroller. So ALSA can show a volume or a filter the DAC does not have: after a start the filter it shows is its own default, not the one the microcontroller holds, and a write that was lost on the way is shown as made.

### On the Raspberry Pi 5 family: writes and the audio clocks

Seen on a Compute Module 5 with kernel 6.12, at the default bus speed of 100 kHz:

- A write on its own is taken, every time, and so is a run of small ones.
- The output closed and opened again, however often, does no harm on its own.
- **A write while the audio clocks are stopping or starting** can be lost without any error, or fail with `i2c_designware ... SDA stuck at low` and `i-sabre-codec ... -121` in the kernel log. A dozen writes across a few reopenings of the output leave the microcontroller deaf: it no longer answers at 0x48.
- A long run of large changes of volume, ten and more one after another, does the same without a reopening.
- A slower bus makes all of it worse: at 10 kHz even a run of small writes is enough.
- A restart of the Raspberry Pi does not bring a deaf microcontroller back; the next start logs `Audiophonics Device ID : FFFFFF87`. Taking the unit's power away does.

The Pi 5's I2C controller gives a transfer up when the data line has been held low for 10 milliseconds, and the transfer is not tried again. The microcontroller holds the line for longer than that at these moments.

Volumio itself brings writes and clock changes together when it starts: it restores the mixer as it rebuilds its audio configuration, and sends its startup volume a few milliseconds before its startup sound opens the output.

What the plugin does about its own part:

- When a setting changes the player's audio path, the program behind Click prevention is stopped first; Volumio rebuilds the path and restores the mixer with nothing of the plugin's holding the output; the volume is put back a moment later, and the program is started a moment after that.
- The program behind Click prevention idles at 44.1 kHz, the rate of Volumio's startup sound, so the output is not opened again for it; and it waits two seconds after the audio configuration changes before it reopens the output.
- With Click prevention on, the output stays open between tracks, so the clocks change only when the sample rate does.

## The remote

The handset and its keys are described in [remote.md](remote.md). The streamer acts on five of them:

| Key | Action |
| --- | --- |
| Play | play or pause |
| Right | next track |
| Left | previous track |
| Up | seek forward; steps on while held |
| Down | seek back; steps on while held |
