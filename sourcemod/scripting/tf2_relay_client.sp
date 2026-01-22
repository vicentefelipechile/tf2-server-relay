/**
 * TF2 Relay Client - SourceMod Plugin
 *
 * Connects TF2 server to the Rust relay server for cross-server communication.
 * Requires: AsyncSocket Extension (https://github.com/srcdslab/sm-ext-asyncsocket)
 *
 * @author TF2 Server Relay Team
 * @version 1.0.0
 */

#include <sourcemod>
#include <sdktools>
#include <tf2>
#include <tf2_stocks>
#include <asyncsocket>

#pragma semicolon 1
#pragma newdecls required

// ============================================================================
// Protocol Constants
// ============================================================================

#define PROTOCOL_MAGIC_1         0x54    // 'T'
#define PROTOCOL_MAGIC_2         0x46    // 'F'
#define PROTOCOL_VERSION         1
#define MAX_PAYLOAD_SIZE         4096
#define HEADER_SIZE              9
#define CHECKSUM_SIZE            1

// Event Type IDs
#define EVENT_HANDSHAKE          0x00
#define EVENT_HANDSHAKE_ACK      0x01
#define EVENT_HEARTBEAT          0x02
#define EVENT_HEARTBEAT_ACK      0x03
#define EVENT_SERVER_CONNECT     0x04
#define EVENT_SERVER_DISCONNECT  0x05

#define EVENT_CHAT_MESSAGE       0x10
#define EVENT_PLAYER_DEATH       0x20
#define EVENT_PLAYER_CONNECT     0x21
#define EVENT_PLAYER_DISCONNECT  0x22
#define EVENT_PLAYER_TEAM_CHANGE 0x23

#define EVENT_ROUND_START        0x40
#define EVENT_ROUND_END          0x41
#define EVENT_MAP_CHANGE         0x42

// ============================================================================
// Plugin Info
// ============================================================================
public Plugin myinfo =
{
    name        = "TF2 Relay Client",
    author      = "SummerTYT / vicentefelipechile",
    description = "Cross-server communication via Rust relay",
    version     = "1.0.0",
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

// CRC-8 lookup table
int         g_iCRC8Table[256];

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

    // Auto-execute config
    AutoExecConfig(true, "tf2_relay");

    // Hook events
    HookEvent("player_say", Event_PlayerSay);
    HookEvent("player_death", Event_PlayerDeath);
    HookEvent("player_connect", Event_PlayerConnect);
    HookEvent("player_disconnect", Event_PlayerDisconnect);
    HookEvent("player_team", Event_PlayerTeam);
    HookEvent("teamplay_round_start", Event_RoundStart);
    HookEvent("teamplay_round_win", Event_RoundEnd);

    // Get current map
    GetCurrentMap(g_sCurrentMap, sizeof(g_sCurrentMap));

    // Register commands
    RegAdminCmd("sm_relay_reconnect", Command_Reconnect, ADMFLAG_ROOT, "Force reconnect to relay");
    RegAdminCmd("sm_relay_status", Command_Status, ADMFLAG_GENERIC, "Show relay status");

    // Initialize state
    g_bConnected         = false;
    g_bHandshakeComplete = false;
    g_iSequence          = 0;
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
    Disconnect();
}

public void OnMapStart()
{
    GetCurrentMap(g_sCurrentMap, sizeof(g_sCurrentMap));

    // Send map change if connected
    if (g_bHandshakeComplete)
    {
        SendMapChange();
    }
}

// ============================================================================
// Connection Management
// ============================================================================

void ConnectToRelay()
{
    if (g_bConnected)
    {
        return;
    }

    char host[128];
    g_cvRelayHost.GetString(host, sizeof(host));
    int port = g_cvRelayPort.IntValue;

    LogMessage("[Relay] Connecting to %s:%d...", host, port);

    // Create socket
    g_hSocket = new AsyncSocket();
    g_hSocket.SetConnectCallback(OnSocketConnect);
    g_hSocket.SetDataCallback(OnSocketData);
    g_hSocket.SetErrorCallback(OnSocketError);
    g_hSocket.SetCloseCallback(OnSocketClose);

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
}

