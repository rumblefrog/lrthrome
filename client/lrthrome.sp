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

    float fConnectedAt;
    float fLastRequest;

    void Create(SocketErrorCB efunc)
    {
        this.hSocket = SocketCreate(SOCKET_TCP, efunc);

        SocketSetOption(this.hSocket, SocketReuseAddr, 1);
        SocketSetOption(this.hSocket, SocketKeepAlive, 1);
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

        this.fLastRequest = GetGameTime();
    }

    void TemperConnect()
    {
        this.fConnectedAt = GetGameTime();
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

            this.ReadByte();
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

	public void Dispatch()
	{
		char sDump[MAX_BUFFER_LENGTH];

		int iLen = this.Dump(sDump, MAX_BUFFER_LENGTH);

		this.Close();

		if (g_cConnection.IsConnected())
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
            this.Cursor = 0;

            this.ReadInt();
        }
    }

    property int TreeSize
    {
        public get()
        {
            this.Cursor = 4;

            this.ReadInt();
        }
    }

    property int CacheTTL
    {
        public get()
        {
            this.Cursor = 8;

            this.ReadInt();
        }
    }

    property int PeerTTL
    {
        public get()
        {
            this.Cursor = 12;

            this.ReadInt();
        }
    }

    public int Banner(char[] buffer, int buffer_len)
    {
        this.Cursor = 16;

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
           this.Cursor = 0;

           return this.ReadInt();
       }
   }

   property int Prefix
   {
       public get()
       {
           this.Cursor = 4;

           return this.ReadInt();
       }
   }

   property int MaskLength
   {
       public get()
       {
           this.Cursor = 8;

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
            return this.ReadByte();
        }
    }

    public int Message(char[] buffer, int buffer_len)
    {
        return this.ReadString(buffer, buffer_len);
    }
}

// ArrayList of user reference ids to be processed
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

    RegAdminCmd("lrthrome_override", DummyCmd, ADMFLAG_GENERIC, "Lrthrome override, users with access skips IP lookup");

    g_cConnection.Create(OnSocketError);

    g_aQueue = new ArrayList();
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

    if (CheckCommandAccess(client, "lrthrome_override", ADMFLAG_GENERIC))
        return;

    if (g_cConnection.IsConnected())
    {
        ProcessUser(client);

        return;
    }

    // Otherwise, push to queue and process upon connection
    g_aQueue.Push(GetClientUserId(client));

    g_cConnection.Connect(OnSocketConnect, OnSocketReceive, OnSocketDisconnect, g_sHost, g_iPort);
}

void ProcessUser(int client)
{
    // Client may have disconnected by socket connection
    if (!IsClientConnected(client))
        return;

    char steamid[32], ip_str[32];

    GetClientAuthId(client, AuthId_Steam3, steamid, sizeof steamid);
    GetClientIP(client, ip_str, sizeof ip_str, true);

    int ip;

    if ((ip = IPToLong(ip_str)) == 0)
        return;

    StringMap meta = new StringMap();

    meta.SetString("steamid", steamid);

    Request(ip, meta).Dispatch();

    delete meta;
}

public Action DummyCmd(int c, int i) {}

public void OnSocketConnect(Handle socket, any arg)
{
    g_cConnection.bConnecting = false;

    for (int i = 0; i < g_aQueue.Length; i += 1)
        ProcessUser(g_aQueue.Get(i));

    g_aQueue.Clear();
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

            PrintToServer("============ Lrthrome Established ============");
            PrintToServer("Rate-limit: %i", e.RateLimit);
            PrintToServer("Tree Size: %i", e.TreeSize);
            PrintToServer("Cache TTL: %i", e.CacheTTL);
            PrintToServer("Peer TTL: %i", e.PeerTTL);
            PrintToServer("============ Lrthrome Established ============");
        }
        case VariantResponseOkFound:
        {
            ResponseOkFound r = view_as<ResponseOkFound>(header);

            char ip[32], prefix[32];

            LongToIP(r.IpAddress, ip, sizeof ip);
            LongToIP(r.Prefix, prefix, sizeof prefix);

            char client_ip[32];

            for (int i = 1; i <= MaxClients; i += 1)
            {
                if (!IsClientConnected(i))
                    continue;

                if (!GetClientIP(i, client_ip, sizeof client_ip, true))
                    continue;

                if (StrEqual(ip, client_ip))
                {
                    char steamid[32];

                    if (!GetClientAuthId(i, AuthId_Steam3, steamid, sizeof steamid))
                        continue;

                    // TODO: Translation support
                    KickClient(i, "Lrthrome Filtered");

                    LogMessage("Lrthrome: Kicked %N (%s) (%s) in range of %s/%i", i, steamid, ip, prefix, r.MaskLength);

                    break;
                }
            }
        }
        case VariantResponseOkNotFound:
        {
            // No addition handling necessary
        }
        case VariantResponseError:
        {
            ResponseError r = view_as<ResponseError>(header);

            // TODO: Keep a local record of requests sent and received
            // For all requests sent and not received, it will re-queue for next connection
            char error_msg[64];

            r.Message(error_msg, sizeof error_msg);

            LogError("Lrthrome error: %s", error_msg);

            g_cConnection.Disconnect();
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
}

public void OnSocketError(Handle socket, const int errorType, const int errorNum, any arg)
{
    g_cConnection.bConnecting = false;

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
