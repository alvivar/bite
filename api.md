    s data.name Bite
    s data.why Simplest database ever
    s data.author.name Andrés Villalobos
    s data.author.twitter matnesis

    js data
    >
    {
        "data": {
            "author": {
                "name": "Andrés Villalobos",
                "twitter": "matnesis"
            },
            "name": "Bite",
            "why": "Simplest database ever"
        }
    }

    js data.why
    >
    {
        "data": {
            "why": "Simplest database ever"
        }
    }

    J data
    >
    {
        "author": {
            "name": "Andrés Villalobos",
            "twitter": "matnesis"
        },
        "name": "Bite",
        "why": "Simplest database ever"
    }

    J data.why
    > "Simplest database ever"
