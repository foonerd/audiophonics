# RaspDac Mini

A streamer built around a Raspberry Pi, an ES9038Q2M DAC board, a 2.4 inch colour display and an infrared receiver. It is a kit: its case brings out the Raspberry Pi's rear ports and the DAC's RCA outputs at the back, and has the display, a start-up LED and the infrared receiver at the front. The Raspberry Pi's HDMI ports are inside the case.

## What the Raspberry Pi drives

| Part | Connection | Notes |
| --- | --- | --- |
| Display | SPI0, CE0 (`/dev/spidev0.0`), mode 0, 64 MHz | ILI9341, 320 x 240 as mounted, sixteen bits a pixel, 4-wire SPI |
| Display D/C | GPIO 27 | |
| Display reset | GPIO 24 | active low |
| Infrared receiver | GPIO 4 | `dtoverlay=gpio-ir,gpio_pin=4` |
| DAC | I2S, and its mixer | `dtoverlay=i-sabre-q2m`; ALSA card "I-Sabre Q2M DAC" |

The display's backlight is not on a line the Raspberry Pi drives: it is lit whenever the unit is.

The display, its two lines and the receiver are wired as on the [EVO Sabre](evo-sabre.md), and the handset is [the same](remote.md). Unlike the EVO Sabre's, this DAC board does not read the handset: every key is the streamer's to act on.

## Boot configuration

With the kernel driving the display, as a framebuffer:

    dtoverlay=fbtft,spi0-0,ili9341,bgr,reset_pin=24,dc_pin=27,rotate=90,speed=64000000,fps=100
    dtoverlay=gpio-ir,gpio_pin=4

With the plugin's program driving it over SPI:

    dtparam=spi=on
    dtoverlay=gpio-ir,gpio_pin=4

The plugin writes one pair or the other into its own block of `/boot/userconfig.txt`, as its *Display driver* setting says. As a framebuffer, the kernel sends the display the rows that changed within a hundredth of a second of their changing (`fps=100`). The DAC's overlay is written by Volumio when the DAC is chosen in Playback Options, as "Audiophonics I-Sabre ES9028Q2M".

## The display

The ILI9341 holds the picture in its own memory, 240 columns by 320 rows. The glass is mounted on its side, so the memory is read out with rows and columns exchanged and both mirrored, and the colour filter's order is blue, green, red: address mode (0x36) 0xE8. Pixels are sixteen bits, five of red, six of green and five of blue (pixel format, 0x3A, 0x55), sent high byte first.

A part of the picture is changed by naming a window (column address 0x2A, page address 0x2B, each a first and a last, sixteen bits, high byte first) and writing its pixels (memory write, 0x2C). The write goes on across the gaps between SPI transfers.

The start-up the panel is given, after a reset, is in `bins/raspdacmini-panel/src/ili9341.rs`: the power and timing registers and the two gamma curves are the panel maker's values. The memory is written black before the display is switched on (0x29).

### As a framebuffer

The kernel's `fbtft` overlay with the parameters above loads the `fb_ili9341` driver, which gives the panel as a framebuffer named `fb_ili9341`: 320 x 240, sixteen bits a pixel, each pixel's low byte first. `rotate=90` and `bgr` make the driver write the same address mode, 0xE8. Its number among the framebuffers depends on what else the system has; it is found by its name:

    grep -l fb_ili9341 /sys/class/graphics/fb*/name

The overlay is applied by the firmware, so the framebuffer is there from the first moments of the boot, and whatever draws on framebuffers then draws on the panel: Volumio's boot screens, and the progress of a system update. While the overlay is loaded the kernel holds the SPI device, and `/dev/spidev0.0` does not exist.

After any write to the framebuffer the driver sends the panel the whole picture, at most thirty times a second.

Version 1 of the plugin drove the panel the same way through an overlay of its own, `raspdac-mini-lcd`, which the plugin no longer installs.

## The DAC's two choices

Both are controls of the card's mixer, each a list of names:

| Control | Names |
| --- | --- |
| `I2S/SPDIF Select` | `I2S`, the player; `SPDIF` |
| `FIR Filter Type` | the filters the DAC has, such as `brick wall` |

    amixer -c <card> sget 'FIR Filter Type'
    amixer -c <card> sset 'I2S/SPDIF Select' SPDIF

The card's number is in `/proc/asound/cards`, on the line that names "I-Sabre Q2M DAC".
