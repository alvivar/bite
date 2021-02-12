# C# Bite

To **connect**.

    var bite = new Bite("127.0.0.1", 1984);

To **send**.

    bite.Send("s author Andrés Villalobos");
    bite.Send("j author");

To receive use **System.Action<string>**.

    bite.OnResponse = YourOnResponse;
    bite.OnError = YourOnError;
    
