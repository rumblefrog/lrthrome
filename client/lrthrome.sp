/**
 * Lrthrome - Fast and light TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint
 * Copyright (C) 2021  rumblefrog
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

#include <sourcemod>
#include <socket>
#include <bytebuffer>

#pragma semicolon 1

#define PLUGIN_AUTHOR "rumblefrog & ziidx"
#define PLUGIN_VERSION "0.0.1"

#define PROTOCOL_VERSION 1

enum struct Connection
{
    Handle hSocket;

    bool bConnecting;

    void Create(SocketErrorCB efunc)
    {
        this.hSocket = SocketCreate(SOCKET_TCP, efunc);

        SocketSetOption(this.hSocket, SocketReuseAddr, 1);
        SocketSetOption(this.hSocket, SocketSendTimeout, 3000);

        #if defined DEBUG
        SocketSetOption(this.hSocket, DebugMode, 1);
        #endif
    }

    void Connect
        (
            SocketConnectCB cfunc,
            SocketReceiveCB rfunc,
            SocketDisconnectCB dfunc,
            const char[] hostname,
            int port
        )
    {
        if (this.bConnecting)
            return;

        this.bConnecting = true;

        SocketConnect(this.hSocket, cfunc, rfunc, dfunc, hostname, port);
    }

    void Send(const char[] data, int len)
    {
        SocketSend(this.hSocket, data, len);
    }

    bool Disconnect()
    {
        return SocketDisconnect(this.hSocket);
    }

    bool IsConnected()
    {
        return SocketIsConnected(this.hSocket);
    }
}

enum State
{
    // Pending to be sent over socket.
    Pending,

    // Sent over socket, but no reply yet.
    Sent,

    // Sent and received response, ready for removal from queue.
    Complete,
}

enum struct Queue
{
    int user_id;

    char ip_address[32];

    State state;
}

Connection g_cConnection;

enum Variant
{
    // Acknowledgement of peer connection.
    // Server public data will be transmitted to peer.
    VariantEstablished = 0,

    // Optional peer payload to server to identify or authenticate itself.
    // Authentication may grant higher limits in the future.
    VariantIdentify = 1,

    // Request to check ip address against tree.
    VariantRequest = 2,

    // Successful response indicating a longest match was found.
    VariantResponseOkFound = 3,

    // Successful response indicating no result.
    VariantResponseOkNotFound = 4,

    // Unsuccessful response.
    // This response is considered fatal, and peer should attempt at another time.
    VariantResponseError = 5,
}

/**
 * Header structure
 *
 * @field protocol_version - Current protocol version. Version is checked to ensure proper parsing on both sides.
 * @field variant - Message variant to indicate parsing procedure.
 */
methodmap Header < ByteBuffer
{
	public Header()
	{
		return view_as<Header>(CreateByteBuffer());
	}

    property int ProtocolVersion
    {
        public get()
        {
            this.Cursor = 0;

            return this.ReadByte();
        }
    }

    property Variant VariantType
    {
        public get()
        {
            this.Cursor = 1;

            return view_as<Variant>(this.ReadByte());
        }
    }

    public void WriteHeader(Variant v)
    {
        this.WriteByte(PROTOCOL_VERSION);
        this.WriteByte(view_as<int>(v));
    }

    public int DataCursor()
    {
        this.Cursor = 2;

        return this.Cursor;
    }

	public void Dispatch()
	{
		char sDump[MAX_BUFFER_LENGTH];

		int iLen = this.Dump(sDump, MAX_BUFFER_LENGTH);

		this.Close();

		if (!g_cConnection.IsConnected())
			return;

		// Len required
		// If len is not included, it will stop at the first \0 terminator
		g_cConnection.Send(sDump, iLen);
	}
}

/**
 * Established structure
 *
 * Server public data sent upon connection.
 *
 * @field rate_limit - Rate limit over the span of 5 seconds, allowing burst.
 * @field tree_size - Number of entries within the lookup tree.
 * @field banner - Optional banner message.
 */
methodmap Established < Header
{
    property int RateLimit
    {
        public get()
        {
            this.DataCursor();

            return this.ReadInt();
        }
    }

    property int TreeSize
    {
        public get()
        {
            this.Cursor = this.DataCursor() + 4;

            return this.ReadInt();
        }
    }

    property int CacheTTL
    {
        public get()
        {
            this.Cursor = this.DataCursor() + 8;

            return this.ReadInt();
        }
    }

    property int PeerTTL
    {
        public get()
        {
            this.Cursor = this.DataCursor() + 12;

            return this.ReadInt();
        }
    }

    public int Banner(char[] buffer, int buffer_len)
    {
        this.Cursor = this.DataCursor() + 16;

        return this.ReadString(buffer, buffer_len);
    }
}

/**
 * Identify structure
 *
 * @field id - Identification token.
 */
methodmap Identify < Header
{
    public Identify(const char[] id)
    {
        Header header = new Header();

        header.WriteHeader(VariantIdentify);
        header.WriteString(id);

        return view_as<Identify>(header);
    }
}

