# Device tree overlays

The overlays the products need.

| Product | Overlay | Where it comes from |
| --- | --- | --- |
| EVO Sabre | `gpio-ir` | the Raspberry Pi kernel |
| EVO Sabre | `i-sabre-q2m` | the Raspberry Pi kernel |

Both EVO Sabre overlays ship with the kernel, so nothing is built here for it. An overlay a product needs that the kernel does not ship has its source in this folder and is compiled into its plugin when the plugin is packaged.
