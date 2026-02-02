/**
 * TF2 Relay Client - SourceMod Plugin
 *
 * Connects TF2 server to the Rust relay server for cross-server communication.
 * Features ghost players to visualize players from other servers.
 * Requires: AsyncSocket Extension (https://github.com/srcdslab/sm-ext-asyncsocket)
 *
 * @author SummerTYT / vicentefelipechile
 * @version 2.0.0
 */

#include <sourcemod>
#include <sdktools>
#include <sdkhooks>
#include <tf2>
#include <tf2_stocks>
#include <AsyncSocket>

#pragma semicolon 1
#pragma newdecls required

// ============================================================================
// Protocol Constants
// ============================================================================

#define PROTOCOL_MAGIC_1          0x54    // 'T'
#define PROTOCOL_MAGIC_2          0x46    // 'F'
#define PROTOCOL_VERSION          1
#define MAX_PAYLOAD_SIZE          4096
#define HEADER_SIZE               9
#define CHECKSUM_SIZE             1

// Event Type IDs - System
#define EVENT_HANDSHAKE           0x00
#define EVENT_HANDSHAKE_ACK       0x01
#define EVENT_HEARTBEAT           0x02
#define EVENT_HEARTBEAT_ACK       0x03
#define EVENT_SERVER_CONNECT      0x04
#define EVENT_SERVER_DISCONNECT   0x05

// Event Type IDs - Chat
#define EVENT_CHAT_MESSAGE        0x10

// Event Type IDs - Players
#define EVENT_PLAYER_DEATH        0x20
#define EVENT_PLAYER_CONNECT      0x21
#define EVENT_PLAYER_DISCONNECT   0x22
#define EVENT_PLAYER_TEAM_CHANGE  0x23
#define EVENT_PLAYER_CLASS_CHANGE 0x24
#define EVENT_PLAYER_SPAWN        0x25

// Event Type IDs - Game
#define EVENT_ROUND_START         0x40
#define EVENT_ROUND_END           0x41
#define EVENT_MAP_CHANGE          0x42

// Event Type IDs - Ghost/Sync
#define EVENT_PLAYER_SYNC         0x70
#define EVENT_GHOST_SPAWN         0x71
#define EVENT_GHOST_DESPAWN       0x72

// ============================================================================
// Ghost System Constants
// ============================================================================

#define MAX_GHOSTS                64      // Max ghost players across all servers
#define GHOST_SYNC_RATE           0.05    // 20 times per second
#define GHOST_TIMEOUT             5.0     // Remove ghost if no update for 5 seconds
#define GHOST_INTERP_TIME         0.1     // Interpolation time in seconds

// TF2 Class models
char g_sClassModels[10][PLATFORM_MAX_PATH] = {
    "",    // Unknown
    "models/player/scout.mdl",
    "models/player/sniper.mdl",
    "models/player/soldier.mdl",
    "models/player/demo.mdl",
    "models/player/medic.mdl",
    "models/player/heavy.mdl",
    "models/player/pyro.mdl",
    "models/player/spy.mdl",
    "models/player/engineer.mdl"
};

// ============================================================================
// Ghost Data Structure
// ============================================================================

enum struct GhostPlayer
{
    bool  active;            // Is this ghost slot in use?
    int   serverId;          // Source server ID
    char  steamId[32];       // Steam ID for identification
    char  playerName[64];    // Display name
    int   team;              // TF2 team
    int   classId;           // TF2 class
    int   health;            // Current health
    int   maxHealth;         // Max health

    // Entity references
    int   entityIndex;      // Main prop entity
    int   glowEntity;       // Glow outline entity
    int   nameTagEntity;    // Floating name (optional)

    // Position data
    float position[3];    // Current position
    float angles[3];      // Current angles
    float velocity[3];    // Movement velocity

    // Interpolation
    float targetPosition[3];    // Target position for interpolation
    float targetAngles[3];      // Target angles for interpolation
    float lastUpdateTime;       // Last time we received an update

    // Animation
    int   animSequence;    // Current animation sequence
    bool  isOnGround;      // For animation purposes
    bool  isDucking;       // Crouching state
}

// ============================================================================
// Plugin Info
// ============================================================================

public Plugin myinfo =
{
    name        = "TF2 Relay Client",
    author      = "SummerTYT / vicentefelipechile",
    description = "Cross-server communication with ghost players",
    version     = "2.0.0",
    url         = "https://github.com/vicentefelipechile/tf2-server-relay"
};

// ============================================================================
// Global Variables
// ============================================================================

// ConVars
ConVar      g_cvRelayHost;
ConVar      g_cvRelayPort;
ConVar      g_cvServerId;
ConVar      g_cvServerName;
ConVar      g_cvReconnectDelay;
ConVar      g_cvEnabled;
ConVar      g_cvGhostEnabled;
ConVar      g_cvGhostGlow;
ConVar      g_cvGhostAlpha;

// Socket
AsyncSocket g_hSocket;
bool        g_bConnected;
bool        g_bHandshakeComplete;

// State
int         g_iServerId;
char        g_sServerName[64];
char        g_sCurrentMap[64];
int         g_iSequence;

// Reconnect timer
Handle      g_hReconnectTimer;
Handle      g_hSyncTimer;
Handle      g_hGhostUpdateTimer;

// CRC-8 lookup table
int         g_iCRC8Table[256];

// Ghost system
GhostPlayer g_Ghosts[MAX_GHOSTS];
int         g_iGhostCount;

