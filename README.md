# csgo-matchbot

Discord bot for managing & organizing team matches

## Features

- Team management that integrates with Discord roles
- Bo1, Bo3, Bo5 series map veto setup
- Automated server setup via Dathost integration
- Integration with [matchbot-api](https://github.com/martig3/matchbot-api) for other automated features

### Example Screenshots

// TODO

### Setup

```
DATABASE_URL=<full postgres db url including user & password>
DISCORD_TOKEN=<discord token>
STEAM_API_KEY=<steam web api key>
```

Start the bot via appropriate release binary (or clone & build yourself if you want) and navigate to the following url -
make sure to insert your bots client id in this url - to add the bot to your
server: `https://discord.com/api/oauth2/authorize?client_id=<your_bot_clientid>&permissions=16780352&scope=bot`

