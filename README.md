# wpm_rt

Live system-wide WPM monitor for Hyprland. A tiny Quickshell overlay appears while you type and fades out after a short idle timeout.

The monitor reads Linux input events and emits only aggregate typing speed. It does not store or print key names.

## Quick start

```bash
nix run path:.
```

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

The default rolling window is `2000ms`; the overlay hides after `900ms` without typing.

Daemon options:

```bash
wpm-rt stream --window-ms 2000 --idle-ms 900 --word-len 4.8
```
