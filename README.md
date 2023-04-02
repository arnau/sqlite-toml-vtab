# SQlite TOML Virtual Table

A SQLite extension exposing [TOML](https://toml.io/) files as a virtual table.

WARNING: This extension is a toy project and you should not expect any active
maintenance. It depends on [sqlite-loadable-rs](https://github.com/asg017/sqlite-loadable-rs)
which is unstable.

Using the feature `rlib` the extension can be used as a rust lib, loadable using [rusqlite](https://github.com/rusqlite/rusqlite).


## Getting started

The examples below assume a set of TOML files containing a recipe each with a
shape such as:

```toml
name = "Yorkshire pudding"
preparation_time = "10 min"
cooking_time = "30 min"
servings = 12
instructions = """
(…)
"""
ingredients = [
  { name = "egg",    quantity = 3                      },
  { name = "milk",   quantity = 1, unit = "cup"        },
  { name = "flour",  quantity = 1, unit = "cup"        },
  { name = "butter", quantity = 2, unit = "tablespoon" },
]
```

### Load the extension

Assuming the compiled name is `toml_vtab`:

 ```sql
 .load ./dist/toml_vtab
```

### Initialise a virtual table

```sql
CREATE VIRTUAL TABLE temp.recipe USING toml(dirname="recipes");
 ```

The above will create a new table `recipe` within the `temp` schema with two
columns `filename` and `value` where the latter will have the full contents of
the TOML file as a JSON string.


## Querying the data

Say you want to list all ingredient names out of all the recipes:

```sql
SELECT DISTINCT
    json_extract(ingredient.value, '$.name') AS ingredient_name
FROM
    recipe, json_each(json_extract(recipe.value, '$.ingredients')) AS ingredient
ORDER BY ingredient_name
```


## Licence

Arnau Siches under the [MIT License](./LICENCE)
