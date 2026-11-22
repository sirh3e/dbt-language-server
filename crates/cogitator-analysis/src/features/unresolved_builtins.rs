//! What dbt provides to a `{{ name(...) }}` call without a package: the
//! Jinja context functions and the global macros of dbt core and its
//! adapters (cross-database helpers, materialization helpers). A call to
//! one of these is never reported by `dbt.unresolved`.

const BUILTINS: &[&str] = &[
    // the Jinja context
    "adapter",
    "api",
    "builtins",
    "caller",
    "config",
    "cycler",
    "debug",
    "dispatch",
    "doc",
    "env_var",
    "exceptions",
    "fromjson",
    "fromyaml",
    "graph",
    "is_incremental",
    "joiner",
    "lipsum",
    "load_result",
    "local_md5",
    "log",
    "modules",
    "namespace",
    "print",
    "range",
    "ref",
    "return",
    "run_query",
    "set",
    "set_strict",
    "source",
    "statement",
    "store_result",
    "store_raw_result",
    "submit_python_job",
    "this",
    "tojson",
    "toyaml",
    "try_or_compiler_error",
    "var",
    "zip",
    "zip_strict",
    "diff_of_two_dicts",
    "dict",
    // global macros: cross-database helpers
    "any_value",
    "array_append",
    "array_concat",
    "array_construct",
    "bool_or",
    "cast",
    "cast_bool_to_text",
    "concat",
    "current_timestamp",
    "current_timestamp_backcompat",
    "current_timestamp_in_utc_backcompat",
    "date",
    "date_spine",
    "date_trunc",
    "dateadd",
    "datediff",
    "escape_single_quotes",
    "except",
    "generate_series",
    "get_intervals_between",
    "get_powers_of_two",
    "hash",
    "intersect",
    "last_day",
    "length",
    "listagg",
    "position",
    "replace",
    "right",
    "safe_cast",
    "split_part",
    "string_literal",
    "type_bigint",
    "type_boolean",
    "type_float",
    "type_int",
    "type_numeric",
    "type_string",
    "type_timestamp",
    // global macros: naming, relations and materializations
    "alter_column_comment",
    "alter_relation_comment",
    "create_schema",
    "create_table_as",
    "create_view_as",
    "drop_relation",
    "drop_relation_if_exists",
    "drop_schema",
    "generate_alias_name",
    "generate_database_name",
    "generate_schema_name",
    "get_columns_in_query",
    "get_columns_in_relation",
    "get_create_table_as_sql",
    "get_where_subquery",
    "incremental_validate_on_schema_change",
    "make_temp_relation",
    "persist_docs",
    "run_hooks",
    "should_full_refresh",
    "should_store_failures",
    "truncate_relation",
];

/// Is `name` something dbt itself provides?
pub fn is_dbt_builtin(name: &str) -> bool {
    BUILTINS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::is_dbt_builtin;
    use rstest::rstest;

    #[rstest]
    #[case("is_incremental", true)]
    #[case("current_timestamp", true)]
    #[case("generate_schema_name", true)]
    #[case("cents_to_dollars", false)]
    #[case("my_macro", false)]
    fn dbt_provides_its_builtins(#[case] name: &str, #[case] expected: bool) {
        assert_eq!(is_dbt_builtin(name), expected);
    }
}
