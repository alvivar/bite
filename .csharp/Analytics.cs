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
    public string app = "default";

    [Header("Info")]
    public string id; // SystemInfo.deviceUniqueIdentifier
    public string key;
    public AnalyticsData data;

    [Header("Config")]
    public int tick = 3;
    public float timer = 3;

    [Header("Optional")]
    public Transform position;

    private Bite bite;

    private bool connected = false;
    private bool lastPositionLoaded = false;

    void Start()
    {
        bite = new Bite("142.93.180.20", 1984);

        bite.OnResponse = OnResponse;
        bite.OnError = OnError;

        id = SystemInfo.deviceUniqueIdentifier;
        key = $"{app}.{id}";
    }

    void Update()
    {
        // Tick.
        if (Time.time < timer)
            return;
        timer = Time.time + tick;

        // Ping until first response.
        if (!connected)
        {
            bite.Send("g");
            return;
        }

        // Statistics.
        SaveTimePlayed(tick);
        SaveLastEpoch();

        SaveLastPosition();
    }

    void OnDestroy() { bite.Stop(); }

    void OnError(string error) { Debug.Log($"{error}"); }

    void OnResponse(string response)
    {
        if (!connected)
        {
            connected = true;

            UpdateDataFromServer();

            LoadOrSetStartedEpoch();

            Debug.Log($"Analytics Started.");
            Debug.Log($"> {response}");
        }
    }

    void UpdateDataFromServer()
    {
        bite.Send($"g {app}.{id}.name", response =>
        {
            if (response.Trim().Length < 1)
                response = "?";

            data.name = response;
        });

        bite.Send($"g {app}.{id}.timePlayed", response =>
        {
            data.timePlayed = Bite.IntOr(response, 0);
        });

        bite.Send($"j {app}.{id}.lastPosition", response =>
        {
            var json = JsonUtility.FromJson<Pos>(response);

            data.lastPosition = new Vector3(
                Bite.FloatOr($"{json.x}", 0),
                Bite.FloatOr($"{json.y}", 0),
                Bite.FloatOr($"{json.z}", 0)
            );

            lastPositionLoaded = true;

        });
    }

    void SaveTimePlayed(int time)
    {
        data.timePlayed += time;
        bite.Send($"s {app}.{id}.timePlayed {data.timePlayed}");
    }

    void SaveLastEpoch()
    {
        data.lastEpoch = DateTimeOffset.Now.ToUnixTimeSeconds();
        bite.Send($"s {app}.{id}.lastEpoch {data.lastEpoch}");
    }

    void LoadOrSetStartedEpoch()
    {
        var key = $"{app}.{id}.startedEpoch";

        bite.Send($"g {key}", response =>
        {
            data.startedEpoch = Bite.LongOr(response, 0);

            if (data.startedEpoch <= 0)
            {
                data.startedEpoch = DateTimeOffset.Now.ToUnixTimeSeconds();
                bite.Send($"s {key} {data.startedEpoch}");
            }
        });
    }

    void SaveLastPosition()
    {
        if (!position || !lastPositionLoaded)
            return;

        data.lastPosition = position.transform.position;

        bite.Send($"s {app}.{id}.lastPosition.x {data.lastPosition.x}");
        bite.Send($"s {app}.{id}.lastPosition.y {data.lastPosition.y}");
        bite.Send($"s {app}.{id}.lastPosition.z {data.lastPosition.z}");
    }

    public void SetName(string name)
    {
        data.name = name;
        bite.Send($"s {app}.{id}.name {data.name}");
    }
}