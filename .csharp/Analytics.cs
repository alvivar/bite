using System;
using UnityEngine;

internal class Pos { public float x; public float y; public float z; }

[System.Serializable]
public class AnalyticsData
{
    public string name;
    public int timePlayed;
    public Vector3 lastPosition;
    public long lastEpoch;
    public long startedEpoch;
}

public class Analytics : MonoBehaviour
{
    public string user = "team.game.user";

    [Header("Server")]
    public string host = "142.93.180.20";
    public int port = 1984;

    [Header("Info")]
    public string id; // SystemInfo.deviceUniqueIdentifier
    public string key;
    public AnalyticsData data;

    [Header("Config")]
    public int tick = 3;
    public float clock = 1;

    [Header("Optional")]
    public Transform position;

    private bool connected = false;
    private bool lastPositionLoaded = false;

    private Bite bite;

    void Start()
    {
        id = SystemInfo.deviceUniqueIdentifier;
        key = $"{user}.{id}";

        Connect();
    }

    void Update()
    {
        // Tick
        if (Time.time < clock)
            return;
        clock = Time.time + tick;

        // Ping until first response.
        if (!connected)
        {
            bite.Send("g");
            return;
        }

        // Statistics
        SaveTimePlayed(tick);

        SaveLastEpoch();

        SaveLastPosition();
    }

    void Connect()
    {
        bite = new Bite(host, port);
        bite.OnResponse += OnResponse;
        bite.OnError += OnError;
    }

    void OnDestroy()
    {
        if (bite != null)
        {
            bite.Stop();
            bite.OnResponse -= OnResponse;
            bite.OnError -= OnError;
        }
    }

    void OnError(string error)
    {
        Debug.Log($"Analytics error: {error}");

        OnDestroy();
        connected = false;
        Start();
    }

    void OnResponse(string response)
    {
        if (!connected)
        {
            connected = true;

            Debug.Log($"Analytics connected");

            LoadDataFromServer();

            LoadOrSetStartedEpoch();
        }
    }

    void LoadDataFromServer()
    {
        bite.Send($"g {key}.name", response =>
        {
            if (response.Trim().Length < 1)
                response = "?";

            data.name = response;
        });

        bite.Send($"g {key}.timePlayed", response =>
        {
            data.timePlayed = Bite.Int(response, 0);
        });

        bite.Send($"j {key}.lastPosition", response =>
        {
            var json = JsonUtility.FromJson<Pos>(response);

            data.lastPosition = new Vector3(
                Bite.Float($"{json.x}", 0),
                Bite.Float($"{json.y}", 0),
                Bite.Float($"{json.z}", 0));

            lastPositionLoaded = true;
        });
    }

    void SaveTimePlayed(int time)
    {
        data.timePlayed += time;
        bite.Send($"s {key}.timePlayed {data.timePlayed}");
    }

    void SaveLastEpoch()
    {
        data.lastEpoch = DateTimeOffset.Now.ToUnixTimeSeconds();
        bite.Send($"s {key}.lastEpoch {data.lastEpoch}");
    }

    void LoadOrSetStartedEpoch()
    {
        bite.Send($"g {key}.startedEpoch", response =>
        {
            data.startedEpoch = Bite.Long(response, 0);

            if (data.startedEpoch <= 0)
            {
                data.startedEpoch = DateTimeOffset.Now.ToUnixTimeSeconds();
                bite.Send($"s {key}.startedEpoch {data.startedEpoch}");
            }
        });
    }

    void SaveLastPosition()
    {
        if (!position || !lastPositionLoaded)
            return;

        if (data.lastPosition == position.transform.position)
            return;

        data.lastPosition = position.transform.position;

        var x = $"s {key}.lastPosition.x {data.lastPosition.x}\n";
        var y = $"s {key}.lastPosition.y {data.lastPosition.y}\n";
        var z = $"s {key}.lastPosition.z {data.lastPosition.z}";
        bite.Send($"{x}{y}{z}");
    }

    public void SetName(string name)
    {
        data.name = name;
        bite.Send($"s {key}.name {data.name}");
    }
}