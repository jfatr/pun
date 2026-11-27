<?php

/**
 * @var array<string, mixed> $args
 */

if (empty($args['post_id']) || ! is_numeric($args['post_id'])) {
    return;
}

$card_post = get_post((int) $args['post_id']);

if (! $card_post instanceof \WP_Post || 'publish' !== $card_post->post_status) {
    return;
}

$classes = isset($args['classes']) && is_string($args['classes']) ? $args['classes'] : '';
echo $classes;

function read(mixed $value): void
{
    if ($value instanceof \Missing) {
        \PHPStan\dumpType($value->status);
        \PHPStan\dumpType($value?->status);
        $value->status = 'draft';
    }
}
