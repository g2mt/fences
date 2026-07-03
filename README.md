# rsfences

Organise desktop shortcuts and files into resizable, movable containers ("fences") on the Windows desktop.

Fences sit on the wallpaper behind the desktop icon layer, and can be moved, resized, renamed, and populated with file/folder shortcuts.

## How it works

- Fences sit on the desktop wallpaper, behind your existing desktop icons.
- Drag a fence by its title bar to move it; drag its edges or corners to resize it.
- Drop files or folders onto a fence to add shortcuts. Right-click inside to add, run, rename, or remove icons.
- Dropping a folder opens an import dialog to pick which items to include.
- Fence positions, sizes, and contents auto-save to `Documents/FencesConf/state.json` and restore on restart.
- Edit appearance (fonts, colours, borders, icon sizes) directly in `Documents/FencesConf/config.json`.

## Usage

- **Add fence** (tray menu): Creates a new empty fence at the centre of the screen.
- **Add fence from folder** (tray menu): Opens a folder picker and populates a fence with its contents.
- **Reload** (tray menu): Spawns a new instance and exits the current one.
- **Exit** (tray menu): Closes the application and saves state.
- **Right-click a fence**: Rename, delete, add icons, open in Explorer, or set sticky position (snap to screen corner).
- **Drag title bar**: Move the fence.
- **Drag edges / corners**: Resize the fence.
- **Drag files / folders onto a fence**: Add shortcuts or import a folder.

## Build

```
cargo build --release
```

## License

MIT
