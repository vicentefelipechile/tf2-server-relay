/**
 * TF2 Relay Include File
 * 
 * Provides natives and forwards for other plugins to interact with the relay.
 */

#if defined _tf2_relay_included
  #endinput
#endif
#define _tf2_relay_included

/**
 * Check if the relay connection is active.
 * 
 * @return  True if connected and handshake complete, false otherwise.
 */
native bool TF2Relay_IsConnected();

/**
 * Get the local server ID.
 * 
 * @return  Server ID (1-4).
 */
native int TF2Relay_GetServerId();

/**
 * Send a custom event to all connected servers.
 * 
 * @param eventName     Custom event identifier (max 32 chars).
 * @param data          Key-value data as JSON string.
 * @return              True if sent successfully.
 */
native bool TF2Relay_SendCustomEvent(const char[] eventName, const char[] data);

/**
 * Called when a chat message is received from another server.
 * 
 * @param serverId      Source server ID.
 * @param playerName    Player name who sent the message.
 * @param steamId       Player's Steam ID (64-bit as string).
 * @param team          Team ID (2=RED, 3=BLU).
 * @param message       Chat message content.
 */
forward void TF2Relay_OnRemoteChat(int serverId, const char[] playerName, const char[] steamId, int team, const char[] message);

/**
 * Called when a player connect is received from another server.
 * 
 * @param serverId      Source server ID.
 * @param playerName    Player name.
 * @param steamId       Player's Steam ID (64-bit as string).
 */
forward void TF2Relay_OnRemoteConnect(int serverId, const char[] playerName, const char[] steamId);

/**
 * Called when a player disconnect is received from another server.
 * 
 * @param serverId      Source server ID.
 * @param playerName    Player name.
 * @param reason        Disconnect reason.
 */
forward void TF2Relay_OnRemoteDisconnect(int serverId, const char[] playerName, const char[] reason);

/**
 * Called when a player death is received from another server.
 * 
 * @param serverId      Source server ID.
 * @param victimName    Victim's name.
 * @param attackerName  Attacker's name (empty if world).
 * @param weapon        Weapon used.
 */
forward void TF2Relay_OnRemoteDeath(int serverId, const char[] victimName, const char[] attackerName, const char[] weapon);

public SharedPlugin __pl_tf2_relay = {
    name = "tf2_relay_client",
    file = "tf2_relay_client.smx",
#if defined REQUIRE_PLUGIN
    required = 1,
#else
    required = 0,
#endif
};

#if !defined REQUIRE_PLUGIN
public void __pl_tf2_relay_SetNTVOptional()
{
    MarkNativeAsOptional("TF2Relay_IsConnected");
    MarkNativeAsOptional("TF2Relay_GetServerId");
    MarkNativeAsOptional("TF2Relay_SendCustomEvent");
}
#endif
