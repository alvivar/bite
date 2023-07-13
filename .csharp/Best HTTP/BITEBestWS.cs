using UnityEngine;
using System;
using BestHTTP.WebSocket;
using BITEClient;
using System.Text;

public class BITEBestWS : MonoBehaviour

{
    // private string address = "ws://127.0.0.1:1983/ws";
    private string address = "ws://167.99.58.31:1991/ws";
    private int serverClientId = -1;
    private int messageId = 0;

    private WebSocket webSocket;
    private Frames frames = new Frames();

    public void Start()
    {
        // Create the WebSocket instance
        this.webSocket = new WebSocket(new Uri(address));

        // Subscribe to the WS events
        this.webSocket.OnOpen += OnOpen;
        this.webSocket.OnMessage += OnMessageReceived;
        this.webSocket.OnBinary += OnBinaryReceived;
        this.webSocket.OnClosed += OnClosed;
        this.webSocket.OnError += OnError;

        // Start connecting to the server
        this.webSocket.Open();
    }

    void OnDestroy()
    {
        if (this.webSocket != null)
        {
            this.webSocket.Close();
            this.webSocket = null;
        }
    }

    [ContextMenu("Close")]
    public void Close()
    {
        this.webSocket.Close(1000, "Bye!");
    }

    [ContextMenu("Send Test Messages")]
    public void SentTestMessages()
    {
        var date = DateTime.Now;

        for (int i = 0; i < 1000; i++)
        {
            Send($"#g bite.best.ws.date.{i}");
            Send($"s bite.best.ws.date.{i} {date}");
            Send($"g bite.best.ws.date.{i}");
            Send($"d bite.best.ws.date.{i}");
        }
    }

    public void Send(string message)
    {
        var data = Encoding.UTF8.GetBytes(message);
        var frame = new Frame().FromProtocol(serverClientId, ++messageId, data);
        this.webSocket.Send(frame.Bytes);
    }

    void OnOpen(WebSocket ws)
    {
        Debug.Log("BITE WS Opened!");
    }

    void OnMessageReceived(WebSocket ws, string message)
    {
        Debug.Log($"BITE WS Message Received ({message.Length}): {message}");
    }

    void OnBinaryReceived(WebSocket ws, byte[] message)
    {
        frames.Feed(message);
        while (frames.ProcessingCompleteFrames()) ;
        while (frames.HasCompleteFrame)
        {
            var frame = frames.Dequeue();

            // The first message is the id.
            if (serverClientId == -1)
                serverClientId = frame.ClientId;

            Debug.Log($"BITE WS Binary Received ({message.Length}), {message} Frame: {frame.ClientId} {frame.MessageId} {frame.Size} {frame.Text}");
        }
    }

    void OnClosed(WebSocket ws, UInt16 code, string message)
    {
        Debug.Log($"BITE WS Closed: {code} {message}");
        webSocket = null;
    }

    void OnError(WebSocket ws, string error)
    {
        Debug.Log($"BITE WS Error: {error}");
        webSocket = null;
    }
}
