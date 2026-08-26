# wpm-rt

Live system-wide WPM monitor for Hyprland. Tiny quick-shell overlay monitor to track your wpm real time. 

## Quick start

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

## Development

```bash
nix develop path:.
./scripts/dev
```

You can test the daemon without the overlay:

```bash
cargo run -- stream
```

List readable keyboard-like devices:

```bash
cargo run -- devices
```

## WPM calculation

`wpm_rt` estimates live WPM from key-down events:

```text
wpm = chars_per_second * 60 / 4.8
```

The default rolling window is `2000ms`, with a `1000ms` minimum denominator at the start of a burst so the first few keys do not inflate the reading. The overlay hides after `900ms` without typing.

Letter keys pressed while Ctrl, Alt, or Super are held are ignored so common shortcuts do not count as typing.

Daemon options:

```bash
wpm-rt stream --window-ms 2000 --idle-ms 900 --word-len 4.8
```
