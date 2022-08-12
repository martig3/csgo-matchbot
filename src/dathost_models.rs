use serde::*;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DathostServerDuplicateResponse {
    #[serde(rename = "added_voice_server")]
    pub added_voice_server: String,
    #[serde(rename = "ark_settings")]
    pub ark_settings: ArkSettings,
    pub autostop: bool,
    #[serde(rename = "autostop_minutes")]
    pub autostop_minutes: i64,
    pub booting: bool,
    pub confirmed: bool,
    #[serde(rename = "cost_per_hour")]
    pub cost_per_hour: i64,
    #[serde(rename = "csgo_settings")]
    pub csgo_settings: CsgoSettings,
    #[serde(rename = "custom_domain")]
    pub custom_domain: String,
    #[serde(rename = "cycle_months_12_discount_percentage")]
    pub cycle_months_12_discount_percentage: i64,
    #[serde(rename = "cycle_months_1_discount_percentage")]
    pub cycle_months_1_discount_percentage: i64,
    #[serde(rename = "cycle_months_3_discount_percentage")]
    pub cycle_months_3_discount_percentage: i64,
    #[serde(rename = "default_file_locations")]
    pub default_file_locations: Vec<String>,
    #[serde(rename = "disk_usage_bytes")]
    pub disk_usage_bytes: i64,
    #[serde(rename = "duplicate_source_server")]
    pub duplicate_source_server: String,
    #[serde(rename = "enable_core_dump")]
    pub enable_core_dump: bool,
    #[serde(rename = "enable_mysql")]
    pub enable_mysql: bool,
    #[serde(rename = "enable_syntropy")]
    pub enable_syntropy: bool,
    #[serde(rename = "first_month_discount_percentage")]
    pub first_month_discount_percentage: i64,
    #[serde(rename = "ftp_password")]
    pub ftp_password: String,
    pub game: String,
    pub id: String,
    pub ip: String,
    pub location: String,
    #[serde(rename = "manual_sort_order")]
    pub manual_sort_order: i64,
    #[serde(rename = "match_id")]
    pub match_id: String,
    #[serde(rename = "max_cost_per_hour")]
    pub max_cost_per_hour: i64,
    #[serde(rename = "max_cost_per_month")]
    pub max_cost_per_month: i64,
    #[serde(rename = "max_disk_usage_gb")]
    pub max_disk_usage_gb: i64,
    #[serde(rename = "month_credits")]
    pub month_credits: i64,
    #[serde(rename = "month_reset_at")]
    pub month_reset_at: i64,
    #[serde(rename = "mumble_settings")]
    pub mumble_settings: MumbleSettings,
    #[serde(rename = "mysql_password")]
    pub mysql_password: String,
    #[serde(rename = "mysql_username")]
    pub mysql_username: String,
    pub name: String,
    pub on: bool,
    #[serde(rename = "players_online")]
    pub players_online: i64,
    pub ports: Ports,
    #[serde(rename = "prefer_dedicated")]
    pub prefer_dedicated: bool,
    #[serde(rename = "private_ip")]
    pub private_ip: String,
    #[serde(rename = "raw_ip")]
    pub raw_ip: String,
    #[serde(rename = "reboot_on_crash")]
    pub reboot_on_crash: bool,
    #[serde(rename = "scheduled_commands")]
    pub scheduled_commands: Vec<ScheduledCommand>,
    #[serde(rename = "server_error")]
    pub server_error: String,
    #[serde(rename = "server_image")]
    pub server_image: String,
    pub status: Vec<Status>,
    #[serde(rename = "subscription_cycle_months")]
    pub subscription_cycle_months: i64,
    #[serde(rename = "subscription_renewal_failed_attempts")]
    pub subscription_renewal_failed_attempts: i64,
    #[serde(rename = "subscription_renewal_next_attempt_at")]
    pub subscription_renewal_next_attempt_at: i64,
    #[serde(rename = "subscription_state")]
    pub subscription_state: String,
    #[serde(rename = "teamfortress2_settings")]
    pub teamfortress2_settings: Teamfortress2Settings,
    #[serde(rename = "teamspeak3_settings")]
    pub teamspeak3_settings: Teamspeak3Settings,
    #[serde(rename = "user_data")]
    pub user_data: String,
    #[serde(rename = "valheim_settings")]
    pub valheim_settings: ValheimSettings,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArkSettings {
    #[serde(rename = "cluster_main_server")]
    pub cluster_main_server: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CsgoSettings {
    #[serde(rename = "autoload_configs")]
    pub autoload_configs: Vec<String>,
    #[serde(rename = "disable_1v1_warmup_arenas")]
    pub disable_1v1_warmup_arenas: bool,
    #[serde(rename = "disable_bots")]
    pub disable_bots: bool,
    #[serde(rename = "enable_csay_plugin")]
    pub enable_csay_plugin: bool,
    #[serde(rename = "enable_gotv")]
    pub enable_gotv: bool,
    #[serde(rename = "enable_gotv_secondary")]
    pub enable_gotv_secondary: bool,
    #[serde(rename = "enable_sourcemod")]
    pub enable_sourcemod: bool,
    #[serde(rename = "game_mode")]
    pub game_mode: String,
    pub insecure: bool,
    pub mapgroup: String,
    #[serde(rename = "mapgroup_start_map")]
    pub mapgroup_start_map: String,
    #[serde(rename = "maps_source")]
    pub maps_source: String,
    pub password: String,
    #[serde(rename = "private_server")]
    pub private_server: bool,
    #[serde(rename = "pure_server")]
    pub pure_server: bool,
    pub rcon: String,
    pub slots: i64,
    #[serde(rename = "sourcemod_admins")]
    pub sourcemod_admins: String,
    #[serde(rename = "sourcemod_plugins")]
    pub sourcemod_plugins: Vec<String>,
    #[serde(rename = "steam_game_server_login_token")]
    pub steam_game_server_login_token: String,
    pub tickrate: i64,
    #[serde(rename = "workshop_authkey")]
    pub workshop_authkey: String,
    #[serde(rename = "workshop_id")]
    pub workshop_id: String,
    #[serde(rename = "workshop_start_map_id")]
    pub workshop_start_map_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MumbleSettings {
    pub password: String,
    pub slots: i64,
    #[serde(rename = "superuser_password")]
    pub superuser_password: String,
    #[serde(rename = "welcome_text")]
    pub welcome_text: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ports {
    #[serde(rename = "*")]
    pub field: GeneratedType,
    pub game: i64,
    pub gotv: i64,
    #[serde(rename = "gotv_secondary")]
    pub gotv_secondary: i64,
    pub query: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedType {
    pub additional_prop1: i64,
    pub additional_prop2: i64,
    pub additional_prop3: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledCommand {
    pub action: String,
    pub command: String,
    pub name: String,
    pub repeat: i64,
    #[serde(rename = "run_at")]
    pub run_at: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub key: String,
    pub value: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Teamfortress2Settings {
    #[serde(rename = "enable_gotv")]
    pub enable_gotv: bool,
    #[serde(rename = "enable_sourcemod")]
    pub enable_sourcemod: bool,
    pub insecure: bool,
    pub password: String,
    pub rcon: String,
    pub slots: i64,
    #[serde(rename = "sourcemod_admins")]
    pub sourcemod_admins: String,
    #[serde(rename = "start_map")]
    pub start_map: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Teamspeak3Settings {
    pub slots: i64,
    #[serde(rename = "ts_admin_token")]
    pub ts_admin_token: String,
    #[serde(rename = "ts_server_id")]
    pub ts_server_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValheimSettings {
    #[serde(rename = "admins_steamid64")]
    pub admins_steamid64: Vec<String>,
    #[serde(rename = "bepinex_plugins")]
    pub bepinex_plugins: Vec<String>,
    #[serde(rename = "enable_bepinex")]
    pub enable_bepinex: bool,
    #[serde(rename = "enable_valheimplus")]
    pub enable_valheimplus: bool,
    pub password: String,
    pub slots: i64,
    #[serde(rename = "world_name")]
    pub world_name: String,
}
