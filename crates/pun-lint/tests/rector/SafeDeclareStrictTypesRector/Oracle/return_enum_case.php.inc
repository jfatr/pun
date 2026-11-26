<?php

namespace Pun\Oracle\SafeStrict;

enum Status: string
{
    case Active = 'active';

    public static function default(): self
    {
        return self::Active;
    }
}
