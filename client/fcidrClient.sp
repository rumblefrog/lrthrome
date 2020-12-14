#include "SCR-Version"

#include <sourcemod>
#include <socket>
#include <morecolors> // Morecolors defines a max buffer as well as bytebuffer but bytebuffer does if defined check
#include <bytebuffer>

#pragma semicolon 1

#define PLUGIN_AUTHOR "Fishy"

#define MAX_EVENT_NAME_LENGTH 128
#define MAX_COMMAND_LENGTH 512
#define PROTOCOL_VERSION 1

#pragma newdecls required

char g_sHostname[64];
char g_sHost[64] = "127.0.0.1";
char g_sToken[64];
char g_sPrefix[8];

int g_iPort = 57452;
int g_iFlag;

bool g_bFlag;

// Core convars
ConVar g_cHost;
ConVar g_cPort;
ConVar g_cPrefix;
ConVar g_cFlag;
ConVar g_cHostname;

// Event convars
ConVar g_cPlayerEvent;
ConVar g_cMapEvent;

// Socket connection handle
Handle g_hSocket;

// Forward handles
Handle g_hMessageSendForward;
Handle g_hMessageReceiveForward;
Handle g_hEventSendForward;
Handle g_hEventReceiveForward;

EngineVersion g_evEngine;

enum MessageType
{
	MessageInvalid = 0,
	MessageAuthenticate,
	MessageAuthenticateResponse,
	MessageChat,
	MessageEvent,
	MessageTypeCount,
}

enum AuthenticateResponse
{
	AuthenticateInvalid = 0,
	AuthenticateSuccess,
	AuthenticateDenied,
	AuthenticateResponseCount,
}

enum IdentificationType
{
	IdentificationInvalid = 0,
	IdentificationSteam,
	IdentificationDiscord,
	IdentificationTypeCount,
}

/**
 * Base message structure
 * 
 */
methodmap BaseMessage < ByteBuffer
{
	public BaseMessage()
	{
		return view_as<BaseMessage>(CreateByteBuffer());
	}

	public int ReadDiscardString()
	{
		char cByte;

		for(int i = 0; i < MAX_BUFFER_LENGTH; i++) {
			cByte = this.ReadByte();
			
			if(cByte == '\0') {
				return i + 1;
			}
		}
		
		return MAX_BUFFER_LENGTH;
	}

	public void Dispatch()
	{
		char sDump[MAX_BUFFER_LENGTH];

		int iLen = this.Dump(sDump, MAX_BUFFER_LENGTH);

		this.Close();

		if (!SocketIsConnected(g_hSocket))
			return;

		// Len required
		// If len is not included, it will stop at the first \0 terminator
		SocketSend(g_hSocket, sDump, iLen);
	}
}

methodmap Request < BaseMessage
{

	public requestMessage(int ip_address, StringMap() keyvals)
	{
		BaseMessage m = BaseMessage();
		m.writeByte(PROTOCOL_VERSION);
		m.writeInt(ip_address);
		m.writeInt(keyvals.Size);
		for(int i = 0; i < keyvals.Snapshot().Length; i++){
			m.writeString(keyvals)
		}
	}

}


/*methodmap Response < BaseMessage
{
	
	public 

}*/


public void OnClientPostAdminCheck(int client)
{		
	StringMap mx = new StringMap();

	if (!IsClientConnected(client))
		return;
}

public Plugin myinfo = 
{
	name = "Source Chat Relay",
	author = PLUGIN_AUTHOR,
	description = "Communicate between Discord & In-Game, monitor server without being in-game, control the flow of messages and user base engagement!",
	version = PLUGIN_VERSION,
	url = "https://keybase.io/RumbleFrog"
};

public APLRes AskPluginLoad2(Handle myself, bool late, char[] error, int err_max)
{
	RegPluginLibrary("Source-Chat-Relay");

	CreateNative("SCR_SendMessage", Native_SendMessage);
	CreateNative("SCR_SendEvent", Native_SendEvent);

	return APLRes_Success;
}

public void OnPluginStart()
{
	CreateConVar("rf_scr_version", PLUGIN_VERSION, "Source Chat Relay Version", FCVAR_REPLICATED | FCVAR_SPONLY | FCVAR_DONTRECORD | FCVAR_NOTIFY);

	g_cHost = CreateConVar("rf_scr_host", "127.0.0.1", "Relay Server Host", FCVAR_PROTECTED);

	g_cPort = CreateConVar("rf_scr_port", "57452", "Relay Server Port", FCVAR_PROTECTED);
	
	g_cPrefix = CreateConVar("rf_scr_prefix", "", "Prefix required to send message to Discord. If empty, none is required.", FCVAR_NONE);
	
	g_cFlag = CreateConVar("rf_scr_flag", "", "If prefix is enabled, this admin flag is required to send message using the prefix", FCVAR_PROTECTED);

	g_cHostname = CreateConVar("rf_scr_hostname", "", "The hostname/displayname to send with messages. If left empty, it will use the server's hostname", FCVAR_NONE);

	// Start basic event convars
	g_cPlayerEvent = CreateConVar("rf_scr_event_player", "0", "Enable player connect/disconnect events", FCVAR_NONE, true, 0.0, true, 1.0);
	
	g_cMapEvent = CreateConVar("rf_scr_event_map", "0", "Enable map start/end events", FCVAR_NONE, true, 0.0, true, 1.0);
	
	AutoExecConfig(true, "Source-Server-Relay");
	
	g_hSocket = SocketCreate(SOCKET_TCP, OnSocketError);

	SocketSetOption(g_hSocket, SocketReuseAddr, 1);
	SocketSetOption(g_hSocket, SocketKeepAlive, 1);
	
	#if defined DEBUG
	SocketSetOption(g_hSocket, DebugMode, 1);
	#endif