// ============================================================================
// Plugin Lifecycle
// ============================================================================
public void OnPluginStart()
{
    // Initialize CRC table
    InitCRC8Table();

    // Create ConVars
    g_cvRelayHost      = CreateConVar("sm_relay_host", "127.0.0.1", "Relay server hostname/IP");
    g_cvRelayPort      = CreateConVar("sm_relay_port", "27050", "Relay server port", _, true, 1.0, true, 65535.0);
    g_cvServerId       = CreateConVar("sm_relay_server_id", "1", "Unique server identifier (1-4)", _, true, 1.0, true, 4.0);
    g_cvServerName     = CreateConVar("sm_relay_server_name", "Server 1", "Human-readable server name");
    g_cvReconnectDelay = CreateConVar("sm_relay_reconnect_delay", "5.0", "Reconnection delay in seconds", _, true, 1.0, true, 60.0);
    g_cvEnabled        = CreateConVar("sm_relay_enabled", "1", "Enable/disable relay functionality", _, true, 0.0, true, 1.0);
    g_cvGhostEnabled   = CreateConVar("sm_relay_ghosts", "1", "Enable ghost players from other servers", _, true, 0.0, true, 1.0);
    g_cvGhostGlow      = CreateConVar("sm_relay_ghost_glow", "1", "Enable glow outline on ghosts", _, true, 0.0, true, 1.0);
    g_cvGhostAlpha     = CreateConVar("sm_relay_ghost_alpha", "180", "Ghost transparency (0-255)", _, true, 0.0, true, 255.0);

    // Auto-execute config
    AutoExecConfig(true, "tf2_relay");

    // Hook events
    HookEvent("player_say", Event_PlayerSay);
    HookEvent("player_death", Event_PlayerDeath);
    HookEvent("player_connect", Event_PlayerConnect);
    HookEvent("player_disconnect", Event_PlayerDisconnect);
    HookEvent("player_team", Event_PlayerTeam);
    HookEvent("player_spawn", Event_PlayerSpawn);
    HookEvent("player_changeclass", Event_PlayerClass);
    HookEvent("teamplay_round_start", Event_RoundStart);
    HookEvent("teamplay_round_win", Event_RoundEnd);

    // Get current map
    GetCurrentMap(g_sCurrentMap, sizeof(g_sCurrentMap));

    // Register commands
    RegAdminCmd("sm_relay_reconnect", Command_Reconnect, ADMFLAG_ROOT, "Force reconnect to relay");
    RegAdminCmd("sm_relay_status", Command_Status, ADMFLAG_GENERIC, "Show relay status");
    RegAdminCmd("sm_relay_ghosts", Command_ListGhosts, ADMFLAG_GENERIC, "List active ghosts");

    // Initialize state
    g_bConnected         = false;
    g_bHandshakeComplete = false;
    g_iSequence          = 0;
    g_iGhostCount        = 0;

    // Initialize ghost array
    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        g_Ghosts[i].active      = false;
        g_Ghosts[i].entityIndex = INVALID_ENT_REFERENCE;
        g_Ghosts[i].glowEntity  = INVALID_ENT_REFERENCE;
    }
}

public void OnConfigsExecuted()
{
    g_iServerId = g_cvServerId.IntValue;
    g_cvServerName.GetString(g_sServerName, sizeof(g_sServerName));

    if (g_cvEnabled.BoolValue)
    {
        ConnectToRelay();
    }
}

public void OnPluginEnd()
{
    // Clean up all ghosts
    RemoveAllGhosts();
    Disconnect();
}

public void OnMapStart()
{
    // Precache all class models
    for (int i = 1; i <= 9; i++)
    {
        if (g_sClassModels[i][0] != '\0')
        {
            PrecacheModel(g_sClassModels[i], true);
        }
    }

    GetCurrentMap(g_sCurrentMap, sizeof(g_sCurrentMap));

    // Remove old ghosts on map change
    RemoveAllGhosts();

    // Send map change if connected
    if (g_bHandshakeComplete)
    {
        SendMapChange();
    }
}

public void OnMapEnd()
{
    RemoveAllGhosts();
}

public void OnGameFrame()
{
    // Update ghost interpolation every frame for smooth movement
    if (!g_cvGhostEnabled.BoolValue)
        return;

    float currentTime = GetGameTime();

    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        if (!g_Ghosts[i].active)
            continue;

        // Check for timeout
        if (currentTime - g_Ghosts[i].lastUpdateTime > GHOST_TIMEOUT)
        {
            RemoveGhost(i);
            continue;
        }

        // Interpolate position
        InterpolateGhost(i, currentTime);
    }
}

// ============================================================================
// Ghost Management
// ============================================================================

int FindGhostBySteamId(const char[] steamId)
{
    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        if (g_Ghosts[i].active && StrEqual(g_Ghosts[i].steamId, steamId))
        {
            return i;
        }
    }
    return -1;
}

int FindFreeGhostSlot()
{
    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        if (!g_Ghosts[i].active)
        {
            return i;
        }
    }
    return -1;
}

int CreateGhost(int serverId, const char[] steamId, const char[] playerName, int team, int classId)
{
    int slot = FindFreeGhostSlot();
    if (slot == -1)
    {
        LogError("[Relay] No free ghost slots available");
        return -1;
    }

    // Initialize ghost data
    g_Ghosts[slot].active   = true;
    g_Ghosts[slot].serverId = serverId;
    strcopy(g_Ghosts[slot].steamId, sizeof(g_Ghosts[].steamId), steamId);
    strcopy(g_Ghosts[slot].playerName, sizeof(g_Ghosts[].playerName), playerName);
    g_Ghosts[slot].team           = team;
    g_Ghosts[slot].classId        = classId;
    g_Ghosts[slot].health         = 100;
    g_Ghosts[slot].maxHealth      = 100;
    g_Ghosts[slot].lastUpdateTime = GetGameTime();
    g_Ghosts[slot].entityIndex    = INVALID_ENT_REFERENCE;
    g_Ghosts[slot].glowEntity     = INVALID_ENT_REFERENCE;

    // Create the visual entity
    CreateGhostEntity(slot);

    g_iGhostCount++;
    LogMessage("[Relay] Created ghost for %s (Server %d, Class %d)", playerName, serverId, classId);

    return slot;
}

