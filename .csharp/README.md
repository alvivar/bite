# C# Bite

To connect.

    var bite = new Bite("127.0.0.1", 1984);

To send.

    bite.Send("s author Andrés Villalobos");
    bite.Send("j author");

To receive use the System.Actions that receive <string>.

    bite.OnError = YourOnError;
    bite.OnResponse = YourOnResponse;
