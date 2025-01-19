# Bot to ping them all

## Usage

1. compile `cargo build --release`
2. configure (see CONFIGURATION)
3. run `./target/release/tgpingbot config.yaml`
4. add telegram bot to your group, with access to messages
5. start pinging! (with `/ping` command in your group). The bot pings everyones it knows, for it to know some user said user should say something in group. You can also introduce someone to bot with admin command `/add_user <chat_id> <user_id>`, where both `chat_id` and `user_id` are `i64`.

## CONFIGURATION

Configuration in YAML format is expected. Example `config.yaml`:

```yaml
storage: "storage.db"

bot:
  token: "123452345:324583y45wejkrh32498p57"
  admin_id: 123871269386
```

`admin_id` is `i64` id of admin user, there are some admin commands that can be used by that user. It is optional.

## TODO

- DB normal form
