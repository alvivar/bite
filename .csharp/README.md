# CSharp & Bite

## Tutorial

To connect:

    var bite = new Bite("142.93.180.20", 1984);

To receive:

    bite.OnError = YourOnError;
    bite.OnResponse = YourOnResponse;

To send:

    bite.Send("s author Andrés Villalobos");
    bite.Send("j author");