void CreateGhostEntity(int slot)
{
    if (!g_Ghosts[slot].active)
        return;

    // Get model for class
    int classId = g_Ghosts[slot].classId;
    if (classId < 1 || classId > 9)
        classId = 1;    // Default to Scout

    // Create prop_dynamic for the ghost
    int entity = CreateEntityByName("prop_dynamic_override");
    if (!IsValidEntity(entity))
    {
        LogError("[Relay] Failed to create ghost entity");
        return;
    }

    // Set model
    SetEntityModel(entity, g_sClassModels[classId]);

    // Set properties
    DispatchKeyValue(entity, "solid", "0");    // Non-solid
    DispatchKeyValue(entity, "DefaultAnim", "stand_LOSER");

    DispatchSpawn(entity);
    ActivateEntity(entity);

    // Set render properties for ghost effect
    int alpha = g_cvGhostAlpha.IntValue;
    SetEntityRenderMode(entity, RENDER_TRANSCOLOR);

    // Team-based color with transparency
    if (g_Ghosts[slot].team == 2)    // RED
    {
        SetEntityRenderColor(entity, 255, 100, 100, alpha);
    }
    else if (g_Ghosts[slot].team == 3)    // BLU
    {
        SetEntityRenderColor(entity, 100, 100, 255, alpha);
    }
    else
    {
        SetEntityRenderColor(entity, 200, 200, 200, alpha);
    }

    // Position
    TeleportEntity(entity, g_Ghosts[slot].position, g_Ghosts[slot].angles, NULL_VECTOR);

    // Store reference
    g_Ghosts[slot].entityIndex = EntIndexToEntRef(entity);

    // Create glow if enabled
    if (g_cvGhostGlow.BoolValue)
    {
        CreateGhostGlow(slot);
    }
}

void CreateGhostGlow(int slot)
{
    int entity = EntRefToEntIndex(g_Ghosts[slot].entityIndex);
    if (!IsValidEntity(entity))
        return;

    int glow = CreateEntityByName("tf_glow");
    if (!IsValidEntity(glow))
        return;

    // Set glow properties
    DispatchKeyValue(glow, "target", "!activator");
    DispatchKeyValue(glow, "Mode", "0");    // Always visible

    // Team color
    char color[32];
    if (g_Ghosts[slot].team == 2)
        Format(color, sizeof(color), "255 50 50 255");
    else if (g_Ghosts[slot].team == 3)
        Format(color, sizeof(color), "50 50 255 255");
    else
        Format(color, sizeof(color), "200 200 200 255");

    DispatchKeyValue(glow, "GlowColor", color);

    DispatchSpawn(glow);
    ActivateEntity(glow);

    // Set parent
    SetVariantString("!activator");
    AcceptEntityInput(glow, "SetParent", entity);

    AcceptEntityInput(glow, "Enable");

    g_Ghosts[slot].glowEntity = EntIndexToEntRef(glow);
}

void UpdateGhostModel(int slot)
{
    int entity = EntRefToEntIndex(g_Ghosts[slot].entityIndex);
    if (!IsValidEntity(entity))
    {
        CreateGhostEntity(slot);
        return;
    }

    int classId = g_Ghosts[slot].classId;
    if (classId < 1 || classId > 9)
        classId = 1;

    SetEntityModel(entity, g_sClassModels[classId]);
}

void UpdateGhostPosition(int slot, float position[3], float angles[3], float velocity[3])
{
    if (!g_Ghosts[slot].active)
        return;

    // Store target for interpolation
    g_Ghosts[slot].targetPosition = position;
    g_Ghosts[slot].targetAngles   = angles;
    g_Ghosts[slot].velocity       = velocity;
    g_Ghosts[slot].lastUpdateTime = GetGameTime();

    // If entity doesn't exist, create it
    int entity                    = EntRefToEntIndex(g_Ghosts[slot].entityIndex);
    if (!IsValidEntity(entity))
    {
        g_Ghosts[slot].position = position;
        g_Ghosts[slot].angles   = angles;
        CreateGhostEntity(slot);
    }
}

void InterpolateGhost(int slot, float currentTime)
{
    if (!g_Ghosts[slot].active)
        return;

    int entity = EntRefToEntIndex(g_Ghosts[slot].entityIndex);
    if (!IsValidEntity(entity))
        return;

    // Simple interpolation factor
    float timeSinceUpdate = currentTime - g_Ghosts[slot].lastUpdateTime;
    float t               = timeSinceUpdate / GHOST_INTERP_TIME;
    if (t > 1.0) t = 1.0;

    // Interpolate position
    float newPos[3];
    for (int i = 0; i < 3; i++)
    {
        newPos[i] = g_Ghosts[slot].position[i] + (g_Ghosts[slot].targetPosition[i] - g_Ghosts[slot].position[i]) * t;
    }

    // Interpolate angles (handle wrap-around)
    float newAngles[3];
    for (int i = 0; i < 3; i++)
    {
        float diff = g_Ghosts[slot].targetAngles[i] - g_Ghosts[slot].angles[i];
        // Normalize angle difference
        while (diff > 180.0)
            diff -= 360.0;
        while (diff < -180.0)
            diff += 360.0;
        newAngles[i] = g_Ghosts[slot].angles[i] + diff * t;
    }

    // Update stored position
    g_Ghosts[slot].position = newPos;
    g_Ghosts[slot].angles   = newAngles;

    // Apply to entity
    TeleportEntity(entity, newPos, newAngles, NULL_VECTOR);
}

void RemoveGhost(int slot)
{
    if (!g_Ghosts[slot].active)
        return;

    // Remove glow entity
    int glow = EntRefToEntIndex(g_Ghosts[slot].glowEntity);
    if (IsValidEntity(glow))
    {
        AcceptEntityInput(glow, "Kill");
    }

    // Remove main entity
    int entity = EntRefToEntIndex(g_Ghosts[slot].entityIndex);
    if (IsValidEntity(entity))
    {
        AcceptEntityInput(entity, "Kill");
    }

    LogMessage("[Relay] Removed ghost: %s", g_Ghosts[slot].playerName);

    // Clear slot
    g_Ghosts[slot].active      = false;
    g_Ghosts[slot].entityIndex = INVALID_ENT_REFERENCE;
    g_Ghosts[slot].glowEntity  = INVALID_ENT_REFERENCE;
    g_iGhostCount--;
}

void RemoveAllGhosts()
{
    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        if (g_Ghosts[i].active)
        {
            RemoveGhost(i);
        }
    }
    g_iGhostCount = 0;
}

void RemoveGhostsByServer(int serverId)
{
    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        if (g_Ghosts[i].active && g_Ghosts[i].serverId == serverId)
        {
            RemoveGhost(i);
        }
    }
}

// ============================================================================
// Position Sync - Sending local players to relay
// ============================================================================

