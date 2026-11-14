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

## The remote

The handset and its keys are described in [remote.md](remote.md). The streamer acts on five of them:

| Key | Action |
| --- | --- |
| Play | play or pause |
| Right | next track |
| Left | previous track |
| Up | seek forward; steps on while held |
| Down | seek back; steps on while held |
