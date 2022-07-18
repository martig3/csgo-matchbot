# csgo-matchbot

Discord bot for managing & organizing team matches

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

`/ct` - pick CT side during side pick phase

`/t`- pick T side during side pick phase

`/pick` - pick map during map veto phase

`/ban` - ban map during map veto phase

`/help` - DMs you help text

_These are privileged admin commands:_

`/addmatch` - add match to schedule

`/deletematch`- delete match from schedule

`/cancel` - cancel setup

### Setup

```
  DISCORD_TOKEN: <your discord bot api token>
  DISCORD_ADMIN_ROLE_ID: <a discord server role id>
  DISCORD_APPLICATION_ID: <bot application id>
  DISCORD_GUILD_ID: <your guild id>
```

_Note: Channel & role ids can be found by enabling discord developer mode. It is also recommended to limit your bot to
one channel via Server Settings>Integration options_

Start the bot via appropriate release binary (or clone & build yourself if you want) and navigate to the following url -
make sure to insert your bot's client id in this url - to add the bot to your
server: `https://discord.com/api/oauth2/authorize?client_id=<your_bot_clientid>&permissions=16780352&scope=bot`

