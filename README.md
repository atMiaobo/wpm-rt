# wpm-rt

Live system-wide WPM monitor for Hyprland. Tiny quick-shell overlay monitor to track your wpm real time. Started as a 1h programming challenge commissioned by a skeptical Japanese friend -since my typing speed is easily in the triple digits, finished in 14.2 minutes.


watch demo
<https://github.com/user-attachments/assets/8a270a0c-ca91-4307-9fe3-82c4baecb7ce>


Inspect your stats: wpm-rt stats:
<img width="2532" height="1507" alt="image" src="https://github.com/user-attachments/assets/46aca74c-2e2b-41a3-99e2-91b75dbf57a2" />


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


### non-nix Linux distributions

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

## CLI

Launch the Quickshell overlay:

```bash
wpm-rt-shell
```

Run the daemon directly and print live samples:

```bash
wpm-rt stream
```

Open the localhost stats dashboard:

```bash
wpm-rt stats
```

List readable keyboard-like input devices:

```bash
wpm-rt devices
```

From a local Nix checkout, use:

```bash
nix run path:.
nix run path:.#daemon -- stream
nix run path:.#daemon -- stats
nix run path:.#daemon -- devices
```

Daemon options:

```bash
wpm-rt stream --window-ms 2000 --idle-ms 900 --word-len 4.8
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