/**
 * Request structure
 *
 * @field ip_address - IP address to check filter for
 * @field meta_size - Number of entries in the meta kv map
 * @field meta - Repeated key/value pairs
 *    @repeated key
 *    @repeated value
 */
methodmap Request < Header
{
	public Request(int ip_address, StringMap meta)
	{
        Header header = Header();

        StringMapSnapshot ss = meta.Snapshot();

        header.WriteHeader(VariantRequest);
        header.WriteInt(ip_address);
        header.WriteByte(meta.Size);

        char key[64], value[64];

        for (int i = 0; i < ss.Length; i += 1)
        {
            ss.GetKey(i, key, sizeof key);
            meta.GetString(key, value, sizeof value);

            header.WriteString(key);
            header.WriteString(value);
        }

        delete ss;

        return view_as<Request>(header);
	}
}

/**
 * ResponseOkFound structure
 *
 * @field ip_address - IP address in which the result was found.
 * @field prefix - Longest match prefixed for the IP address.
 * @field mask_len - Prefix mask length.
 */
methodmap ResponseOkFound < Header
{
   property int IpAddress
   {
       public get()
       {
           this.Cursor = this.DataCursor();

           return this.ReadInt();
       }
   }

   property int Prefix
   {
       public get()
       {
           this.Cursor = this.DataCursor() + 4;

           return this.ReadInt();
       }
   }

   property int MaskLength
   {
       public get()
       {
           this.Cursor = this.DataCursor() + 8;

           return this.ReadInt();
       }
   }
}

/**
 * ResponseOkNotFound structure
 *
 * @field ip_address - IP address in which the result was not found.
 *
 */
methodmap ResponseOkNotFound < Header
{
    property int IpAddress
    {
        public get()
        {
            this.DataCursor();

            return this.ReadInt();
        }
    }
}

/**
 * ResponseError structure
 *
 * @field code - Corresponding error code for the message. Useful for peer-side handling of error.
 * @field message - Human facing error message.
 */
methodmap ResponseError < Header
{
    property int Code
    {
        public get()
        {
            this.DataCursor();

            return this.ReadByte();
        }
    }

    public int Message(char[] buffer, int buffer_len)
    {
        this.Cursor = this.DataCursor() + 1;

        return this.ReadString(buffer, buffer_len);
    }
}

ArrayList g_aQueue;

ConVar g_cHost;
ConVar g_cPort;

char g_sHost[64] = "127.0.0.1";

int g_iPort = 25597;

public Plugin myinfo =
{
	name = "Lrthrome",
	author = PLUGIN_AUTHOR,
	description = "Fast TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint.",
	version = PLUGIN_VERSION,
	url = "https://github.com/rumblefrog/lrthrome"
};

public void OnPluginStart()
{
    CreateConVar("lrthrome_version", PLUGIN_VERSION, "Lrthrome Version", FCVAR_SPONLY | FCVAR_REPLICATED | FCVAR_NOTIFY | FCVAR_DONTRECORD);

    g_cHost = CreateConVar("lrthrome_host", "127.0.0.1", "Lrthrome server host address", FCVAR_PROTECTED);

    g_cPort = CreateConVar("lrthrome_port", "25597", "Lrthrome server host port", FCVAR_PROTECTED);

    RegAdminCmd("lrthrome_override", DummyCmd, ADMFLAG_RESERVATION, "Lrthrome override, users with access skips IP lookup");

    g_cConnection.Create(OnSocketError);

    g_aQueue = new ArrayList(sizeof Queue);
}

public void OnConfigExecuted()
{
    g_cHost.GetString(g_sHost, sizeof g_sHost);
    g_iPort = g_cPort.IntValue;
}

public void OnClientPostAdminCheck(int client)
{
    if (!IsClientConnected(client))
        return;

    if (IsFakeClient(client))
        return;

    if (CheckCommandAccess(client, "lrthrome_override", ADMFLAG_RESERVATION))
        return;

    if (g_cConnection.IsConnected())
    {
        ProcessUser(client);

        return;
    }

    Queue queue;

    queue.user_id = GetClientUserId(client);
    queue.state = Pending;
    GetClientIP(client, queue.ip_address, sizeof Queue::ip_address, true);

    // Otherwise, push to queue and process upon connection
    g_aQueue.PushArray(queue);

    g_cConnection.Connect(OnSocketConnect, OnSocketReceive, OnSocketDisconnect, g_sHost, g_iPort);
}

void ProcessUser(int client)
{
    char steamid[32], ip_str[32];

    if(!GetClientAuthId(client, AuthId_Steam3, steamid, sizeof steamid))
        return;

    if (!GetClientIP(client, ip_str, sizeof ip_str, true))
        return;

    int ip;

    if ((ip = IPToLong(ip_str)) == 0)
        return;

    StringMap meta = new StringMap();

    meta.SetString("steamid", steamid);

    Request(ip, meta).Dispatch();

    delete meta;
}

