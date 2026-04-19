# Ouroboros

Discord mod bot that is supposed to do one thing and one thing well. Currently in private beta but open source if you want to self host it yourself.

Open source as I believe that people, especially moderators, should know exactly what their tools do.

Feel free to contribute!

Developed and tested on Rust 1.89.0-nightly.
Requires a PostgreSQL database.
Tested with Windows 11, Ubuntu 22/Ubuntu 24. Other linux based operating systems should work fine. Other operating systems wont work with the update command.

**features:**
- Modern semantics: Infers arguments using replies (reply to someone, automod logs with +ban and the bot fills in the rest!)
- Logs pull additional data from audit log, allowing for display such as who deleted a message
- Dynamic message cache which allows for giant cache sizes where it matters while keeping down memory consumption. (In tests a moderately active channel with ~300 messages per channel has a size of ~200 messages!)
- Very fast response times due to aggressive caching (additionally depends on latency to Discord servers)
- No bloat. Expect this bot to not turn into another kitchen sink bot with 5000 commands. We are dedicated to moderation.

**Use:**
(Discord installation link coming soon, once the bot enters its first public release)

Join the Discord server: https://discord.gg/SdUf7TrbDq

**install (self hosting):**
- grab the latest binary from the latest build artifact or build it yourself
- create a Config.toml in the same directory as the binary (see example below)
- run the binary (or set up )
- win

**Update:**

The update fetches the newest binary from the artifact actions of the specified repository and shuts the process down. If you have systemd or similar set up to auto restart everything is automatic. If you need more specific behaviour feel free to fork the bot!

Config format:
Minimal:
```toml
[bot]
env = "release"

[release]
token = "" # bot token
prefix = "+" # bot prefix
database_url = "postgres://user:password@ip/database"
```
Full:
```toml
[bot]
env = "release" # either release or dev to set which environment it pulls settings from

[release]
token = "" # bot token
prefix = "+" # bot prefix
database_url = "postgres://user:password@ip/database" # database url, must be postgres
max_connections = 5 # database max connections
repository = "chloecinders/ouroboros" # the repository to update from
github_token = "" # the github token with actions access to the repository in case its private (must add the artifacts permission to the token)
dev_ids = [1234567890] # list of user ids which have access to developer commands
whitelist_enabled = false # enables the whitelist
whitelist = [987654321, 1234567890] # list of whitelisted server ids

# same thing as above...
[dev]
token = ""
prefix = "!"
database_url = ""
```