void SendPlayerSync(int client)
{
    if (!g_bHandshakeComplete)
        return;

    if (!IsClientInGame(client) || !IsPlayerAlive(client))
        return;

    char buffer[128];
    int  offset = HEADER_SIZE;

    char steamId[32], playerName[64];
    GetClientAuthId(client, AuthId_SteamID64, steamId, sizeof(steamId));
    GetClientName(client, playerName, sizeof(playerName));

    float position[3], angles[3], velocity[3];
    GetClientAbsOrigin(client, position);
    GetClientEyeAngles(client, angles);
    GetEntPropVector(client, Prop_Data, "m_vecVelocity", velocity);

    // Write player info
    offset += WriteString(buffer, offset, playerName);
    offset += WriteU64(buffer, offset, steamId);
    offset += WriteU8(buffer, offset, GetClientTeam(client));
    offset += WriteU8(buffer, offset, view_as<int>(TF2_GetPlayerClass(client)));
    offset += WriteU16(buffer, offset, GetClientHealth(client));

    // Write position (as fixed-point for precision)
    offset += WriteFloat(buffer, offset, position[0]);
    offset += WriteFloat(buffer, offset, position[1]);
    offset += WriteFloat(buffer, offset, position[2]);

    // Write angles
    offset += WriteFloat(buffer, offset, angles[0]);
    offset += WriteFloat(buffer, offset, angles[1]);
    offset += WriteFloat(buffer, offset, angles[2]);

    // Write velocity
    offset += WriteFloat(buffer, offset, velocity[0]);
    offset += WriteFloat(buffer, offset, velocity[1]);
    offset += WriteFloat(buffer, offset, velocity[2]);

    // Flags
    int flags = 0;
    if (GetEntityFlags(client) & FL_ONGROUND) flags |= 0x01;
    if (GetEntityFlags(client) & FL_DUCKING) flags |= 0x02;
    offset += WriteU8(buffer, offset, flags);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_PLAYER_SYNC, payloadLen);

    SendPacket(buffer, offset);
}

int WriteFloat(char[] buffer, int offset, float value)
{
    // Convert float to 4 bytes
    int intValue       = view_as<int>(value);
    buffer[offset]     = intValue & 0xFF;
    buffer[offset + 1] = (intValue >> 8) & 0xFF;
    buffer[offset + 2] = (intValue >> 16) & 0xFF;
    buffer[offset + 3] = (intValue >> 24) & 0xFF;
    return 4;
}

float ReadFloat(const char[] buffer, int offset)
{
    int intValue = buffer[offset] | (buffer[offset + 1] << 8) | (buffer[offset + 2] << 16) | (buffer[offset + 3] << 24);
    return view_as<float>(intValue);
}

public Action Timer_SyncPositions(Handle timer)
{
    if (!g_bHandshakeComplete)
        return Plugin_Continue;

    // Send position of all alive players
    for (int i = 1; i <= MaxClients; i++)
    {
        if (IsClientInGame(i) && IsPlayerAlive(i) && !IsFakeClient(i))
        {
            SendPlayerSync(i);
        }
    }

    return Plugin_Continue;
}

// ============================================================================
// Connection Management
// ============================================================================

void ConnectToRelay()
{
    if (g_bConnected)
        return;

    char host[128];
    g_cvRelayHost.GetString(host, sizeof(host));
    int port = g_cvRelayPort.IntValue;

    LogMessage("[Relay] Connecting to %s:%d...", host, port);

    g_hSocket = new AsyncSocket();
    g_hSocket.SetConnectCallback(OnSocketConnect);
    g_hSocket.SetDataCallback(OnSocketData);
    g_hSocket.SetErrorCallback(OnSocketError);

    g_hSocket.Connect(host, port);
}

void Disconnect()
{
    if (g_hSocket != null)
    {
        delete g_hSocket;
        g_hSocket = null;
    }

    g_bConnected         = false;
    g_bHandshakeComplete = false;

    if (g_hReconnectTimer != null)
    {
        delete g_hReconnectTimer;
        g_hReconnectTimer = null;
    }

    if (g_hSyncTimer != null)
    {
        delete g_hSyncTimer;
        g_hSyncTimer = null;
    }
}

void ScheduleReconnect()
{
    if (g_hReconnectTimer != null)
        return;

    float delay       = g_cvReconnectDelay.FloatValue;
    g_hReconnectTimer = CreateTimer(delay, Timer_Reconnect);
    LogMessage("[Relay] Reconnecting in %.1f seconds...", delay);
}

public Action Timer_Reconnect(Handle timer)
{
    g_hReconnectTimer = null;

    if (g_cvEnabled.BoolValue && !g_bConnected)
    {
        ConnectToRelay();
    }

    return Plugin_Stop;
}

// ============================================================================
// Socket Callbacks
// ============================================================================
public void OnSocketConnect(AsyncSocket socket)
{
    g_bConnected = true;
    LogMessage("[Relay] Connected! Sending handshake...");

    SendHandshake();
}

public void OnSocketData(AsyncSocket socket, const char[] data, int dataSize)
{
    ProcessIncomingData(data, dataSize);
}

public void OnSocketError(AsyncSocket socket, int errorType, const char[] errorName)
{
    LogError("[Relay] Socket error: type=%d, name=%s", errorType, errorName);

    Disconnect();
    ScheduleReconnect();
}

// ============================================================================
// Packet Building
// ============================================================================

int BuildPacketHeader(char[] buffer, int eventType, int payloadLen)
{
    buffer[0]   = PROTOCOL_MAGIC_1;
    buffer[1]   = PROTOCOL_MAGIC_2;
    buffer[2]   = PROTOCOL_VERSION;
    buffer[3]   = eventType;
    buffer[4]   = payloadLen & 0xFF;
    buffer[5]   = (payloadLen >> 8) & 0xFF;
    buffer[6]   = g_iServerId;
    buffer[7]   = g_iSequence & 0xFF;
    buffer[8]   = (g_iSequence >> 8) & 0xFF;

    g_iSequence = (g_iSequence + 1) & 0xFFFF;

    return HEADER_SIZE;
}

int WriteString(char[] buffer, int offset, const char[] str)
{
    int len = strlen(str);
    if (len > 255) len = 255;

    buffer[offset] = len;
    for (int i = 0; i < len; i++)
    {
        buffer[offset + 1 + i] = str[i];
    }

    return 1 + len;
}

int WriteU8(char[] buffer, int offset, int value)
{
    buffer[offset] = value & 0xFF;
    return 1;
}

