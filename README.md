# Bot to ping them all

## Usage

1. compile `cargo build --release`
2. configure (see CONFIGURATION)
3. run `./target/release/tgpingbot config.yaml`
4. add telegram bot to your group, with access to messages
5. start pinging! (with `/ping` command in your group). The bot pings everyones it knows, for it to know some user said user should say something in group.

## CONFIGURATION

Configuration in YAML format is expected. Example `config.yaml`:

```yaml
token: "123452345:324583y45wejkrh32498p57"
storage: "storage.db"
```

## TODO
