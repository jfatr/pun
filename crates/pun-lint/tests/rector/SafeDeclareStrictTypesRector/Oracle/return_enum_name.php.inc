<?php

namespace Pun\Oracle\SafeStrict;

enum Size
{
    case Small;
}

final class SizeName
{
    public function name(): string
    {
        return Size::Small->name;
    }
}
