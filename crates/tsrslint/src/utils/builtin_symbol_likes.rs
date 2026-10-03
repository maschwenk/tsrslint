//! Ports of internal/utils/builtin_symbol_likes.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

use tsrs_checker::{Checker, Type};
use tsrs_compiler::Program;

/// builtin_symbol_likes.go IsAnyBuiltinSymbolLike.
pub fn is_any_builtin_symbol_like(program: &Program, c: &mut Checker, t: P<Type>) -> bool {
    is_builtin_symbol_like_recurser(program, c, t, &mut |_c, sub| {
        let Some(symbol) = sub.symbol() else {
            return BuiltinMatch::False;
        };
        if is_symbol_from_default_library(program, Some(symbol)) {
            BuiltinMatch::True
        } else {
            BuiltinMatch::Unknown
        }
    })
}
