# API

## SET

    set name           Bite
    set version        v0.1
    set description    A json database.

    set user.0.id                         20210202170229
    set user.0.profile.name               Andrés Villalobos
    set user.0.profile.twitter            twitter.com/matnesis
    set user.0.workingOn.0.name           The Eye of Minds
    set user.0.workingOn.0.description    A game about death and hell.
    set user.0.workingOn.1                Bite
    set user.0.workingOn.1.description    Minimalist Key-Value Database for quick prototypes.

    set user.1.id              20210202172717
    set user.1.profile.name    This can be you!

## GET

    get name ->
    Bite

    get user.1.profile.name ->
    This can be you!

    get user ->
    {
    	"user": [
    		{
    			"id": 20210202170229,
    			"profile": {
    				"name": "Andrés Villalobos",
    				"twitter": "twitter.com/matnesis",
    			},
    			"workingOn": [
    				{
    					"name": "The Eye of Minds",
    					"description": "A game about death and hell."
    				},
    				{
    					"name": "Bite",
    					"description": "Minimalist Key-Value Database for quick prototypes."
    				}
    			]
    		},
    		{
    			"id": 20210202172717,
    			"profile": {
    				"name": "This can be you!"
    			}
    		}
    	]
    }
