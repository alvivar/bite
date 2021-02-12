# C# Bite

To **connect**.

    Bite bite = new Bite("127.0.0.1", 1984);

To **send**.

    bite.Send("s author Andrés Villalobos");
    bite.Send("j author");

You can use a **System.Action<string>** callback on **send** to deal directly
with the **response**.

    bite.Send("g author", response => {
        // Handle your response.
    });

You also have a couple **System.Action<string>** to suscribe.

    bite.OnResponse += YourOnResponse;
    bite.OnError += YourOnError;

That's it!
