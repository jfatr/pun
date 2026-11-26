<?php

namespace PunBenevolentTranslations;

use function PHPStan\Testing\assertType;

final class Labels
{
    public function ternary(bool $short): string
    {
        return $short ? 'Hi' : __('Hello');
    }

    public function match(int $count): string
    {
        return match (true) {
            $count === 0 => __('None'),
            default => trans_choice(':count item|:count items', $count),
        };
    }

    public function append(bool $more): string
    {
        $text = __('Hello');

        if ($more) {
            $text .= __('World');
        }

        return $text;
    }

    public function argument(string $text): string
    {
        return mb_substr($text === '' ? __('Nothing') : $text, 0, 10);
    }

    public function nullable(bool $flag): string
    {
        return $flag ? null : __('Hello');
    }
}

function types(bool $b, string $s, ?string $ns): void
{
    assertType('(array|string)', $b ? 'Hi' : __('Hello'));
    assertType('(array|string)', $b ? $s : __('Hello'));
    assertType('1|array|string', $b ? 1 : __('Hello'));
    assertType('array|string|null', $b ? __('Hello') : $ns);
}
