# csgo-matchbot

Discord bot for managing & organizing team matches

## Features

- Team management that integrates with Discord roles
- Bo1, Bo3, Bo5 series map veto setup
- Automated server setup via Dathost integration
- Integration with [matchbot-api](https://github.com/martig3/matchbot-api) for other automated features

## Usage

Create `.env` file with the following fields:

```env
DATABASE_URL=<db url i.e. postgres://postgres:postgres@localhost/matchbot>
DISCORD_TOKEN=<discord token>
STEAM_API_KEY=<steam web api key>
MATCH_END_WEBHOOK_URL=<url, optional>
ROUND_END_WEBHOOK_URL=<url, optional>
SERIES_END_WEBHOOK_URL=<url, optional>
DATHOST_USER=<dathost account username/email>
DATHOST_PASSWORD=<dathost account password>
BUCKET_URL=<optional, s3 bucket base url for match demos (see matchbot-api integration)>
```

### Docker

`docker run --env-file .env -d ghcr.io/martig3/csgo-matchbot:latest`

### Add bot to discord server

Bot client id can be found under OAuth2 > General

`https://discord.com/api/oauth2/authorize?client_id=<bot client id>&permissions=326685952000&scope=bot`
