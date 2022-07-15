# valorant-matchbot

Simple Discord bot for managing & organizing team matches

## Features

- Add matches to schedule
- Schedule matches
- Bo1, Bo3, Bo5 series map veto setup
- Match setup history

### Example Screenshots

// TODO

### Commands

`/setup` - start user's team's next match setup

`/schedule` - schedule match

`/matches` - list matches

`/maps` - list maps

`/cancel` - cancel setup

`/defense` - pick defense side during side pick phase

`/attack`- pick attack side during side pick phase

`/pick` - pick map during map veto phase

`/ban` - ban map during map veto phase

`/help` - DMs you help text

_These are privileged admin commands:_

`/addmatch` - add match to schedule

`/deletematch`- delete match from schedule

`/cancel` - cancel setup

### Setup

Download the latest release and place it inside a new folder. Inside this folder create a config.yaml file.

Note: Channel & role ids can be found by enabling discord developer mode.

Start the bot via appropriate release binary (or clone & build yourself if you want) and navigate to the following url - make sure to insert your bot's client id in this url - to add the bot to your server: `https://discord.com/api/oauth2/authorize?client_id=<your_bot_clientid>&permissions=16780352&scope=bot`

Note: Make sure to only allow the bot to read messages from one channel under Server Settings -> Integrations

### Example config.yaml

```yaml
discord:
  token: <your discord bot api token>
  admin_role_id: <a discord server role id> -- optional, but highly recommended!!!
  channel_id: <text channel id to bind to bot>
  application_id: <bot application id>
  guild_id: <your guild id>
```
