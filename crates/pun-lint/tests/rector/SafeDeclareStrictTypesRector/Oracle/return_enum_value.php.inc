<?php

namespace Pun\Oracle\SafeStrict;

enum Level: string
{
    case Low = 'low';
}

final class LevelValue
{
    public function default(): string
    {
        return Level::Low->value;
    }

    public function name(): string
    {
        return Level::Low->name;
    }
}