int WriteU16(char[] buffer, int offset, int value)
{
    buffer[offset]     = value & 0xFF;
    buffer[offset + 1] = (value >> 8) & 0xFF;
    return 2;
}

int WriteU32(char[] buffer, int offset, int value)
{
    buffer[offset]     = value & 0xFF;
    buffer[offset + 1] = (value >> 8) & 0xFF;
    buffer[offset + 2] = (value >> 16) & 0xFF;
    buffer[offset + 3] = (value >> 24) & 0xFF;
    return 4;
}

int WriteU64(char[] buffer, int offset, const char[] steamId64)
{
    // Parse Steam ID 64-bit string to binary
    // Steam64 format: "76561198012345678" (17-18 digits)
    // We need to write 8 bytes in little-endian

    // SourcePawn doesn't have native 64-bit ints, so we parse manually
    // Split into two 32-bit parts: high and low

    int len = strlen(steamId64);
    if (len == 0)
    {
        // Empty - write zeros
        for (int i = 0; i < 8; i++)
        {
            buffer[offset + i] = 0;
        }
        return 8;
    }

    // For Steam64, we can use a simplified approach:
    // Parse the string character by character and build the bytes
    int bytes[8];
    for (int i = 0; i < 8; i++)
        bytes[i] = 0;

    // Simple decimal string to bytes conversion
    for (int i = 0; i < len; i++)
    {
        int digit = steamId64[i] - '0';
        if (digit < 0 || digit > 9) continue;

        // Multiply current value by 10 and add digit
        // bytes[0] is least significant
        int carry = digit;
        for (int j = 0; j < 8; j++)
        {
            int val  = bytes[j] * 10 + carry;
            bytes[j] = val & 0xFF;
            carry    = val >> 8;
        }
    }

    // Write to buffer (little-endian)
    for (int i = 0; i < 8; i++)
    {
        buffer[offset + i] = bytes[i];
    }

    return 8;
}

// Read Steam ID 64-bit from buffer and convert to string
int ReadU64ToString(const char[] buffer, int offset, char[] steamId64, int maxLen)
{
    // Read 8 bytes in little-endian
    int bytes[8];
    for (int i = 0; i < 8; i++)
    {
        bytes[i] = buffer[offset + i] & 0xFF;
    }

    // Convert to decimal string (reverse of WriteU64)
    // We'll build the string by repeatedly dividing by 10
    char result[24];
    int  resultLen = 0;

    // Check if all zeros
    bool isZero    = true;
    for (int i = 0; i < 8; i++)
    {
        if (bytes[i] != 0)
        {
            isZero = false;
            break;
        }
    }

    if (isZero)
    {
        strcopy(steamId64, maxLen, "0");
        return 8;
    }

    // Divide by 10 repeatedly to get digits
    while (!isZero)
    {
        int remainder = 0;
        // Divide from most significant byte
        for (int i = 7; i >= 0; i--)
        {
            int val   = remainder * 256 + bytes[i];
            bytes[i]  = val / 10;
            remainder = val % 10;
        }

        result[resultLen++] = '0' + remainder;

        // Check if all zeros
        isZero              = true;
        for (int i = 0; i < 8; i++)
        {
            if (bytes[i] != 0)
            {
                isZero = false;
                break;
            }
        }
    }

    // Reverse the string
    for (int i = 0; i < resultLen / 2; i++)
    {
        char tmp                  = result[i];
        result[i]                 = result[resultLen - 1 - i];
        result[resultLen - 1 - i] = tmp;
    }
    result[resultLen] = '\0';

    strcopy(steamId64, maxLen, result);
    return 8;
}

void SendPacket(const char[] buffer, int size)
{
    if (!g_bConnected || g_hSocket == null)
        return;

    int  crc = CalculateCRC8(buffer, size);

    char finalBuffer[MAX_PAYLOAD_SIZE + HEADER_SIZE + CHECKSUM_SIZE];
    for (int i = 0; i < size; i++)
    {
        finalBuffer[i] = buffer[i];
    }
    finalBuffer[size] = crc;

    g_hSocket.Write(finalBuffer, size + 1);
}

// ============================================================================
// Event Senders
// ============================================================================

void SendHandshake()
{
    char buffer[256];
    int  offset = HEADER_SIZE;

    offset += WriteU8(buffer, offset, g_iServerId);
    offset += WriteString(buffer, offset, g_sServerName);
    offset += WriteString(buffer, offset, g_sCurrentMap);
    offset += WriteU8(buffer, offset, MaxClients);
    offset += WriteU8(buffer, offset, GetClientCount(true));

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_HANDSHAKE, payloadLen);

    SendPacket(buffer, offset);
}

void SendHeartbeat()
{
    char buffer[64];
    int  offset = HEADER_SIZE;

    offset += WriteU32(buffer, offset, GetTime());
    offset += WriteU8(buffer, offset, GetClientCount(true));

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_HEARTBEAT, payloadLen);

    SendPacket(buffer, offset);
}

void SendChatMessage(int client, const char[] message, bool teamChat)
{
    if (!g_bHandshakeComplete) return;

    char buffer[512];
    int  offset = HEADER_SIZE;

    char playerName[64], steamId[32];
    GetClientName(client, playerName, sizeof(playerName));
    GetClientAuthId(client, AuthId_SteamID64, steamId, sizeof(steamId));

    offset += WriteString(buffer, offset, playerName);
    offset += WriteU64(buffer, offset, steamId);
    offset += WriteU8(buffer, offset, GetClientTeam(client));
    offset += WriteU8(buffer, offset, teamChat ? 1 : 0);
    offset += WriteString(buffer, offset, message);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_CHAT_MESSAGE, payloadLen);

    SendPacket(buffer, offset);
}

