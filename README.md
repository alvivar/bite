# Bite

Minimalist Key Value Store.

It's basically a chat, a TcpListener waiting for messages to GET and SET values.

## How

To set a value:

    SET somekeyname Whatever kind of string I guess
    > Ok.

To get the value:

    GET somekeyname
    > Whatever kind of string I guess

To get the value or default:

    GET keyname Some kind of default, just in case
    > Some kind of default, just in case
