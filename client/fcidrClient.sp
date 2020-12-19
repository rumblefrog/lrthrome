#include "SCR-Version"

#include <sourcemod>
#include <socket>
#include <bytebuff>

#define PROTOCOL_VERSION 1


/**
 * Base message structure
 *
 */
methodmap Header < ByteBuffer
{
	public Header()
	{
		return view_as<Header>(CreateByteBuffer());
	}

	public void baseHeader()
	{
<<<<<<< HEAD
		this.writeByte(PROTOCOL_VERSION);
=======
		this.WriteByte(PROTOCOL_VERSION);
>>>>>>> 39374856cfba3eb068559717518d602ce2c85a0c
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

methodmap Request < Header
{
	public Request(int ip_address, StringMap() keyvals)
	{
		BaseMessage mreq = BaseMessage();
		StringMapSnapshot ss = keyvals.Snapshot();

		mreq.baseHeader();
		mreq.WriteInt(ip_address);
		mreq.WriteInt(keyvals.Size);

		for(int i = 0; i < ss.Length; i++){
			ss.getKey(i, keyBuf);
			keyvals.getString(keyBuf, valBuf);
			mreq.WriteString(keyBuf);
			mreq.WriteString(valBuf);
		}

		delete ss;
	}

}


methodmap Response < BaseMessage
{
	public void responseMessage(int ip_address/*,byte info_byte*/)
	{
		BaseMessage mres = BaseMessage();
		/*

		char[] infoByte = info_byte.ConvertToString();
		int infilByte = <int>infoByte[0]
		char[7] limitByte = {};

		for(int i = 1; i < 8; i++){
			limitByte[i] = infoByte[i];
		}

		mres.writeByte(infilByte);
		mres.writeByte(<int> limitByte);
		mres.writeInt(ip_address);

		*/
	}
}


public void OnClientPostAdminCheck(int client)
{
	StringMap mx = new StringMap();
	char keyBuf[20];
	char valBuf[20];
	if (!IsClientConnected(client))
		return;
}

public void OnPluginStart()
{
	g_hSocket = SocketCreate(SOCKET_TCP, OnSocketError);

	SocketSetOption(g_hSocket, SocketReuseAddr, 1);
	SocketSetOption(g_hSocket, SocketKeepAlive, 1);

	#if defined DEBUG
	SocketSetOption(g_hSocket, DebugMode, 1);
	#endif
}
