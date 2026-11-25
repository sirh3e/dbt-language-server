# udf_shop

A made-up dbt project for the user-defined functions (`functions/`,
`{{ function('…') }}`, dbt 1.11+). What it holds, and why:

- `is_positive_int` — a SQL body with its `functions:` entry (arguments,
  returns, config) and one overload, `is_positive_int_numeric`, whose body
  is no function of its own.
- `text/slugify.py` — a Python body without a `functions:` entry.
- `masked_email` — a `functions:` entry without a body.
- `dbt_packages/udf_pkg` — a package that defines `is_positive_int` again
  (the project's own wins) and `pkg_only`.
- `models/checked_orders.sql` — calls each, and `is_postive_int`, a typo.
