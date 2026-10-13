# infiltrator-bevy-desktop-host

> 层级：L3

Owns the Bevy desktop executable and its `DesktopProductSession` composition. It injects the desktop Core surface and command adapter into `infiltrator-bevy-ui`; the UI crate does not open a desktop session or own a process entrypoint.

The `--capture` entry is available only with the explicit `capture-tools` feature. Product builds keep fixture code out of the default feature set. The executable keeps the established `infiltrator-bevy-ui` product name while its package owns the desktop composition.