void ScheduleReconnect()
{
    if (g_hReconnectTimer != null)
    {
        return;
    }

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
public void OnSocketConnect(AsyncSocket socket, any arg)
{
    g_bConnected = true;
    LogMessage("[Relay] Connected! Sending handshake...");

    SendHandshake();
}

public void OnSocketData(AsyncSocket socket, const char[] data, int dataSize, any arg)
{
    ProcessIncomingData(data, dataSize);
}

public void OnSocketError(AsyncSocket socket, int errorType, int errorNum, any arg)
{
    LogError("[Relay] Socket error: type=%d, num=%d", errorType, errorNum);

    Disconnect();
    ScheduleReconnect();
}

public void OnSocketClose(AsyncSocket socket, any arg)
{
    LogMessage("[Relay] Connection closed");

    g_bConnected         = false;
    g_bHandshakeComplete = false;
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

int WriteU64(char[] buffer, int offset, const char[] steamId)
{
    // Convert Steam ID to 64-bit - simplified placeholder
    // In production, use proper Steam ID conversion
    for (int i = 0; i < 8; i++)
    {
        buffer[offset + i] = 0;
    }
    return 8;
}

void SendPacket(const char[] buffer, int size)
{
    if (!g_bConnected || g_hSocket == null)
    {
        return;
    }

    // Calculate CRC
    int  crc = CalculateCRC8(buffer, size);

    // Create final buffer with CRC
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

    // Payload
    offset += WriteU8(buffer, offset, g_iServerId);
    offset += WriteString(buffer, offset, g_sServerName);
    offset += WriteString(buffer, offset, g_sCurrentMap);
    offset += WriteU8(buffer, offset, MaxClients);
    offset += WriteU8(buffer, offset, GetClientCount(true));

    // Build header
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

    char playerName[64];
    GetClientName(client, playerName, sizeof(playerName));

    char steamId[32];
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
    offset += WriteU16(buffer, offset, 0);    // Death flags

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
    offset += WriteU32(buffer, offset, 0);    // IP hash (privacy)

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

void SendRoundStart()
{
    if (!g_bHandshakeComplete) return;

    char buffer[128];
    int  offset = HEADER_SIZE;

    offset += WriteString(buffer, offset, g_sCurrentMap);
    offset += WriteU8(buffer, offset, 1);     // Round number
    offset += WriteU16(buffer, offset, 0);    // Time limit

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
    {
        return;
    }

    // Verify magic
    if (data[0] != PROTOCOL_MAGIC_1 || data[1] != PROTOCOL_MAGIC_2)
    {
        LogError("[Relay] Invalid magic bytes");
        return;
    }

    int eventType = data[3];

    switch (eventType)
    {
        case EVENT_HANDSHAKE_ACK:
        {
            bool success = data[HEADER_SIZE] != 0;
            if (success)
            {
                g_bHandshakeComplete = true;
                LogMessage("[Relay] Handshake complete! Server ID: %d", g_iServerId);

                // Start heartbeat timer
                CreateTimer(1.0, Timer_Heartbeat, _, TIMER_REPEAT | TIMER_FLAG_NO_MAPCHANGE);
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
            // Display chat from other servers
            ProcessRemoteChatMessage(data, size);
        }
        case EVENT_PLAYER_DEATH:
        {
            // Display death from other servers
            ProcessRemotePlayerDeath(data, size);
        }
        case EVENT_PLAYER_CONNECT:
        {
            ProcessRemotePlayerConnect(data, size);
        }
        case EVENT_PLAYER_DISCONNECT:
        {
            ProcessRemotePlayerDisconnect(data, size);
        }
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

    int  serverId   = data[6];    // From header

    char teamColor[16];
    if (team == 2) teamColor = "\x07BD3B3B";         // RED
    else if (team == 3) teamColor = "\x075B8DD8";    // BLU
    else teamColor = "\x07CCCCCC";

    PrintToChatAll("\x01[S%d] %s%s\x01: %s", serverId, teamColor, playerName, message);
}

void ProcessRemotePlayerDeath(const char[] data, int size)
{
    // Parse and display remote death notification
    int serverId = data[6];

    // Simplified - just log it
    LogMessage("[Relay] Death notification from Server %d", serverId);
}

void ProcessRemotePlayerConnect(const char[] data, int size)
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

    int serverId        = data[6];

    PrintToChatAll("\x04[S%d] %s connected", serverId, playerName);
}

void ProcessRemotePlayerDisconnect(const char[] data, int size)
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

    int serverId        = data[6];

    PrintToChatAll("\x04[S%d] %s disconnected", serverId, playerName);
}

// ============================================================================
// Event Handlers
// ============================================================================
public Action Event_PlayerSay(Event event, const char[] name, bool dontBroadcast = false)
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
    // Could send team change event
    return Plugin_Continue;
}

public Action Event_RoundStart(Event event, const char[] name, bool dontBroadcast)
{
    SendRoundStart();
    return Plugin_Continue;
}

public Action Event_RoundEnd(Event event, const char[] name, bool dontBroadcast)
{
    // Could send round end event
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
