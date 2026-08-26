# wpm-rt

Live system-wide WPM monitor for Hyprland. Tiny quick-shell overlay monitor to track your wpm real time. 


## Instantaneous wpm estimation calculus larp formula:
Every active 100ms compute:
seconds = clamp(now - first_key_in_window, min = 1.0s, max = 2.0s)
    cps = key_count_in_window / seconds
    wpm = cps * 60 / 4.8

Basically every active 100ms map active typing time to 1s if t<1s and 2s if t>2s, otherwise stay the same. Then calculate wpm by dividing by 4.8 -aka average word length in English, tailored specifically for everyday peasant output. change to like 5 or smtht for academic writing.




## Quick start

### Nix / NixOS

```bash
sudo setfacl -m u:$USER:r /dev/input/event*
nix run path:.
```
add in config for permanence.

Once the repo is committed, `nix run .` works too.

The app needs permission to read keyboard event devices. On NixOS, the simple development option is:

```nix
users.users.YOUR_USER.extraGroups = [ "input" ];
```

Log out and back in after changing groups.

### Other retarded Linux distributions

Install Quickshell through your distro first, then install `wpm_rt` with Cargo:

```bash
cargo install --git https://github.com/YOUR_USER/wpm_rt
```

Grant your user input-device access:

```bash
sudo usermod -aG input "$USER"
```

Log out and back in, then run:

```bash
wpm-rt-shell
```

For Hyprland autostart, add this to `~/.config/hypr/hyprland.conf`:

```ini
exec-once = wpm-rt-shell
```

## Development

```bash
nix develop path:.
./scripts/dev
```

You can test the daemon without the overlay:

```bash
cargo run -- stream
```

You can run the Cargo-installed style launcher locally:

```bash
cargo run --bin wpm-rt-shell
```

List readable keyboard-like devices:

```bash
cargo run -- devices
```


Letter keys pressed while Ctrl, Alt, or Super are held are ignored so common shortcuts do not count as typing.

Daemon options:

```bash
wpm-rt stream --window-ms 2000 --idle-ms 900 --word-len 4.8
```
