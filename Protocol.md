# BITE Protocol

6 bytes as header, then any data.

    [  2 BYTES  ][  2 BYTES   ][ 2 BYTES ][ MAX 65535 - 6 ]
    [ CLIENT ID ][ MESSAGE ID ][   SIZE  ][   DATA BYTES  ]

-   2 bytes to represent the client ID.
-   2 bytes to represent the message ID.
-   2 bytes to represent the size of the complete message (including the
    header).

-   Because we are using 2 bytes to represent the size, the maximum size of the
    message is 65535 bytes, and because the header is 6 bytes, the maximum size
    of the data in it is 65529 bytes.
