# The remote control

The EVO Sabre and the RaspDac Mini LCD ship the same infrared handset. Its frames are NEC with a 16-bit address: four bytes, address low, address high, command, and the command inverted, each sent least significant bit first, on a 38 kHz carrier.

## Receiving it

The receiver is wired to a GPIO and described to the kernel by the `gpio-ir` overlay. The kernel's own NEC decoder turns each frame into a scancode and reports it on the receiver's input device, once when a key goes down and again for each repeat the handset sends while it is held, about nine a second.

The decoder has to be switched on for the receiver: the overlay's default key table switches on RC-6 alone. A udev rule the plugins install asks for it whenever the receiver appears:

    ACTION=="add|change", SUBSYSTEM=="rc", ENV{DRV_NAME}=="gpio_ir_recv", ATTR{protocols}="+nec"

To see what a receiver decodes, and to watch scancodes arrive:

    cat /sys/class/rc/rc*/protocols        the decoders; those switched on are in [brackets]
    journalctl -u evosabre-panel -f        with the plugin's journal set to Changes

## Keys

The frame is each key's four bytes as they are transmitted, first bit first: the form a recording of the handset gives. The scancode is what the kernel reports for it.

| Key | Frame | Scancode |
| --- | --- | --- |
| Power | `0x019808F7` | `0x801910` |
| Input | `0x01989867` | `0x801919` |
| Option | `0x019818E7` | `0x801918` |
| Up | `0x0198E817` | `0x801917` |
| Down | `0x01986897` | `0x801916` |
| Left | `0x0198F20D` | `0x80194F` |
| Right | `0x01988A75` | `0x801951` |
| OK | `0x01980AF5` | `0x801950` |
| Play left | `0x0198F807` | `0x80191F` |
| Play | `0x0198B847` | `0x80191D` |
| Play right | `0x019858A7` | `0x80191A` |
| Volume down | `0x019838C7` | `0x80191C` |
| Volume up | `0x0198629D` | `0x801946` |
| Mute | `0x01987887` | `0x80191E` |

The scancode follows from the frame: each byte is turned round, and a frame whose second byte is not the first inverted is extended NEC, reported as address, address and command in three bytes.
