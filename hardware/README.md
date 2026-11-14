# Hardware

What the plugins know about the hardware they drive, one page per product, and what the products share.

- [EVO Sabre](evo-sabre.md)
- [RaspDac Mini](raspdac-mini.md)
- [The remote control](remote.md)

## Boards

| Board | SoC | Header GPIO, SPI and I2S |
| --- | --- | --- |
| Raspberry Pi 4, Pi 400, Compute Module 4 | BCM2711 | in the SoC; GPIO chip labelled `pinctrl-bcm2711` |
| Raspberry Pi 5, Compute Module 5 | BCM2712 | on the RP1; GPIO chip labelled `pinctrl-rp1` |

A Compute Module needs a carrier board that brings out the 40-pin header.

The header's pin functions are the same on both families, and so are the device nodes the programs open: `/dev/spidev0.0` for SPI0 CE0, and the receiver's input device wherever the kernel put it. What differs is the GPIO chip's number, which has also moved between kernels, so the programs never use it: they take the chip whose label starts with `pinctrl-`.

Volumio's Raspberry Pi images run a 32-bit ARM userland on every board, under a 32-bit or a 64-bit kernel. The programs are static 32-bit ARMv7 binaries and run on all of them.

Each program says which board it found when its journal is set to Changes:

    board: Raspberry Pi 5 Model B Rev 1.0 (BCM2712)