void SendPlayerDeath(int victim, int attacker, const char[] weapon, int critType)
{
    if (!g_bHandshakeComplete) return;

    char buffer[512];
    int  offset = HEADER_SIZE;

    char victimName[64], attackerName[64];
    char victimSteamId[32], attackerSteamId[32];

    GetClientName(victim, victimName, sizeof(victimName));
    GetClientAuthId(victim, AuthId_SteamID64, victimSteamId, sizeof(victimSteamId));

    if (attacker > 0 && attacker <= MaxClients && IsClientInGame(attacker))
    {
        GetClientName(attacker, attackerName, sizeof(attackerName));
        GetClientAuthId(attacker, AuthId_SteamID64, attackerSteamId, sizeof(attackerSteamId));
    }
    else
    {
        attackerName[0]    = '\0';
        attackerSteamId[0] = '\0';
    }

    offset += WriteString(buffer, offset, victimName);
    offset += WriteU64(buffer, offset, victimSteamId);
    offset += WriteU8(buffer, offset, GetClientTeam(victim));
    offset += WriteU8(buffer, offset, view_as<int>(TF2_GetPlayerClass(victim)));
    offset += WriteString(buffer, offset, attackerName);
    offset += WriteU64(buffer, offset, attackerSteamId);
    offset += WriteU8(buffer, offset, attacker > 0 ? GetClientTeam(attacker) : 0);
    offset += WriteU8(buffer, offset, attacker > 0 ? view_as<int>(TF2_GetPlayerClass(attacker)) : 0);
    offset += WriteString(buffer, offset, weapon);
    offset += WriteU8(buffer, offset, critType);
    offset += WriteU16(buffer, offset, 0);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_PLAYER_DEATH, payloadLen);

    SendPacket(buffer, offset);
}

void SendPlayerConnect(int client)
{
    if (!g_bHandshakeComplete) return;

    char buffer[256];
    int  offset = HEADER_SIZE;

    char playerName[64], steamId[32];
    GetClientName(client, playerName, sizeof(playerName));
    GetClientAuthId(client, AuthId_SteamID64, steamId, sizeof(steamId));

    offset += WriteString(buffer, offset, playerName);
    offset += WriteU64(buffer, offset, steamId);
    offset += WriteU32(buffer, offset, 0);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_PLAYER_CONNECT, payloadLen);

    SendPacket(buffer, offset);
}

void SendPlayerDisconnect(int client, const char[] reason)
{
    if (!g_bHandshakeComplete) return;

    char buffer[256];
    int  offset = HEADER_SIZE;

    char playerName[64], steamId[32];
    GetClientName(client, playerName, sizeof(playerName));
    GetClientAuthId(client, AuthId_SteamID64, steamId, sizeof(steamId));

    offset += WriteString(buffer, offset, playerName);
    offset += WriteU64(buffer, offset, steamId);
    offset += WriteString(buffer, offset, reason);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_PLAYER_DISCONNECT, payloadLen);

    SendPacket(buffer, offset);
}

void SendPlayerSpawn(int client)
{
    if (!g_bHandshakeComplete) return;

    char buffer[256];
    int  offset = HEADER_SIZE;

    char playerName[64], steamId[32];
    GetClientName(client, playerName, sizeof(playerName));
    GetClientAuthId(client, AuthId_SteamID64, steamId, sizeof(steamId));

    float position[3];
    GetClientAbsOrigin(client, position);

    offset += WriteString(buffer, offset, playerName);
    offset += WriteU64(buffer, offset, steamId);
    offset += WriteU8(buffer, offset, GetClientTeam(client));
    offset += WriteU8(buffer, offset, view_as<int>(TF2_GetPlayerClass(client)));
    offset += WriteFloat(buffer, offset, position[0]);
    offset += WriteFloat(buffer, offset, position[1]);
    offset += WriteFloat(buffer, offset, position[2]);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_GHOST_SPAWN, payloadLen);

    SendPacket(buffer, offset);
}

void SendRoundStart()
{
    if (!g_bHandshakeComplete) return;

    char buffer[128];
    int  offset = HEADER_SIZE;

    offset += WriteString(buffer, offset, g_sCurrentMap);
    offset += WriteU8(buffer, offset, 1);
    offset += WriteU16(buffer, offset, 0);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_ROUND_START, payloadLen);

    SendPacket(buffer, offset);
}

void SendMapChange()
{
    if (!g_bHandshakeComplete) return;

    char buffer[256];
    int  offset = HEADER_SIZE;

    char oldMap[64];
    GetCurrentMap(oldMap, sizeof(oldMap));

    offset += WriteString(buffer, offset, oldMap);
    offset += WriteString(buffer, offset, g_sCurrentMap);

    int payloadLen = offset - HEADER_SIZE;
    BuildPacketHeader(buffer, EVENT_MAP_CHANGE, payloadLen);

    SendPacket(buffer, offset);
}

// ============================================================================
// Incoming Data Processing
// ============================================================================

void ProcessIncomingData(const char[] data, int size)
{
    if (size < HEADER_SIZE + CHECKSUM_SIZE)
        return;

    if (data[0] != PROTOCOL_MAGIC_1 || data[1] != PROTOCOL_MAGIC_2)
    {
        LogError("[Relay] Invalid magic bytes");
        return;
    }

    int eventType      = data[3];
    int sourceServerId = data[6];

    // Ignore events from our own server
    if (sourceServerId == g_iServerId)
        return;

    switch (eventType)
    {
        case EVENT_HANDSHAKE_ACK:
        {
            bool success = data[HEADER_SIZE] != 0;
            if (success)
            {
                g_bHandshakeComplete = true;
                LogMessage("[Relay] Handshake complete! Server ID: %d", g_iServerId);

                CreateTimer(1.0, Timer_Heartbeat, _, TIMER_REPEAT | TIMER_FLAG_NO_MAPCHANGE);
                g_hSyncTimer = CreateTimer(GHOST_SYNC_RATE, Timer_SyncPositions, _, TIMER_REPEAT | TIMER_FLAG_NO_MAPCHANGE);
            }
            else
            {
                LogError("[Relay] Handshake rejected");
                Disconnect();
            }
        }
        case EVENT_HEARTBEAT_ACK:
        {
            // Heartbeat acknowledged
        }
        case EVENT_CHAT_MESSAGE:
        {
            ProcessRemoteChatMessage(data, size);
        }
        case EVENT_PLAYER_DEATH:
        {
            ProcessRemotePlayerDeath(data, size, sourceServerId);
        }
        case EVENT_PLAYER_CONNECT:
        {
            ProcessRemotePlayerConnect(data, size, sourceServerId);
        }
        case EVENT_PLAYER_DISCONNECT:
        {
            ProcessRemotePlayerDisconnect(data, size, sourceServerId);
        }
        case EVENT_PLAYER_SYNC:
        {
            if (g_cvGhostEnabled.BoolValue)
            {
                ProcessRemotePlayerSync(data, size, sourceServerId);
            }
        }
        case EVENT_GHOST_SPAWN:
        {
            if (g_cvGhostEnabled.BoolValue)
            {
                ProcessRemoteGhostSpawn(data, size, sourceServerId);
            }
        }
        case EVENT_GHOST_DESPAWN:
        {
            ProcessRemoteGhostDespawn(data, size, sourceServerId);
        }
        case EVENT_SERVER_DISCONNECT:
        {
            // Remove all ghosts from disconnected server
            RemoveGhostsByServer(sourceServerId);
        }
    }
}

