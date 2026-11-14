# Audiophonics for Volumio

The Volumio 4 plugins for Audiophonics hardware, and everything they are built from: the programs that draw the displays and read the remote, the device tree overlays, what is known of the hardware, and the tooling that turns it all into one zip per plugin.

Each plugin ships a small native program written in Rust and a thin plugin around it. Nothing is compiled on the player, no Node add-on has to be rebuilt when Volumio moves to a new Node, and no remote-control daemon is installed: the kernel decodes the handset and the program reads what it decoded.

## Plugins

| Plugin | Store name | Hardware | Status |
| --- | --- | --- | --- |
| [Evo Sabre](plugins/evo_sabre/README.md) | `audiophonics_evo_sabre` | EVO Sabre: second OLED display, remote | 2.99 builds on the bench; 3.0.0 will be its first release to the store |

## Supported boards

Raspberry Pi 4 and Raspberry Pi 5, and the Compute Module 4 and 5 on a carrier board that brings out the 40-pin header. The programs find the GPIO chip by its label and the remote's receiver by its driver, so the same binary runs on both families. See [hardware/](hardware/README.md).

## What is where

    plugins/     one folder per plugin: its Volumio side, its settings page, its installer
    bins/        the programs the plugins ship, one per product
    crates/      what the programs share: the player, the remote, the journal and the settings
    components/  what a plugin ships from another project's release, by version
    keys/        the key those releases are signed with
    overlays/    device tree overlays the products need
    hardware/    what is known of each product: buses, pins, the handset
    artwork/     the Audiophonics mark the greetings are drawn from
    scripts/     check, build, package

## Building

The toolchain is pinned in `rust-toolchain.toml`; `rustup` installs it on first use. The shipped programs are static musl binaries for 32-bit ARM, linked with the GNU cross driver:

    sudo apt-get install gcc-arm-linux-gnueabihf qemu-user shellcheck zip unzip curl openssl

Then, from the repository's root:

    scripts/check.sh              formatting, lints, tests, documentation, every screen drawn, the plugins' files and installers
    scripts/ship.sh               the programs, built for the player, into target/ship/
    scripts/package.sh            each plugin's zip, into dist/

A plugin may ship a program this repository does not build. Each is named in `components/<name>.lock` with its version and comes from its own GitHub release; `scripts/package.sh` fetches it (`scripts/fetch-components.sh`) and puts nothing of it in a zip before the release's checksums have been verified against their Ed25519 signature with `keys/release-signing.pub`, the archive against the checksums, and the archive's own manifest against its signature and its files. Packaging therefore needs the network once per version.

`scripts/ship.sh` and `scripts/package.sh` take plugin names to do less: `scripts/ship.sh evo_sabre`. With `qemu-user` installed, `ship.sh` runs each ARM binary and checks that it draws the same pixels as the host's build.

The plugins' own checks run under Node 20; where Node is not installed they run in the `node:20-slim` container. Each plugin's `install.sh` and `uninstall.sh` are run for real in a Debian bookworm container laid out like a player (`scripts/installers.sh`): a fresh install, an install over the previous version, a second install and an uninstall. Both need Docker.

### Looking at the screens

Every screen a display draws can be written out as an image by the code that draws the glass:

    cargo run -p evosabre-panel -- --render target/renders/evo_sabre --assets plugins/evo_sabre/assets

The files are binary PGM, 256 x 64. `magick playing.pgm -filter point -resize 400% playing.png` makes one easy to look at.

## Installing a build on a player

Copy the zip to the player and install it as any plugin zip:

    scp dist/audiophonics_evo_sabre-2.99.0.zip volumio@volumio.local:
    ssh volumio@volumio.local
    mkdir evo && cd evo && miniunzip ../audiophonics_evo_sabre-2.99.0.zip && volumio plugin install

`volumio plugin install` installs the plugin's Node dependencies on the player, so the player needs the network for it; the zip carries no `node_modules`, as the plugin store requires.

A plugin that changes the boot configuration says so when it is installed; restart the player once after that.

## Releases

A release is one plugin at one version. Its tag is the plugin's folder and the version, `evo_sabre-v2.99.0`, and the version is the one its program carries in `bins/<program>/Cargo.toml`; the plugin's `package.json` and `CHANGELOG.md` must agree, and the check says so when they do not.

    git tag evo_sabre-v2.99.0 && git push origin evo_sabre-v2.99.0

A new major version is numbered below itself while it is on the bench: the builds of what will be 3.0.0 are 2.99.0, 2.99.1 and so on, and the round number is kept for the first build that has run on the unit and been accepted by the plugin store.

A release to the store carries its settings page in English and in German, Spanish, French, Italian, Japanese, Dutch, Polish, Portuguese, Russian and Chinese; `scripts/check.sh` refuses one that does not. A build for the bench may carry fewer.

The tag runs the check, builds the program, packages the plugin and publishes the zip with its SHA-256 as a pre-release, with that version's section of the plugin's changelog as the notes. A pre-release becomes the latest release once it has run on the unit it is for:

    gh release edit evo_sabre-v2.99.0 --prerelease=false --latest

The release workflow can also be run by hand for a plugin; it then builds everything and publishes nothing.

## Credits

Audiophonics donated the hardware these plugins are developed and tested on.

Olivier Schwach wrote the original EVO Sabre and RaspDac Mini plugins for Audiophonics, and the work started with him. The plugins here are rewrites from the ground up by Just a Nerd.

## Support this project

These plugins are free and open source, designed, built and maintained by one developer in his own time. If they have earned a place on your player, you can support their development:

- GitHub Sponsors: https://github.com/sponsors/foonerd
- Ko-fi: https://ko-fi.com/justanerd

Sponsorship is a thank-you, not a purchase: it buys no priority, no features and no support agreement.

## Licence

MIT; see [LICENSE](LICENSE). What a plugin ships from elsewhere keeps its own licence, with a note beside it in the plugin: GNU Unifont, as font data, and Audio Keepalive, under the GNU General Public License.
