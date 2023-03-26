//! TOML Virtual Table.
//!
//! Reads TOML files and exposes them as a single virtual table where the content of each file is
//! can be queried as JSON.
use rusqlite::ffi;
use rusqlite::types::Null;
use rusqlite::vtab::{
    escape_double_quote, parameter, read_only_module, Context, CreateVTab, IndexInfo, VTab,
    VTabConfig, VTabConnection, VTabCursor, VTabKind, Values,
};
use rusqlite::{Connection, Error, Result};
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::path::Path;
use std::str;
use walkdir::WalkDir;

/// Register the "toml" module.
///
/// ```sql
/// CREATE VIRTUAL TABLE vtab USING toml(
///   dirname=DIRNAME -- Name of directory containing TOML files.
/// );
/// ```
pub fn load_module(conn: &Connection) -> Result<()> {
    let aux: Option<()> = None;
    conn.create_module("toml", read_only_module::<TomlTab>(), aux)
}


/// An instance of the CSV virtual table
#[repr(C)]
struct TomlTab {
    /// Base class. Must be first
    base: ffi::sqlite3_vtab,
    /// Name of the CSV file
    dirname: String,
    headers: Headers,
    schema: String,
}

type Headers = Vec<String>;
type Record = Vec<String>;
type Records = Vec<Record>;

impl TomlTab {
    fn read_data(&mut self) -> Result<Records, anyhow::Error> {
        let mut records: Records = Vec::new();

        for entry in WalkDir::new("test_data") {
            let entry = entry?;
            if entry.file_type().is_file() {
                let b = std::fs::read(entry.path())?;
                let s = String::from_utf8_lossy(&b);
                let value: toml::Value = toml::from_str(&s)?;
                let data = serde_json::to_string(&value)?;
                let record = vec![entry.path().display().to_string(), data];

                records.push(record);
            }
        }

        Ok(records)
    }
}

unsafe impl<'vtab> VTab<'vtab> for TomlTab {
    type Aux = ();
    type Cursor = TomlTabCursor<'vtab>;

    fn connect(
        db: &mut VTabConnection,
        _aux: Option<&()>,
        args: &[&[u8]],
    ) -> Result<(String, TomlTab)> {
        if args.len() < 4 {
            return Err(Error::ModuleError("no dirname specified".to_owned()));
        }

        let mut vtab = TomlTab {
            base: ffi::sqlite3_vtab::default(),
            dirname: "".to_owned(),
            headers: vec!["filename".to_string(), "value".to_string()],
            schema: r#"
                CREATE TABLE x(
                    "filename" TEXT,
                    "value" TEXT
                );
            "#
            .to_string(),
        };
        let args = &args[3..];
        for c_slice in args {
            let (param, value) = parameter(c_slice)?;
            match param {
                "dirname" => {
                    let value = value.trim();
                    if !Path::new(value).exists() {
                        return Err(Error::ModuleError(format!("dir '{value}' does not exist",)));
                    }
                    vtab.dirname = value.to_owned();
                }
                _ => {
                    return Err(Error::ModuleError(format!(
                        "unrecognized parameter '{param}'",
                    )));
                }
            }
        }

        if vtab.dirname.is_empty() {
            return Err(Error::ModuleError("no directory specified".to_owned()));
        }

        // let mut sql = String::from("CREATE TABLE x(");
        // for (i, col) in vtab.headers.iter().enumerate() {
        //     sql.push('"');
        //     sql.push_str(col);
        //     sql.push_str("\" TEXT");
        //     if i == vtab.headers.len() - 1 {
        //         sql.push_str(");");
        //     } else {
        //         sql.push_str(", ");
        //     }
        // }
        // dbg!(&sql);

        db.config(VTabConfig::DirectOnly)?;
        Ok((vtab.schema.clone(), vtab))
    }

    // Only a forward full table scan is supported.
    fn best_index(&self, info: &mut IndexInfo) -> Result<()> {
        info.set_estimated_cost(1_000_000.);
        Ok(())
    }

    fn open(&mut self) -> Result<TomlTabCursor<'_>> {
        let records = self
            .read_data()
            .map_err(|err| Error::ModuleError(err.to_string()))?;

