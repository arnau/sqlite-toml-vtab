#![cfg(feature = "loadable")]

use fallible_iterator::FallibleIterator;
use rusqlite::{Connection, LoadExtensionGuard, Result};

fn load_toml_vtab(conn: &Connection) -> Result<()> {
    unsafe {
        let _guard = LoadExtensionGuard::new(conn)?;
        conn.load_extension("dist/toml_vtab", None)
    }
}

#[test]
fn test_toml_module() -> Result<()> {
    let db = Connection::open_in_memory()?;
    load_toml_vtab(&db)?;

    db.execute_batch(
        r#"
            CREATE VIRTUAL TABLE vtab
            USING toml(dirname="tests/data")
        "#,
    )?;

    {
        let mut s = db.prepare("SELECT rowid, * FROM vtab")?;
        {
            let headers = s.column_names();
            assert_eq!(vec!["rowid", "filename", "value"], headers);
        }

        let ids: Result<Vec<i32>> = s.query([])?.map(|row| row.get::<_, i32>(0)).collect();
        let sum = ids?.iter().sum::<i32>();
        assert_eq!(sum, 3);
    }
    db.execute_batch("DROP TABLE vtab")
}

#[test]
fn test_toml_cursor() -> Result<()> {
    let db = Connection::open_in_memory()?;
    load_toml_vtab(&db)?;

    db.execute_batch(
        r#"
            CREATE VIRTUAL TABLE vtab
            USING toml(dirname="tests/data")
        "#,
    )?;

    {
        let mut s = db.prepare(
            r#"
                SELECT
                    count(1)
                FROM
                    vtab
            "#,
        )?;

        let mut rows = s.query([])?;
        let row = rows.next()?.unwrap();
        assert_eq!(row.get_unwrap::<_, i32>(0), 2);
    }
    db.execute_batch("DROP TABLE vtab")
}

#[test]
fn test_toml_json() -> Result<()> {
    let db = Connection::open_in_memory()?;
    load_toml_vtab(&db)?;
    db.execute_batch(
        r#"
            CREATE VIRTUAL TABLE vtab
            USING toml(dirname="tests/data")
        "#,
    )?;

    {
        let mut s = db.prepare(
            r#"
                SELECT DISTINCT
                    keyword.value
                FROM
                    vtab, json_each(json_extract(vtab.value, '$.keywords')) AS keyword
                ORDER BY keyword.value
            "#,
        )?;

        let keywords: Result<Vec<String>> = s.query([])?.map(|row| row.get(0)).collect();
        assert_eq!(keywords.unwrap(), vec!["bar", "foo", "qux"]);
    }
    db.execute_batch("DROP TABLE vtab")
}
