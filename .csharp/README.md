# CSharp & Bite

## How

To connect

    var bite = new Bite("142.93.180.20", 1984);

receive

    bite.OnError = YourOnError;
    bite.OnResponse = YourOnResponse;

and Send.

    bite.Send("s author Andrés Villalobos");
    bite.Send("j author");