        Ok(TomlTabCursor::new(self.headers.clone(), records))
    }
}

impl CreateVTab<'_> for TomlTab {
    const KIND: VTabKind = VTabKind::Default;
}

/// A cursor for the CSV virtual table
#[repr(C)]
struct TomlTabCursor<'vtab> {
    /// Base class. Must be first
    base: ffi::sqlite3_vtab_cursor,
    /// The record headers (i.e. the column names)
    headers: Headers,
    records: Records,
    /// Current cursor position used as rowid
    row_number: usize,
    eof: bool,
    phantom: PhantomData<&'vtab TomlTab>,
}

impl TomlTabCursor<'_> {
    fn new<'vtab>(headers: Headers, records: Records) -> TomlTabCursor<'vtab> {
        TomlTabCursor {
            base: ffi::sqlite3_vtab_cursor::default(),
            headers,
            records,
            row_number: 1,
            eof: false,
            phantom: PhantomData,
        }
    }
}

unsafe impl VTabCursor for TomlTabCursor<'_> {
    // Only a full table scan is supported.  So `filter` simply rewinds to
    // the beginning.
    fn filter(
        &mut self,
        _idx_num: c_int,
        _idx_str: Option<&str>,
        _args: &Values<'_>,
    ) -> Result<()> {
        self.row_number = 1;

        Ok(())
    }

    fn next(&mut self) -> Result<()> {
        if self.row_number == self.records.len() {
            self.eof = true;
            return Ok(());
        }

        self.row_number += 1;

        Ok(())
    }

    fn eof(&self) -> bool {
        self.eof
    }

    fn column(&self, ctx: &mut Context, col: c_int) -> Result<()> {
        if col < 0 || col as usize >= self.headers.len() {
            return Err(Error::ModuleError(format!(
                "column index out of bounds: {col}"
            )));
        }

        if let Some(record) = &self.records.get(self.row_number) {
            ctx.set_result(&record[col as usize].to_owned())
        } else {
            ctx.set_result(&Null)
        }
    }

    fn rowid(&self) -> Result<i64> {
        Ok(self.row_number as i64)
    }
}

// impl From<csv::Error> for Error {
//     #[cold]
//     fn from(err: csv::Error) -> Error {
//         Error::ModuleError(err.to_string())
//     }
// }

#[cfg(test)]
mod test {
    use rusqlite::{Connection, Result};
    use fallible_iterator::FallibleIterator;

    #[test]
    fn test_toml_module() -> Result<()> {
        let db = Connection::open_in_memory()?;
        super::load_module(&db)?;
        db.execute_batch(r#"
            CREATE VIRTUAL TABLE vtab
            USING toml(dirname=test_data)
        "#)?;

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
        super::load_module(&db)?;
        db.execute_batch(r#"
            CREATE VIRTUAL TABLE vtab
            USING toml(dirname=test_data)
        "#)?;

        {
            let mut s = db.prepare(r#"
                SELECT
                    count(1)
                FROM
                    vtab
            "#,)?;

            let mut rows = s.query([])?;
            let row = rows.next()?.unwrap();
            assert_eq!(row.get_unwrap::<_, i32>(0), 2);
        }
        db.execute_batch("DROP TABLE vtab")
    }

    #[test]
    fn test_toml_json() -> Result<()> {
        let db = Connection::open_in_memory()?;
        super::load_module(&db)?;
        db.execute_batch(r#"
            CREATE VIRTUAL TABLE vtab
            USING toml(dirname=test_data)
        "#)?;

        {
            let mut s = db.prepare(r#"
                SELECT DISTINCT
                    keyword.value
                FROM
                    vtab, json_each(json_extract(vtab.value, '$.keywords')) AS keyword
                ORDER BY keyword.value
            "#,)?;

            let keywords: Result<Vec<String>> = s.query([])?.map(|row| row.get(0)).collect();
            assert_eq!(keywords.unwrap(), vec!["bar", "foo", "qux"]);
        }
        db.execute_batch("DROP TABLE vtab")
    }

}
