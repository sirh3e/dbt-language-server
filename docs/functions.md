# User-defined functions

dbt 1.11 added [user-defined functions](https://docs.getdbt.com/docs/build/udfs)
(UDFs): a function's body lives in `functions/`, its signature in a
`functions:` entry of a properties file, and a model calls it with
`{{ function('name') }}(…)`. dbt creates the function in the warehouse
before the models that call it. cogitator reads these functions as it reads
models, macros and docs blocks.

```text
functions/
  is_positive_int.sql     -- the body: an expression
  schema.yml              -- its functions: entry
models/
  orders.sql              -- select {{ function('is_positive_int') }}(amount) …
```

```yaml
functions:
  - name: is_positive_int
    description: Whether a string holds a positive integer
    config:
      volatility: deterministic
    arguments:
      - name: a_string
        data_type: string
        description: The text to check
    returns:
      data_type: boolean
```

These are not the dialect's own SQL functions (`regexp_instr`,
`date_trunc`). Those get hover and completion from the bundled dialect
documentation, as before.

## What you get

| Where | What |
|---|---|
| Hover on `function('is_positive_int')` (the name or the word `function`) | The signature, `is_positive_int(a_string string) -> boolean`, the description, each documented argument, what it returns, and its type, volatility, language, overloads and project |
| Go to definition, and the document link | The body file. A function with only a `functions:` entry goes to that entry's `name:` |
| Completion after `function('` | Every function of the project and its packages, its signature as the detail. The closing `') }}` is added as for `ref('`, and the documentation resolves when the item is selected |
| Diagnostics | `dangling-function`: a `function('…')` that names no function, with the closest names as did-you-mean. `dbt.unresolved` lists these under the kind `function` |
| Find references | From a call, or from the `name:` of the `functions:` entry: every call in the project's models, tests and analyses, plus the body and the entry as declarations |
| Semantic tokens | The name in `function('…')` is a `function` token |

`{{ function('f') }}(x)` stays valid SQL for everything else the server
reads (the syntax check, the query plan, column lineage). The call renders
as an identifier in front of its arguments, as dbt's compile renders it as
`database.schema.f(x)`.

## Where functions come from

- **`function-paths`** in `dbt_project.yml`. Without it, `functions/` is
  used when that directory exists, as dbt does.
- **Bodies**: every `.sql`, `.py` and `.js` file under those paths,
  subdirectories included. The file name without its extension is the
  function's name (`functions/text/slugify.py` is `slugify`). The
  extension gives the language that hover reports.
- **Signatures**: the `functions:` entries of every `.yml` and `.yaml` file
  under those paths: `name`, `description`, `arguments` (`name`,
  `data_type`, `description`, `default_value`), `returns` (`data_type`,
  `description`), `config.type`, `config.volatility` and `overloads`.
  Other keys are ignored, as warehouse-specific configs vary.
- **Overloads** (dbt 1.12): a body named in an `overloads[].defined_in`
  belongs to its root function and is not a function of its own.
- **Half-written functions** still count: a body without an entry (hover
  then says so), or an entry without a body (definition goes to the
  entry).
- **Packages**: functions of installed packages (`dbt_packages/`) count
  too. On a name both define, the project's own wins, as for docs blocks.
  `function()` takes no package argument.

## Not yet

- A function is not a node of the project graph yet. `:DbtDag`, the
  model-graph browser, impact, `dbt.select` with `resource_type:function`
  and the upstream lenses do not show functions, though dbt draws an edge
  from a function to each model that calls it.
- Renaming a function across its calls, its body file and its entry.
- Signature help for the arguments after `{{ function('f') }}(`, and a
  check of how many arguments a call passes.
- `functions:` entries outside the function paths. dbt recommends keeping
  them beside the bodies, and cogitator reads them only there.
- Hover in the body file itself, and the YAML schema's own completion of
  `functions:` keys.
