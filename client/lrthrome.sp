#include <sourcemod>
#include <socket>
#include <bytebuffer>

#define PROTOCOL_VERSION 1

Handle g_hSocket;

enum Variant {
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

		if (!SocketIsConnected(g_hSocket))
			return;

		// Len required
		// If len is not included, it will stop at the first \0 terminator
		SocketSend(g_hSocket, sDump, iLen);
	}
}

/**
 * Established structure
 *
 * Server public data sent upon connection.
 *
 * @field rate_limit - Rate limit over the span of 5 seconds, allowing burst.
 * @field tree_size - Number of entires within the lookup tree.
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

    public int Banner(char[] buffer, int buffer_len)
    {
        this.Cursor = 8;

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

        return view_as<Identify(header);
    }
}

methodmap Request < Header
{
	public Request(int ip_address, StringMap meta)
	{
        Header header = new Header();

        StringMapSnapshot ss = meta.Snapshot();

        header.WriteHeader(VariantRequest);
        header.WriteInt(ip_address);
        header.WriteByte(meta.Size);

        char key[64], value[64];

        for (int i = 0; i < ss.Size; i += 1)
        {
            ss.GetKey(i, key, sizeof key);
            meta.GetString(key, value, sizeof value);

            header.WriteString(key);
            header.WriteString(value);
        }

        delete ss;
        delete meta;

        return view_as<Request>(header);
	}
}


// methodmap Response < BaseMessage
// {
// 	public void responseMessage(int ip_address/*,byte info_byte*/)
// 	{
// 		BaseMessage mres = BaseMessage();
// 		/*

// 		char[] infoByte = info_byte.ConvertToString();
// 		int infilByte = <int>infoByte[0]
// 		char[7] limitByte = {};

// 		for(int i = 1; i < 8; i++){
// 			limitByte[i] = infoByte[i];
// 		}

// 		mres.writeByte(infilByte);
// 		mres.writeByte(<int> limitByte);
// 		mres.writeInt(ip_address);

// 		*/
// 	}
// }


// public void OnClientPostAdminCheck(int client)
// {
// 	StringMap mx = new StringMap();
// 	char keyBuf[20];
// 	char valBuf[20];
// 	if (!IsClientConnected(client))
// 		return;
// }

// public void OnPluginStart()
// {
// 	g_hSocket = SocketCreate(SOCKET_TCP, OnSocketError);

// 	SocketSetOption(g_hSocket, SocketReuseAddr, 1);
// 	SocketSetOption(g_hSocket, SocketKeepAlive, 1);

// 	#if defined DEBUG
// 	SocketSetOption(g_hSocket, DebugMode, 1);
// 	#endif
// }