void ProcessRemotePlayerSync(const char[] data, int size, int sourceServerId)
{
    int  offset = HEADER_SIZE;

    // Read player name
    char playerName[64];
    int  nameLen = data[offset];
    offset++;
    for (int i = 0; i < nameLen && i < 63; i++)
    {
        playerName[i] = data[offset + i];
    }
    playerName[nameLen] = '\0';
    offset += nameLen;

    // Read Steam ID (8 bytes)
    char steamId[32];
    ReadU64ToString(data, offset, steamId, sizeof(steamId));
    offset += 8;

    // Read team and class
    int team = data[offset];
    offset++;
    int classId = data[offset];
    offset++;

    // Read health
    int health = data[offset] | (data[offset + 1] << 8);
    offset += 2;

    // Read position
    float position[3];
    position[0] = ReadFloat(data, offset);
    offset += 4;
    position[1] = ReadFloat(data, offset);
    offset += 4;
    position[2] = ReadFloat(data, offset);
    offset += 4;

    // Read angles
    float angles[3];
    angles[0] = ReadFloat(data, offset);
    offset += 4;
    angles[1] = ReadFloat(data, offset);
    offset += 4;
    angles[2] = ReadFloat(data, offset);
    offset += 4;

    // Read velocity
    float velocity[3];
    velocity[0] = ReadFloat(data, offset);
    offset += 4;
    velocity[1] = ReadFloat(data, offset);
    offset += 4;
    velocity[2] = ReadFloat(data, offset);
    offset += 4;

    // Read flags
    int flags = data[offset];
    offset++;

    // Find or create ghost
    int slot = FindGhostBySteamId(steamId);
    if (slot == -1)
    {
        slot = CreateGhost(sourceServerId, steamId, playerName, team, classId);
        if (slot == -1)
            return;
    }

    // Update ghost data
    g_Ghosts[slot].health     = health;
    g_Ghosts[slot].isOnGround = (flags & 0x01) != 0;
    g_Ghosts[slot].isDucking  = (flags & 0x02) != 0;

    // Check if class changed
    if (g_Ghosts[slot].classId != classId)
    {
        g_Ghosts[slot].classId = classId;
        UpdateGhostModel(slot);
    }

    // Update position for interpolation
    UpdateGhostPosition(slot, position, angles, velocity);
}

void ProcessRemoteGhostSpawn(const char[] data, int size, int sourceServerId)
{
    int  offset = HEADER_SIZE;

    char playerName[64];
    int  nameLen = data[offset];
    offset++;
    for (int i = 0; i < nameLen && i < 63; i++)
    {
        playerName[i] = data[offset + i];
    }
    playerName[nameLen] = '\0';
    offset += nameLen;

    char steamId[32];
    ReadU64ToString(data, offset, steamId, sizeof(steamId));
    offset += 8;

    int team = data[offset];
    offset++;
    int classId = data[offset];
    offset++;

    float position[3];
    position[0] = ReadFloat(data, offset);
    offset += 4;
    position[1] = ReadFloat(data, offset);
    offset += 4;
    position[2] = ReadFloat(data, offset);
    offset += 4;

    int slot = FindGhostBySteamId(steamId);
    if (slot == -1)
    {
        slot = CreateGhost(sourceServerId, steamId, playerName, team, classId);
        if (slot == -1)
            return;
    }

    // Set initial position
    g_Ghosts[slot].position       = position;
    g_Ghosts[slot].targetPosition = position;

    PrintToChatAll("\x04[S%d] %s spawned", sourceServerId, playerName);
}

void ProcessRemoteGhostDespawn(const char[] data, int size, int sourceServerId)
{
    int  offset = HEADER_SIZE;

    char steamId[32];
    ReadU64ToString(data, offset, steamId, sizeof(steamId));
    offset += 8;

    int slot = FindGhostBySteamId(steamId);
    if (slot != -1)
    {
        RemoveGhost(slot);
    }
}

void ProcessRemoteChatMessage(const char[] data, int size)
{
    int  offset = HEADER_SIZE;

    char playerName[64], message[256];
    int  nameLen = data[offset];
    offset++;

    for (int i = 0; i < nameLen && i < 63; i++)
    {
        playerName[i] = data[offset + i];
    }
    playerName[nameLen] = '\0';
    offset += nameLen;

    offset += 8;    // Steam ID
    int team = data[offset];
    offset++;
    offset++;    // Chat type

    int msgLen = data[offset];
    offset++;
    for (int i = 0; i < msgLen && i < 255; i++)
    {
        message[i] = data[offset + i];
    }
    message[msgLen] = '\0';

    int  serverId   = data[6];

    char teamColor[16];
    if (team == 2) teamColor = "\x07BD3B3B";
    else if (team == 3) teamColor = "\x075B8DD8";
    else teamColor = "\x07CCCCCC";

    PrintToChatAll("\x01[S%d] %s%s\x01: %s", serverId, teamColor, playerName, message);
}

void ProcessRemotePlayerDeath(const char[] data, int size, int sourceServerId)
{
    int  offset = HEADER_SIZE;

    char victimName[64];
    int  nameLen = data[offset];
    offset++;
    for (int i = 0; i < nameLen && i < 63; i++)
    {
        victimName[i] = data[offset + i];
    }
    victimName[nameLen] = '\0';
    offset += nameLen;

    char steamId[32];
    Format(steamId, sizeof(steamId), "%d_%d", sourceServerId, offset);

    // Remove ghost of dead player
    int slot = FindGhostBySteamId(steamId);
    if (slot != -1)
    {
        RemoveGhost(slot);
    }

    PrintToChatAll("\x07FF0000[S%d] %s died", sourceServerId, victimName);
}

