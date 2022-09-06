# BITE Protocol

6 bytes as header, then a maximum of 65529 bytes of data, a total of 65535 bytes
together.

    [  2 Bytes  ][  2 Bytes   ][ 2 Bytes ][ Max 65535 - 6 ]
    [ Client Id ][ Message Id ][   Size  ][   Data Bytes  ]

## Details

-   2 bytes to represent the client id.
-   2 bytes to represent the message id.
-   2 bytes to represent the size of the complete message (including the
    header).
-   We are using 2 bytes to represent the size, so, the maximum size of the
    message can be 65535 bytes, and the header is 6 bytes, so, the maximum size
    of the data is 65529 bytes.

## Expected from a client

-   When a client connects, BITE sends an id. This id needs be included in the
    first two bytes in all messages or BITE would disconnect the client.
-   The client can do whatever it wants with the message id. Bite will always
    include the same client id and message id on each respective response to the
    query sent, so, the recommendation is using the message id to know which
    query send the message.

## Goals

-   Fast
-   Simple
-   Small

## What's missing for production

-   An authentication method for clients
