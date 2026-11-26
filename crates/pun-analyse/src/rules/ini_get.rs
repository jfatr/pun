//! PHPStan's `IniGetReturnTypeExtension`: `ini_get()` of an option PHP always defines is a `string` (a
//! `numeric-string` for the numeric ones), not `string|false`. Any other option, or a name that is not a constant
//! string, keeps the stub's return type.

use mago_analyzer::plugin::context::{InvocationInfo, ProviderContext};
use mago_analyzer::plugin::provider::function::{FunctionReturnTypeProvider, FunctionTarget};
use mago_analyzer::plugin::provider::{Provider, ProviderMeta};
use mago_codex::ttype::combiner::CombinerOptions;
use mago_codex::ttype::union::TUnion;
use mago_codex::ttype::{combine_union_types, get_numeric_string, get_string};

static META: ProviderMeta =
    ProviderMeta::new("pun-ini-get", "pun", "PHPStan's ini_get() return type");

/// `max_memory_limit` exists from PHP 8.5 (PhpVersions::supportsMaxMemoryLimit()).
pub(super) struct IniGet {
    pub(super) php_version: u32,
}

impl Provider for IniGet {
    fn meta() -> &'static ProviderMeta {
        &META
    }
}

impl FunctionReturnTypeProvider for IniGet {
    fn targets() -> FunctionTarget {
        FunctionTarget::Exact(b"ini_get")
    }

    fn get_return_type(
        &self,
        context: &ProviderContext<'_, '_, '_>,
        invocation: &InvocationInfo<'_, '_, '_>,
    ) -> Option<TUnion> {
        let argument = context.get_expression_type(invocation.get_argument(0, &[b"option"])?)?;
        let mut result: Option<TUnion> = None;
        for atomic in argument.types.iter() {
            let ty = match atomic.get_literal_string_value()? {
                b"date.timezone" | b"memory_limit" => get_string(),
                b"max_memory_limit" if self.php_version >= 80500 => get_string(),
                b"max_execution_time"
                | b"max_input_time"
                | b"default_socket_timeout"
                | b"precision" => get_numeric_string(),
                _ => return None,
            };
            result = Some(match result {
                Some(all) => {
                    combine_union_types(&all, &ty, context.codebase(), CombinerOptions::default())
                }
                None => ty,
            });
        }
        result
    }
}