// Clear queue of complete requests
void PurgeQueue()
{
    Queue queue;

    // Reversal to prevent disruption of index order with Erase
    for (int i = g_aQueue.Length - 1; i >= 0; i -= 1)
    {
        g_aQueue.GetArray(i, queue);

        if (queue.state == Complete)
            g_aQueue.Erase(i);
    }
}

public Action DummyCmd(int c, int i) {}

public void OnSocketConnect(Handle socket, any arg)
{
    g_cConnection.bConnecting = false;

    int client;

    Queue queue;

    for (int i = 0; i < g_aQueue.Length; i += 1)
    {
        g_aQueue.GetArray(i, queue);

        if (queue.state == Pending)
        {
            if ((client = GetClientOfUserId(queue.user_id)) != 0)
            {
                queue.state = Sent;

                ProcessUser(client);
            }
            // Client no longer exists, mark for complete
            else
            {
                queue.state = Complete;
            }

            g_aQueue.SetArray(i, queue);
        }
    }
}

public void OnSocketReceive(Handle socket, const char[] receiveData, const int dataSize, any arg)
{
    Header header = view_as<Header>(CreateByteBuffer(true, receiveData, dataSize));

    if (header.ProtocolVersion != PROTOCOL_VERSION)
        SetFailState("Mismatching protocol version (expected: %i) (received: %i)", PROTOCOL_VERSION, header.ProtocolVersion);

    switch (header.VariantType)
    {
        case VariantEstablished:
        {
            Established e = view_as<Established>(header);

            char banner[128];

            e.Banner(banner, sizeof banner);

            PrintToServer("============ Lrthrome Established ============");
            PrintToServer("Rate-limit: %i", e.RateLimit);
            PrintToServer("Tree Size: %i", e.TreeSize);
            PrintToServer("Cache TTL: %i", e.CacheTTL);
            PrintToServer("Peer TTL: %i", e.PeerTTL);
            PrintToServer("Banner: %s", banner);
            PrintToServer("============ Lrthrome Established ============");
        }
        case VariantResponseOkFound:
        {
            ResponseOkFound r = view_as<ResponseOkFound>(header);

            char ip[32], prefix[32];

            LongToIP(r.IpAddress, ip, sizeof ip);
            LongToIP(r.Prefix, prefix, sizeof prefix);

            Queue queue;

            for (int i = 0; i < g_aQueue.Length; i += 1)
            {
                g_aQueue.GetArray(i, queue);

                if (queue.state != Sent)
                    continue;

                if (StrEqual(ip, queue.ip_address))
                {
                    queue.state = Complete;
                    g_aQueue.SetArray(i, queue);

                    int client;

                    if ((client = GetClientOfUserId(queue.user_id)) == 0)
                        break;

                    char steamid[32];

                    if (!GetClientAuthId(client, AuthId_Steam3, steamid, sizeof steamid))
                        break;

                    // TODO: Translation support
                    KickClient(client, "Lrthrome Filtered");

                    LogMessage("Lrthrome: Kicked %N (%s) (%s) in range of %s/%i", client, steamid, ip, prefix, r.MaskLength);

                    break;
                }
            }

            PurgeQueue();
        }
        case VariantResponseOkNotFound:
        {
            // No addition handling necessary
        }
        case VariantResponseError:
        {
            ResponseError r = view_as<ResponseError>(header);

            char error_msg[64];

            r.Message(error_msg, sizeof error_msg);

            LogError("Lrthrome error: %s", error_msg);

            g_cConnection.Disconnect();

            // Iterate local records of requests, requests in Sent state is re-queued to Pending
            Queue queue;

            for (int i = 0; i < g_aQueue.Length; i += 1)
            {
                g_aQueue.GetArray(i, queue);

                if (queue.state == Sent)
                {
                    queue.state = Pending;

                    g_aQueue.SetArray(i, queue);
                }
            }
        }
        default:
        {
            // ???
        }
    }

    header.Close();
}

public void OnSocketDisconnect(Handle socket, any arg)
{
    g_cConnection.bConnecting = false;
    g_cConnection.Disconnect();
}

public void OnSocketError(Handle socket, const int errorType, const int errorNum, any arg)
{
    g_cConnection.bConnecting = false;
    g_cConnection.Disconnect();

    if (errorType == EMPTY_HOST)
        SetFailState("Empty host address provided");

    if (errorType == CONNECT_ERROR)
        LogError("Unable to connect: %i", errorNum);
}

void LongToIP(int ip, char[] buffer, int buffer_size)
{
    Format(
        buffer,
        buffer_size,
        "%i.%i.%i.%i",
        (ip >> 24) & 0xFF,
        (ip >> 16) & 0xFF,
        (ip >> 8) & 0xFF,
        ip & 0xFF
    );
}

int IPToLong(const char[] ip)
{
    char octets[4][4];

    if (ExplodeString(ip, ".", octets, sizeof octets, sizeof octets[]) != 4)
        return 0;

    return (
        StringToInt(octets[0]) << 24 |
        StringToInt(octets[1]) << 16 |
        StringToInt(octets[2]) << 8 |
        StringToInt(octets[3])
    );
}