void ProcessRemotePlayerConnect(const char[] data, int size, int sourceServerId)
{
    int  offset = HEADER_SIZE;

    char playerName[64];
    int  nameLen = data[offset];
    offset++;

    for (int i = 0; i < nameLen && i < 63; i++)
    {
        playerName[i] = data[offset + i];
    }
    playerName[nameLen] = '\0';

    PrintToChatAll("\x04[S%d] %s connected", sourceServerId, playerName);
}

void ProcessRemotePlayerDisconnect(const char[] data, int size, int sourceServerId)
{
    int  offset = HEADER_SIZE;

    char playerName[64];
    int  nameLen = data[offset];
    offset++;

    for (int i = 0; i < nameLen && i < 63; i++)
    {
        playerName[i] = data[offset + i];
    }
    playerName[nameLen] = '\0';

    char steamId[32];
    Format(steamId, sizeof(steamId), "%d_%d", sourceServerId, offset);

    // Remove ghost
    int slot = FindGhostBySteamId(steamId);
    if (slot != -1)
    {
        RemoveGhost(slot);
    }

    PrintToChatAll("\x04[S%d] %s disconnected", sourceServerId, playerName);
}

// ============================================================================
// Event Handlers
// ============================================================================
public Action Event_PlayerSay(Event event, const char[] name, bool dontBroadcast)
{
    int client = GetClientOfUserId(event.GetInt("userid"));

    if (client < 1 || !IsClientInGame(client))
        return Plugin_Continue;

    char message[256];
    event.GetString("text", message, sizeof(message));

    SendChatMessage(client, message, false);

    return Plugin_Continue;
}

public Action Event_PlayerDeath(Event event, const char[] name, bool dontBroadcast)
{
    int victim   = GetClientOfUserId(event.GetInt("userid"));
    int attacker = GetClientOfUserId(event.GetInt("attacker"));

    if (victim < 1)
        return Plugin_Continue;

    char weapon[64];
    event.GetString("weapon", weapon, sizeof(weapon));

    int critType = event.GetInt("crit_type");

    SendPlayerDeath(victim, attacker, weapon, critType);

    return Plugin_Continue;
}

public Action Event_PlayerConnect(Event event, const char[] name, bool dontBroadcast)
{
    int client = GetClientOfUserId(event.GetInt("userid"));

    if (client > 0 && IsClientInGame(client))
    {
        SendPlayerConnect(client);
    }

    return Plugin_Continue;
}

public Action Event_PlayerDisconnect(Event event, const char[] name, bool dontBroadcast)
{
    int client = GetClientOfUserId(event.GetInt("userid"));

    if (client > 0)
    {
        char reason[128];
        event.GetString("reason", reason, sizeof(reason));
        SendPlayerDisconnect(client, reason);
    }

    return Plugin_Continue;
}

public Action Event_PlayerTeam(Event event, const char[] name, bool dontBroadcast)
{
    return Plugin_Continue;
}

public Action Event_PlayerSpawn(Event event, const char[] name, bool dontBroadcast)
{
    int client = GetClientOfUserId(event.GetInt("userid"));

    if (client > 0 && IsClientInGame(client) && !IsFakeClient(client))
    {
        CreateTimer(0.1, Timer_DelayedSpawn, GetClientUserId(client));
    }

    return Plugin_Continue;
}

public Action Timer_DelayedSpawn(Handle timer, int userId)
{
    int client = GetClientOfUserId(userId);
    if (client > 0 && IsClientInGame(client) && IsPlayerAlive(client))
    {
        SendPlayerSpawn(client);
    }
    return Plugin_Stop;
}

public Action Event_PlayerClass(Event event, const char[] name, bool dontBroadcast)
{
    return Plugin_Continue;
}

public Action Event_RoundStart(Event event, const char[] name, bool dontBroadcast)
{
    SendRoundStart();
    return Plugin_Continue;
}

public Action Event_RoundEnd(Event event, const char[] name, bool dontBroadcast)
{
    return Plugin_Continue;
}

// ============================================================================
// Timers
// ============================================================================
public Action Timer_Heartbeat(Handle timer)
{
    if (g_bHandshakeComplete)
    {
        SendHeartbeat();
    }

    return g_bConnected ? Plugin_Continue : Plugin_Stop;
}

// ============================================================================
// Commands
// ============================================================================
public Action Command_Reconnect(int client, int args)
{
    Disconnect();
    RemoveAllGhosts();
    ConnectToRelay();

    ReplyToCommand(client, "[Relay] Reconnecting...");
    return Plugin_Handled;
}

public Action Command_Status(int client, int args)
{
    ReplyToCommand(client, "[Relay] Status:");
    ReplyToCommand(client, "  Connected: %s", g_bConnected ? "Yes" : "No");
    ReplyToCommand(client, "  Handshake: %s", g_bHandshakeComplete ? "Complete" : "Pending");
    ReplyToCommand(client, "  Server ID: %d", g_iServerId);
    ReplyToCommand(client, "  Server Name: %s", g_sServerName);
    ReplyToCommand(client, "  Active Ghosts: %d", g_iGhostCount);

    return Plugin_Handled;
}

public Action Command_ListGhosts(int client, int args)
{
    ReplyToCommand(client, "[Relay] Active Ghosts (%d):", g_iGhostCount);

    for (int i = 0; i < MAX_GHOSTS; i++)
    {
        if (g_Ghosts[i].active)
        {
            ReplyToCommand(client, "  [%d] %s (Server %d, Class %d)",
                           i, g_Ghosts[i].playerName, g_Ghosts[i].serverId, g_Ghosts[i].classId);
        }
    }

    return Plugin_Handled;
}

// ============================================================================
// CRC-8 Implementation
// ============================================================================

void InitCRC8Table()
{
    for (int i = 0; i < 256; i++)
    {
        int crc = i;
        for (int j = 0; j < 8; j++)
        {
            if (crc & 0x80)
                crc = (crc << 1) ^ 0x07;
            else
                crc = crc << 1;
        }
        g_iCRC8Table[i] = crc & 0xFF;
    }
}

int CalculateCRC8(const char[] data, int length)
{
    int crc = 0;
    for (int i = 0; i < length; i++)
    {
        crc = g_iCRC8Table[(crc ^ data[i]) & 0xFF];
    }
    return crc;
}
