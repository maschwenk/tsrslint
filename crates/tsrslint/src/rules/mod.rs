// The implemented rules (a vertical slice of tsgolint's ~60).

use crate::rule::Rule;

pub mod no_floating_promises;
pub mod no_implied_eval;
pub mod no_unnecessary_boolean_literal_compare;
pub mod promise_function_async;
pub mod return_await;

pub const RULE_NAMES: &[&str] = &[
    "no-floating-promises",
    "no-implied-eval",
    "no-unnecessary-boolean-literal-compare",
    "promise-function-async",
    "return-await",
];

pub fn create_rule(name: &str, options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    match name {
        "no-floating-promises" => no_floating_promises::create(options),
        "no-implied-eval" => Ok(Box::new(no_implied_eval::NoImpliedEval)),
        "no-unnecessary-boolean-literal-compare" => no_unnecessary_boolean_literal_compare::create(options),
        "promise-function-async" => promise_function_async::create(options),
        "return-await" => return_await::create(options),
        _ => Err(format!("unknown rule: {name} (tsrslint implements only: {})", RULE_NAMES.join(", "))),
    }
}
